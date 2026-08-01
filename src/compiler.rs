use std::sync::{atomic::AtomicBool, Arc, Mutex};

use ri_agent_graph::event_sink::{EventSink, GraphEvent};
use ri_agent_graph::join::JoinNode;
use ri_agent_graph::reducer::{AddReducer, AppendReducer, LastWriteWins, MergeReducer};
use ri_agent_graph::retry::RetryPolicy;
use ri_agent_graph::AgentGraph;
use tokio::sync::Notify;

use crate::nodes::{
    legacy_router, HumanApprovalNode, LlmNode, PassthroughNode, RouterConfig, RouterNode,
    RunContext, ToolNode, TransformConfig, TransformNode,
};
use crate::spec::{GraphSpec, NodeType, ReducerKind};
use serde_json::Value;

pub struct CompileContext {
    pub base_url: String,
    pub default_model: String,
    pub cancelled: Arc<AtomicBool>,
    pub cancellation: Arc<Notify>,
    pub events: Arc<Mutex<Vec<GraphEvent>>>,
}

struct Collector(Arc<Mutex<Vec<GraphEvent>>>);
impl EventSink for Collector {
    fn emit(&self, event: GraphEvent) {
        if let Ok(mut events) = self.0.lock() {
            if events.len() < 2048 {
                events.push(event);
            }
        }
    }
}

pub fn compile(spec: &GraphSpec, cx: CompileContext) -> Result<AgentGraph, String> {
    let run = RunContext {
        cancelled: cx.cancelled,
        cancellation: cx.cancellation,
    };
    let mut builder = AgentGraph::builder()
        .with_name(&spec.name)
        .with_max_iterations(spec.max_iterations.unwrap_or(64))
        .with_cycle_detection(false)
        .with_event_sink(Arc::new(Collector(cx.events)));
    for node in &spec.nodes {
        GraphSpec::executable_node_type(&node.node_type)
            .map_err(|error| format!("node '{}': {error}", node.id))?;
        let boxed: Box<dyn ri_agent_graph::node::Node> = match node.node_type {
            NodeType::Passthrough => Box::new(PassthroughNode { ctx: run.clone() }),
            NodeType::Llm => {
                let input_key = node
                    .config
                    .get("input_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__input__")
                    .to_owned();
                let output_key = node
                    .config
                    .get("output_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__input__")
                    .to_owned();
                Box::new(LlmNode {
                    id: node.id.clone(),
                    base_url: cx.base_url.clone(),
                    default_model: cx.default_model.clone(),
                    prompt: node
                        .prompt
                        .clone()
                        .or_else(|| {
                            node.config
                                .get("prompt")
                                .and_then(|v| v.as_str())
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| "{input}".into()),
                    model: node.model.clone(),
                    json_mode: node.json_mode,
                    evidence_required: node.evidence_required,
                    max_tokens: node.max_tokens,
                    timeout_ms: node
                        .config
                        .get("timeout_ms")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(120_000),
                    input_key,
                    output_key,
                    ctx: run.clone(),
                })
            }
            NodeType::StateTransform => Box::new(TransformNode {
                config: serde_json::from_value::<TransformConfig>(node.config.clone())
                    .map_err(|e| format!("node '{}': {e}", node.id))?,
                ctx: run.clone(),
            }),
            NodeType::Router => {
                let config = if let Some(routes) = &node.routes {
                    legacy_router(routes)
                } else {
                    serde_json::from_value::<RouterConfig>(node.config.clone())
                        .map_err(|e| format!("node '{}': {e}", node.id))?
                };
                Box::new(RouterNode {
                    config,
                    ctx: run.clone(),
                })
            }
            NodeType::Join => {
                let inputs = node
                    .config
                    .get("inputs")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| format!("join '{}' requires inputs", node.id))?
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect();
                let output = node
                    .config
                    .get("output")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("join '{}' requires output", node.id))?;
                match node
                    .config
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("collect_array")
                {
                    "collect_array" => Box::new(JoinNode::collect_array(inputs, output)),
                    "merge_objects" => Box::new(JoinNode::merge_objects(inputs, output)),
                    "first_non_null" => Box::new(JoinNode::new(inputs, output, |values| {
                        Ok(values
                            .into_iter()
                            .map(|(_, v)| v)
                            .find(|v| !v.is_null())
                            .unwrap_or(serde_json::Value::Null))
                    })),
                    "all_success" => Box::new(JoinNode::new(inputs, output, |values| {
                        let all = values.iter().all(|(_, value)| {
                            value.as_bool().unwrap_or_else(|| {
                                value
                                    .get("success")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false)
                            })
                        });
                        Ok(
                            serde_json::json!({"all_success": all, "values": values.into_iter().map(|(_, value)| value).collect::<Vec<_>>() }),
                        )
                    })),
                    "quorum" => {
                        let required = node
                            .config
                            .get("required")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(1) as usize;
                        Box::new(JoinNode::new(inputs, output, move |values| {
                            let approvals = values
                                .iter()
                                .filter(|(_, value)| value.as_bool().unwrap_or(false))
                                .count();
                            Ok(
                                serde_json::json!({"met": approvals >= required, "approvals": approvals, "required": required}),
                            )
                        }))
                    }
                    mode => return Err(format!("unsupported join mode '{mode}'")),
                }
            }
            NodeType::Parallel => {
                // Parallel node: compile branches as passthrough nodes that fan out.
                // The engine handles parallel execution when multiple nodes are targets
                // from the same source in a superstep. We create a passthrough here
                // and rely on edge routing to fan out to individual branch entries.
                let _branches = node
                    .config
                    .get("branches")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len())
                    .unwrap_or(0);
                // Write branch metadata to state for introspection
                Box::new(PassthroughNode { ctx: run.clone() })
            }
            NodeType::Subgraph => {
                // Subgraph: the referenced graph must be registered separately.
                // We create a passthrough that records the intent; actual subgraph
                // embedding requires cross-graph lookup at execution time.
                let _graph_name = node
                    .config
                    .get("graph_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                Box::new(PassthroughNode { ctx: run.clone() })
            }
            NodeType::HumanApproval => {
                // Human approval: emit interrupt signal to state.
                // The caller (Hermes) monitors for InterruptError and handles the
                // approval lifecycle via graph_resume.
                let prompt_key = node
                    .config
                    .get("prompt_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__approval_prompt__")
                    .to_owned();
                let output_key = node
                    .config
                    .get("output_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__approval_decision__")
                    .to_owned();
                let audience: Vec<String> = node
                    .config
                    .get("audience")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let allowed: Vec<String> = node
                    .config
                    .get("allowed_decisions")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["approve".into(), "reject".into()]);
                let expiry_ms = node
                    .config
                    .get("expiry_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(300_000);

                Box::new(HumanApprovalNode {
                    prompt_key,
                    output_key,
                    audience,
                    allowed_decisions: allowed,
                    expiry_ms,
                    ctx: run.clone(),
                })
            }
            NodeType::External | NodeType::Loop => {
                return Err(format!(
                    "node '{}' is not executable by this local runtime",
                    node.id
                ));
            }
            NodeType::Tool => {
                let python = node
                    .config
                    .get("python")
                    .and_then(|v| v.as_str())
                    .unwrap_or("python3")
                    .to_owned();
                let hermes_source = node
                    .config
                    .get("hermes_source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/home/sikmindz/.hermes/hermes-agent")
                    .to_owned();
                let lease = node.config.get("lease").cloned().unwrap_or(Value::Null);
                let receipt_dir = node
                    .config
                    .get("receipt_dir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/tmp/agent-graph-tool-receipts")
                    .to_owned();
                let timeout_ms = node
                    .config
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(120_000);
                Box::new(ToolNode {
                    id: node.id.clone(),
                    python,
                    hermes_source,
                    lease,
                    receipt_dir,
                    timeout_ms,
                    ctx: run.clone(),
                })
            }
        };
        if let Some(retry) = node.config.get("retry") {
            let attempts = retry
                .get("max_attempts")
                .and_then(|v| v.as_u64())
                .unwrap_or(3) as usize;
            let initial = retry
                .get("initial_delay_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(250);
            let max_delay = retry
                .get("max_delay_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(5_000);
            let policy = RetryPolicy::new()
                .with_max_attempts(attempts)
                .with_initial_interval(std::time::Duration::from_millis(initial))
                .with_max_interval(std::time::Duration::from_millis(max_delay))
                .with_backoff_factor(
                    retry
                        .get("backoff_factor")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(2.0),
                )
                .with_jitter(
                    retry
                        .get("jitter")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                );
            builder = builder.add_node_with_retry(&node.id, boxed, policy);
        } else {
            builder = builder.add_node(&node.id, boxed);
        }
    }
    builder = builder.set_entry_point(&spec.entry);
    for edge in &spec.edges {
        let target = if edge.to == "END" {
            ri_agent_graph::END
        } else {
            edge.to.as_str()
        };
        builder = builder.add_edge(&edge.from, target);
    }
    for (key, reducer) in &spec.reducers {
        builder = match reducer {
            ReducerKind::LastWriteWins => builder.with_reducer(key, LastWriteWins),
            ReducerKind::Append => builder.with_reducer(key, AppendReducer),
            ReducerKind::Add => builder.with_reducer(key, AddReducer),
            ReducerKind::Merge => builder.with_reducer(key, MergeReducer),
        };
    }
    builder.build().map_err(|e| e.to_string())
}
