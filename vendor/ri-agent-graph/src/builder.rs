#![allow(deprecated)] // Constructs GraphEvent with legacy trace_id/attempt fields during migration

use crate::checkpoint_store::CheckpointStore;
use crate::checkpointer::CheckpointSaver;
use crate::edge::EdgeType;
use crate::error::{AgentGraphError, Result};
use crate::event_sink::EventSink;
use crate::executor::Executor;
use crate::graph::AgentGraph;
use crate::graph::{END, START};
use crate::interrupt::InterruptConfig;
use crate::node::Node;
use crate::reducer::Reducer;
use crate::retry::RetryPolicy;
use crate::router::RoutingFunction;
use std::collections::HashMap;
use std::sync::Arc;

/// Builder for AgentGraph.
pub struct AgentGraphBuilder {
    pub(crate) nodes: HashMap<String, Arc<dyn Node>>,
    pub(crate) edges: HashMap<String, Vec<EdgeType>>,
    pub(crate) max_iterations: usize,
    pub(crate) enable_cycle_detection: bool,
    pub(crate) retry_policies: HashMap<String, RetryPolicy>,
    pub(crate) interrupt_config: Option<InterruptConfig>,
    pub(crate) checkpointer: Option<Arc<dyn CheckpointSaver>>,
    pub(crate) checkpoint_store: Option<Arc<dyn CheckpointStore>>,
    pub(crate) reducers: Vec<(String, Arc<dyn Reducer>)>,
    pub(crate) graph_name: Option<String>,
    pub(crate) event_sink: Option<Arc<dyn EventSink>>,
    pub(crate) executor: Option<Arc<dyn Executor>>,
}

impl AgentGraphBuilder {
    pub(crate) fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            max_iterations: 100,
            enable_cycle_detection: true,
            retry_policies: HashMap::new(),
            interrupt_config: None,
            checkpointer: None,
            checkpoint_store: None,
            reducers: Vec::new(),
            graph_name: None,
            event_sink: None,
            executor: None,
        }
    }

    /// Set the graph name (used in streaming events and debugging).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.graph_name = Some(name.into());
        self
    }

    /// Add a node to the graph.
    pub fn add_node(mut self, name: impl Into<String>, node: Box<dyn Node>) -> Self {
        self.nodes.insert(name.into(), Arc::from(node));
        self
    }

    /// Add a node with a retry policy.
    pub fn add_node_with_retry(
        mut self,
        name: impl Into<String>,
        node: Box<dyn Node>,
        retry: RetryPolicy,
    ) -> Self {
        let name = name.into();
        self.nodes.insert(name.clone(), Arc::from(node));
        self.retry_policies.insert(name, retry);
        self
    }

    /// Add a subgraph as a node.
    pub fn add_subgraph(mut self, name: impl Into<String>, subgraph: AgentGraph) -> Self {
        self.nodes.insert(name.into(), Arc::new(subgraph));
        self
    }

    /// Add a normal edge (always goes to next node).
    /// Multiple edges from the same node create fan-out (parallel execution).
    pub fn add_edge(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        let from = from.into();
        let to = to.into();
        self.edges
            .entry(from)
            .or_default()
            .push(EdgeType::Normal(to));
        self
    }

    /// Add a conditional edge (uses router to determine next node).
    pub fn add_conditional_edge(
        mut self,
        from: impl Into<String>,
        router: Box<dyn RoutingFunction>,
    ) -> Self {
        let from = from.into();
        self.edges
            .entry(from)
            .or_default()
            .push(EdgeType::Conditional(router));
        self
    }

    /// Set the entry point (sugar for add_edge(START, node)).
    pub fn set_entry_point(self, node: impl Into<String>) -> Self {
        self.add_edge(START, node)
    }

    /// Set the finish point (sugar for add_edge(node, END)).
    pub fn set_finish_point(self, node: impl Into<String>) -> Self {
        self.add_edge(node, END)
    }

    /// Set maximum iterations before stopping (prevents infinite loops).
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Enable or disable cycle detection.
    pub fn with_cycle_detection(mut self, enable: bool) -> Self {
        self.enable_cycle_detection = enable;
        self
    }

    /// Register a state reducer for a key.
    pub fn with_reducer(mut self, key: impl Into<String>, reducer: impl Reducer + 'static) -> Self {
        self.reducers.push((key.into(), Arc::new(reducer)));
        self
    }

    /// Set interrupt-before configuration.
    pub fn with_interrupt_before(mut self, nodes: Vec<String>) -> Self {
        let cfg = self
            .interrupt_config
            .get_or_insert_with(InterruptConfig::new);
        cfg.interrupt_before.extend(nodes);
        self
    }

    /// Set interrupt-after configuration.
    pub fn with_interrupt_after(mut self, nodes: Vec<String>) -> Self {
        let cfg = self
            .interrupt_config
            .get_or_insert_with(InterruptConfig::new);
        cfg.interrupt_after.extend(nodes);
        self
    }

    /// Set the legacy checkpointer for persistence (superstep-level).
    pub fn with_checkpointer(mut self, checkpointer: impl CheckpointSaver + 'static) -> Self {
        self.checkpointer = Some(Arc::new(checkpointer));
        self
    }

    /// Set the granular checkpoint store (per-attempt recording).
    pub fn with_checkpoint_store(mut self, store: Arc<dyn CheckpointStore>) -> Self {
        self.checkpoint_store = Some(store);
        self
    }

    /// Set a custom event sink for structured event handling.
    pub fn with_event_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.event_sink = Some(sink);
        self
    }

    /// Set a custom executor for node execution.
    pub fn with_executor(mut self, executor: Arc<dyn Executor>) -> Self {
        self.executor = Some(executor);
        self
    }

    /// Build the graph.
    pub fn build(self) -> Result<AgentGraph> {
        for (from, edge_list) in &self.edges {
            for edge in edge_list {
                if let EdgeType::Normal(to) = edge {
                    if to != END && !self.nodes.contains_key(to) && to != START {
                        return Err(AgentGraphError::NodeNotFound(format!(
                            "Edge from '{}' points to non-existent node '{}'",
                            from, to
                        )));
                    }
                }
            }
        }

        Ok(AgentGraph {
            nodes: self.nodes,
            edges: self.edges,
            max_iterations: self.max_iterations,
            enable_cycle_detection: self.enable_cycle_detection,
            retry_policies: self.retry_policies,
            interrupt_config: self.interrupt_config,
            checkpointer: self.checkpointer,
            checkpoint_store: self.checkpoint_store,
            reducers: self.reducers,
            graph_name: self.graph_name,
            event_sink: self.event_sink,
            executor: self.executor,
        })
    }
}
