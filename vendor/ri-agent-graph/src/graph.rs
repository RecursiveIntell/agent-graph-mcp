#![allow(deprecated)] // Constructs GraphEvent with legacy trace_id/attempt fields during migration

use crate::checkpoint::Checkpoint;
use crate::checkpoint_store::CheckpointStore;
use crate::checkpointer::CheckpointSaver;
use crate::command::NodeOutput;
use crate::config::GraphConfig;
use crate::edge::EdgeType;
use crate::error::{AgentGraphError, Result};
use crate::event_sink::{ChannelEventSink, EventSink, NoopEventSink};
use crate::executor::Executor;
use crate::interrupt::InterruptConfig;
use crate::node::Node;
use crate::reducer::Reducer;
use crate::retry::RetryPolicy;
use crate::state::AgentState;
use crate::stream::StreamEvent;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

// Re-export the builder so `crate::graph::AgentGraphBuilder` still works.
pub use crate::builder::AgentGraphBuilder;

/// Virtual start node name.
pub const START: &str = "__start__";
/// Virtual end node name.
pub const END: &str = "__end__";

/// The agent graph — an orchestrator that owns control-flow and delegates
/// node work to the Payload layer.
pub struct AgentGraph {
    pub(crate) nodes: HashMap<String, Arc<dyn Node>>,
    pub(crate) edges: HashMap<String, Vec<EdgeType>>,
    pub(crate) max_iterations: usize,
    pub(crate) enable_cycle_detection: bool,
    pub(crate) retry_policies: HashMap<String, RetryPolicy>,
    pub(crate) interrupt_config: Option<InterruptConfig>,
    // Legacy checkpointer (superstep-level)
    pub(crate) checkpointer: Option<Arc<dyn CheckpointSaver>>,
    // New granular checkpoint store (per-attempt)
    pub(crate) checkpoint_store: Option<Arc<dyn CheckpointStore>>,
    pub(crate) reducers: Vec<(String, Arc<dyn Reducer>)>,
    pub(crate) graph_name: Option<String>,
    // New abstractions
    pub(crate) event_sink: Option<Arc<dyn EventSink>>,
    pub(crate) executor: Option<Arc<dyn Executor>>,
}

impl AgentGraph {
    /// Create a new graph builder.
    pub fn builder() -> AgentGraphBuilder {
        AgentGraphBuilder::new()
    }

    // ── Internal helpers used by both graph.rs and engine.rs ──

    pub(crate) async fn register_reducers_on_state(&self, state: &AgentState) {
        let mut state_reducers = state.reducers.write().await;
        for (key, reducer) in &self.reducers {
            state_reducers.insert(key.clone(), reducer.clone());
        }
    }

    /// Resolve the event sink to use for this execution.
    pub(crate) fn resolve_event_sink(
        &self,
        stream_tx: Option<mpsc::Sender<StreamEvent>>,
    ) -> Arc<dyn EventSink> {
        if let Some(tx) = stream_tx {
            // Streaming path: wrap the channel
            if let Some(ref configured_sink) = self.event_sink {
                // Both configured sink and channel: composite
                Arc::new(crate::event_sink::CompositeEventSink::new(vec![
                    configured_sink.clone(),
                    Arc::new(ChannelEventSink::new(tx)),
                ]))
            } else {
                Arc::new(ChannelEventSink::new(tx))
            }
        } else if let Some(ref sink) = self.event_sink {
            sink.clone()
        } else {
            Arc::new(NoopEventSink)
        }
    }

    /// Create a run ID from a configured checkpoint store, or locally when no store is configured.
    ///
    /// A configured store is a durable-execution contract: its creation failure
    /// is returned to the caller rather than silently degrading to a UUID.
    pub(crate) async fn create_run_id(&self) -> Result<String> {
        if let Some(ref store) = self.checkpoint_store {
            let name = self.graph_name.as_deref().unwrap_or("unnamed");
            store
                .create_run(name)
                .await
                .map_err(|error| AgentGraphError::CheckpointStore {
                    operation: crate::error::CheckpointStoreOperation::CreateRun,
                    message: error.to_string(),
                })
        } else {
            Ok(stack_ids::GraphRunId::random("agent-graph").to_string())
        }
    }

    /// Resume execution from an interrupt checkpoint.
    ///
    /// Validates that the graph topology hasn't changed since the checkpoint
    /// was taken. Returns `CheckpointMismatch` if the graph hash differs.
    /// Use [`Self::resume_force`] to skip this check.
    pub async fn resume(
        &self,
        state: AgentState,
        config: GraphConfig,
        checkpoint: crate::interrupt::InterruptCheckpoint,
    ) -> Result<AgentState> {
        if let Some(ref saved_hash) = checkpoint.graph_hash {
            let current_hash = self.compute_graph_hash();
            if *saved_hash != current_hash {
                return Err(AgentGraphError::CheckpointMismatch {
                    expected: saved_hash.clone(),
                    actual: current_hash,
                });
            }
        }
        self.execute_with_config(&checkpoint.resume_node, state, config)
            .await
    }

    /// Resume execution from an interrupt checkpoint without validating graph topology.
    pub async fn resume_force(
        &self,
        state: AgentState,
        config: GraphConfig,
        checkpoint: crate::interrupt::InterruptCheckpoint,
    ) -> Result<AgentState> {
        self.execute_with_config(&checkpoint.resume_node, state, config)
            .await
    }

    /// Get current state from checkpointer.
    pub async fn get_state(&self, config: &GraphConfig) -> Result<Option<AgentState>> {
        if let (Some(checkpointer), Some(thread_id)) = (&self.checkpointer, &config.thread_id) {
            if let Some(cp) = checkpointer.load(thread_id).await? {
                let state = AgentState::new();
                state.restore(&cp.state).await;
                return Ok(Some(state));
            }
        }
        Ok(None)
    }

    /// Get checkpoint history for a thread.
    pub async fn get_state_history(&self, config: &GraphConfig) -> Result<Vec<Checkpoint>> {
        if let (Some(checkpointer), Some(thread_id)) = (&self.checkpointer, &config.thread_id) {
            return checkpointer.load_history(thread_id).await;
        }
        Ok(Vec::new())
    }

    /// Update state in the checkpointer (time travel).
    pub async fn update_state(
        &self,
        config: &GraphConfig,
        updates: HashMap<String, Value>,
    ) -> Result<()> {
        if let (Some(checkpointer), Some(thread_id)) = (&self.checkpointer, &config.thread_id) {
            if let Some(mut cp) = checkpointer.load(thread_id).await? {
                for (k, v) in updates {
                    cp.state.data.insert(k, v);
                }
                checkpointer.save(&cp).await?;
            }
        }
        Ok(())
    }

    /// Generate a Mermaid diagram of the graph structure.
    pub fn to_mermaid(&self) -> String {
        let mut lines = vec!["graph TD".to_string()];
        lines.push(format!("    {}([START])", START));
        lines.push(format!("    {}([END])", END));

        let mut node_names: Vec<&String> = self.nodes.keys().collect();
        node_names.sort();
        for name in &node_names {
            lines.push(format!("    {0}[{0}]", name));
        }

        let mut edge_sources: Vec<&String> = self.edges.keys().collect();
        edge_sources.sort();
        for from in edge_sources {
            if let Some(edge_list) = self.edges.get(from) {
                for edge in edge_list {
                    match edge {
                        EdgeType::Normal(to) => {
                            lines.push(format!("    {} --> {}", from, to));
                        }
                        EdgeType::Conditional(_) => {
                            lines.push(format!("    {} -.->|condition| ?", from));
                        }
                    }
                }
            }
        }

        lines.join("\n")
    }

    /// Get the graph name.
    pub fn name(&self) -> Option<&str> {
        self.graph_name.as_deref()
    }

    /// Get the node names.
    pub fn node_names(&self) -> Vec<&String> {
        self.nodes.keys().collect()
    }

    /// Get the edge map for inspection.
    pub fn edge_map(&self) -> &HashMap<String, Vec<EdgeType>> {
        &self.edges
    }

    /// Compute a stable hash of the graph's topology (node names + edges).
    ///
    /// Used to detect graph-definition drift when resuming from a checkpoint.
    /// Two graphs with the same nodes and edges produce the same hash.
    pub fn compute_graph_hash(&self) -> String {
        use std::collections::BTreeMap;
        use std::hash::{Hash, Hasher};

        let mut hasher = std::hash::DefaultHasher::new();

        // Sort node names for determinism
        let mut sorted_nodes: Vec<&String> = self.nodes.keys().collect();
        sorted_nodes.sort();
        for name in &sorted_nodes {
            name.hash(&mut hasher);
        }

        // Sort edges by source node for determinism
        let sorted_edges: BTreeMap<&String, &Vec<EdgeType>> = self.edges.iter().collect();
        for (from, edges) in &sorted_edges {
            from.hash(&mut hasher);
            for edge in *edges {
                match edge {
                    EdgeType::Normal(to) => {
                        "normal".hash(&mut hasher);
                        to.hash(&mut hasher);
                    }
                    EdgeType::Conditional(router) => {
                        "conditional".hash(&mut hasher);
                        from.hash(&mut hasher);
                        router.semantic_digest().hash(&mut hasher);
                    }
                }
            }
        }

        format!("{:016x}", hasher.finish())
    }
}

// Implement Node for AgentGraph to enable subgraph support.
#[async_trait::async_trait]
impl Node for AgentGraph {
    async fn execute(&self, state: &AgentState, config: &GraphConfig) -> Result<NodeOutput> {
        let subgraph_state = state.fork().await;

        let start = if self.edges.contains_key(START) {
            START
        } else {
            let all_targets: std::collections::HashSet<&str> = self
                .edges
                .values()
                .flat_map(|edges| {
                    edges.iter().filter_map(|e| match e {
                        EdgeType::Normal(to) => Some(to.as_str()),
                        _ => None,
                    })
                })
                .collect();
            let entry = self
                .nodes
                .keys()
                .find(|n| !all_targets.contains(n.as_str()))
                .ok_or_else(|| {
                    AgentGraphError::ExecutionError("Subgraph has no entry point".to_string())
                })?;
            entry.as_str()
        };

        let result = self
            .execute_with_config(start, subgraph_state, config.clone())
            .await?;

        let result_data = result.export().await;
        for (key, value) in result_data {
            state.set(&key, value).await?;
        }

        Ok(NodeOutput::Done)
    }

    fn name(&self) -> Option<&str> {
        self.graph_name.as_deref()
    }
}

impl std::fmt::Debug for AgentGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentGraph")
            .field("nodes", &self.nodes.keys().collect::<Vec<_>>())
            .field("edges", &format!("{} edge groups", self.edges.len()))
            .field("max_iterations", &self.max_iterations)
            .finish()
    }
}
