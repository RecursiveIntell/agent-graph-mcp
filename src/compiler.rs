use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64},
    Arc, Mutex,
};

use ri_agent_graph::event_sink::{EventSink, GraphEvent};
use ri_agent_graph::join::JoinNode;
use ri_agent_graph::reducer::{AddReducer, AppendReducer, LastWriteWins, MergeReducer};
use ri_agent_graph::retry::RetryPolicy;
use ri_agent_graph::AgentGraph;
use tokio::sync::Notify;

use crate::model_executor::ExecutorHandle;
use crate::nodes::{
    legacy_router, HumanApprovalNode, LlmNode, PassthroughNode, RouterConfig, RouterNode,
    RunContext, SubgraphExecutor, SubgraphNode, ToolNode, TransformConfig, TransformNode,
};
use crate::spec::{GraphSpec, NodeType, ReducerKind};
use serde_json::Value;

fn default_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_memory",
                "description": "Search semantic memory for facts, claims, and past work.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Search keywords or namespace: prefix"}
                    },
                    "required": ["query"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file. Returns content with line numbers.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File path to read"}
                    },
                    "required": ["path"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_codebase",
                "description": "Search codebase for files, types, or patterns.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Search pattern"},
                        "path": {"type": "string", "description": "Directory to search"}
                    },
                    "required": ["pattern"]
                }
            }
        }),
    ]
}

// ---------------------------------------------------------------------------
// Strategy join helpers (five-stage pipeline: validate -> normalize ->
// contradictions -> adjudicate -> certify). Self-contained so the daemon
// works against the published ri-agent-graph core. Every strategy emits a
// certification envelope; a `fail` certification surfaces as an execution
// error, quarantine/abstain complete visibly.
// ---------------------------------------------------------------------------

fn join_path_get<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn join_claim_of(value: &Value, claim_path: &str) -> Option<String> {
    match join_path_get(value, claim_path) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

fn join_envelope(
    join: &str,
    certification: &str,
    value: Value,
    contradictions: Vec<Value>,
    minority_report: Vec<Value>,
    notes: Vec<String>,
) -> Value {
    serde_json::json!({
        "join": join,
        "certification": certification,
        "value": value,
        "contradictions": contradictions,
        "minority_report": minority_report,
        "notes": notes,
    })
}

fn join_evidence_valid(value: &Value) -> Result<(), String> {
    let Some(entries) = value.get("evidence").and_then(Value::as_array) else {
        return Ok(());
    };
    for entry in entries {
        let witness_id = entry.get("witness_id").and_then(Value::as_str);
        let digest = entry.get("digest").and_then(Value::as_str);
        match (witness_id, digest) {
            (Some(w), Some(d)) if !w.is_empty() && !d.is_empty() => {}
            _ => return Err("evidence entry must carry non-empty witness_id and digest".into()),
        }
    }
    Ok(())
}

fn join_checks_valid(value: &Value) -> Result<(), String> {
    let Some(entries) = value.get("checks").and_then(Value::as_array) else {
        return Ok(());
    };
    if entries.is_empty() {
        return Err("checks must not be empty".into());
    }
    for entry in entries {
        let status = entry.get("status").and_then(Value::as_str);
        if status != Some("passed") {
            return Err(
                "every check must carry status \"passed\" (executed check receipts only)".into(),
            );
        }
    }
    Ok(())
}

fn strategy_join_dedupe(
    values: Vec<(String, Value)>,
    identity_path: Option<&str>,
) -> Result<Value, String> {
    if values.is_empty() {
        return Err("dedupe_by_identity requires at least one branch artifact".into());
    }
    if let Some(path) = identity_path {
        for (key, value) in &values {
            if join_path_get(value, path).is_none() {
                return Err(format!(
                    "dedupe_by_identity: artifact '{key}' is missing identity path '{path}'"
                ));
            }
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (key, value) in values {
        let identity = identity_path
            .and_then(|path| join_path_get(&value, path))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| key.clone());
        if seen.insert(identity) {
            out.push(value);
        }
    }
    Ok(join_envelope(
        "dedupe_by_identity",
        "pass",
        Value::Array(out),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
}

fn strategy_join_contradiction_matrix(
    values: Vec<(String, Value)>,
    scope_path: &str,
    claim_path: &str,
    time_path: &str,
    strict: bool,
) -> Result<Value, String> {
    if values.is_empty() {
        return Err("contradiction_matrix requires at least one branch artifact".into());
    }
    let mut contradictions = Vec::new();
    for left in 0..values.len() {
        for right in (left + 1)..values.len() {
            let (l_key, l_value) = &values[left];
            let (r_key, r_value) = &values[right];
            let (Some(l_scope), Some(r_scope)) = (
                join_path_get(l_value, scope_path),
                join_path_get(r_value, scope_path),
            ) else {
                continue;
            };
            let (Some(l_time), Some(r_time)) = (
                join_path_get(l_value, time_path),
                join_path_get(r_value, time_path),
            ) else {
                continue;
            };
            if l_scope != r_scope || l_time != r_time {
                continue;
            }
            let (Some(l_claim), Some(r_claim)) = (
                join_claim_of(l_value, claim_path),
                join_claim_of(r_value, claim_path),
            ) else {
                continue;
            };
            if l_claim == r_claim {
                continue;
            }
            contradictions.push(serde_json::json!({
                "left": l_key,
                "right": r_key,
                "scope": l_scope,
                "time": l_time,
                "left_claim": l_claim,
                "right_claim": r_claim,
            }));
        }
    }
    let values: Vec<Value> = values.into_iter().map(|(_, v)| v).collect();
    let mut notes = Vec::new();
    if !contradictions.is_empty() {
        notes.push(format!(
            "{} contradiction pair(s) exposed",
            contradictions.len()
        ));
    }
    let certification = if strict && !contradictions.is_empty() {
        notes.push("strict mode: quarantined on contradiction".into());
        "quarantine"
    } else {
        "pass"
    };
    Ok(join_envelope(
        "contradiction_matrix",
        certification,
        Value::Array(values),
        contradictions,
        Vec::new(),
        notes,
    ))
}

fn strategy_join_minority_report(
    values: Vec<(String, Value)>,
    dissent_path: &str,
) -> Result<Value, String> {
    if values.is_empty() {
        return Err("minority_report requires at least one branch artifact".into());
    }
    let mut majority = Vec::new();
    let mut minority = Vec::new();
    for (_, value) in values {
        let dissent = join_path_get(&value, dissent_path)
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if dissent {
            minority.push(value);
        } else {
            majority.push(value);
        }
    }
    let mut notes = Vec::new();
    if !minority.is_empty() {
        notes.push(format!(
            "{} dissenting artifact(s) preserved in minority report",
            minority.len()
        ));
    }
    Ok(join_envelope(
        "minority_report",
        "pass",
        Value::Array(majority),
        Vec::new(),
        minority,
        notes,
    ))
}

fn strategy_join_proof_carrying(
    values: Vec<(String, Value)>,
    required_fields: &[String],
) -> Result<Value, String> {
    if values.is_empty() {
        return Err("proof_carrying_join requires at least one branch artifact".into());
    }
    let mut valid = Vec::new();
    let mut notes = Vec::new();
    let mut invalid = 0usize;
    for (key, value) in values {
        let mut reasons = Vec::new();
        for field in required_fields {
            if join_path_get(&value, field).is_none() {
                reasons.push(format!("missing required field '{field}'"));
            }
        }
        if let Err(e) = join_evidence_valid(&value) {
            reasons.push(e);
        }
        if let Err(e) = join_checks_valid(&value) {
            reasons.push(e);
        }
        if reasons.is_empty() {
            valid.push(value);
        } else {
            invalid += 1;
            notes.push(format!(
                "artifact '{key}' quarantined: {}",
                reasons.join("; ")
            ));
        }
    }
    let certification = if invalid == 0 { "pass" } else { "quarantine" };
    Ok(join_envelope(
        "proof_carrying_join",
        certification,
        Value::Array(valid),
        Vec::new(),
        Vec::new(),
        notes,
    ))
}

pub struct CompileContext {
    pub base_url: String,
    pub default_model: String,
    pub cancelled: Arc<AtomicBool>,
    pub cancellation: Arc<Notify>,
    pub events: Arc<Mutex<Vec<GraphEvent>>>,
    pub llm_calls: Arc<AtomicU64>,
    pub max_llm_calls: Option<u64>,
    pub llm_invocations: Arc<Mutex<Vec<Value>>>,
    /// Provider API key for http(s) llm-pipeline calls (Bearer header).
    pub api_key: Option<String>,
    /// Optional model invocation executor for local backends (proveKV).
    pub executor: ExecutorHandle,
    /// Optional subgraph executor provided by the host (server/run manager).
    pub subgraph: Option<SubgraphExecutor>,
    /// Shared subgraph nesting depth counter.
    pub subgraph_depth: Arc<AtomicU32>,
    /// Maximum allowed subgraph nesting depth.
    pub subgraph_limit: u32,
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
        llm_calls: cx.llm_calls,
        max_llm_calls: cx.max_llm_calls,
        llm_invocations: cx.llm_invocations,
        executor: cx.executor.clone(),
        subgraph: cx.subgraph.clone(),
        subgraph_depth: cx.subgraph_depth.clone(),
        subgraph_limit: cx.subgraph_limit,
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
                let context_file = node
                    .config
                    .get("context_file")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                let tools: Vec<serde_json::Value> = node
                    .config
                    .get("tools")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.clone())
                    .unwrap_or_default();
                Box::new(LlmNode {
                    id: node.id.clone(),
                    base_url: cx.base_url.clone(),
                    default_model: cx.default_model.clone(),
                    api_key: cx.api_key.clone(),
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
                    context_file,
                    tools,
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
                    "collect_object" => Box::new(JoinNode::new(inputs, output, |values| {
                        let obj: serde_json::Map<String, serde_json::Value> =
                            values.into_iter().map(|(k, v)| (k, v)).collect();
                        Ok(serde_json::Value::Object(obj))
                    })),
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
                    "dedupe_by_identity" => {
                        let identity_path = node
                            .config
                            .get("identity_path")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                        Box::new(JoinNode::new(inputs, output, move |values| {
                            strategy_join_dedupe(values, identity_path.as_deref())
                                .map_err(ri_agent_graph::AgentGraphError::ExecutionError)
                        }))
                    }
                    "contradiction_matrix" => {
                        let scope_path = node
                            .config
                            .get("scope_path")
                            .and_then(Value::as_str)
                            .unwrap_or("scope")
                            .to_owned();
                        let claim_path = node
                            .config
                            .get("claim_path")
                            .and_then(Value::as_str)
                            .unwrap_or("claim")
                            .to_owned();
                        let time_path = node
                            .config
                            .get("time_path")
                            .and_then(Value::as_str)
                            .unwrap_or("time")
                            .to_owned();
                        let strict = node
                            .config
                            .get("strict")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        Box::new(JoinNode::new(inputs, output, move |values| {
                            strategy_join_contradiction_matrix(
                                values,
                                &scope_path,
                                &claim_path,
                                &time_path,
                                strict,
                            )
                            .map_err(ri_agent_graph::AgentGraphError::ExecutionError)
                        }))
                    }
                    "minority_report" => {
                        let dissent_path = node
                            .config
                            .get("dissent_path")
                            .and_then(Value::as_str)
                            .unwrap_or("dissent")
                            .to_owned();
                        Box::new(JoinNode::new(inputs, output, move |values| {
                            strategy_join_minority_report(values, &dissent_path)
                                .map_err(ri_agent_graph::AgentGraphError::ExecutionError)
                        }))
                    }
                    "proof_carrying_join" => {
                        let required_fields = node
                            .config
                            .get("required_fields")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_owned)
                                    .collect::<Vec<_>>()
                            })
                            .filter(|fields| !fields.is_empty())
                            .unwrap_or_else(|| {
                                vec!["evidence".into(), "checks".into(), "receipt".into()]
                            });
                        Box::new(JoinNode::new(inputs, output, move |values| {
                            strategy_join_proof_carrying(values, &required_fields)
                                .map_err(ri_agent_graph::AgentGraphError::ExecutionError)
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
                let graph_name = node
                    .config
                    .get("graph_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
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
                    .unwrap_or("__subgraph_output__")
                    .to_owned();
                Box::new(SubgraphNode {
                    graph_name,
                    input_key,
                    output_key,
                    ctx: run.clone(),
                })
            }
            NodeType::Loop => {
                let entry = node
                    .config
                    .get("entry")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let exit = node
                    .config
                    .get("exit")
                    .and_then(|v| v.as_str())
                    .unwrap_or("END")
                    .to_owned();
                let max_iterations = node
                    .config
                    .get("max_iterations")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                let counter_key = node
                    .config
                    .get("counter_key")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("__loop__:{}", node.id));
                Box::new(crate::nodes::LoopNode {
                    entry,
                    exit,
                    max_iterations,
                    counter_key,
                    ctx: run.clone(),
                })
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
            NodeType::External => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ri_agent_graph::command::{Navigation, NodeOutput};
    use ri_agent_graph::config::GraphConfig;
    use ri_agent_graph::node::Node;
    use ri_agent_graph::state::AgentState;
    use serde_json::json;

    fn artifact(key: &str, value: Value) -> (String, Value) {
        (key.to_owned(), value)
    }

    #[test]
    fn dedupe_collapses_duplicate_identities() {
        let values = vec![
            artifact("a", json!({"claim": {"id": "C1"}, "finding": "x"})),
            artifact("b", json!({"claim": {"id": "C1"}, "finding": "x"})),
            artifact("c", json!({"claim": {"id": "C2"}, "finding": "y"})),
        ];
        let envelope = strategy_join_dedupe(values, Some("claim.id")).unwrap();
        assert_eq!(envelope["join"], "dedupe_by_identity");
        assert_eq!(envelope["certification"], "pass");
        assert_eq!(envelope["value"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn dedupe_rejects_missing_identity_path() {
        let values = vec![artifact("a", json!({"finding": "x"}))];
        let err = strategy_join_dedupe(values, Some("claim.id")).unwrap_err();
        assert!(err.contains("missing identity path"));
    }

    #[test]
    fn contradiction_matrix_exposes_and_quarantines_when_strict() {
        let values = vec![
            artifact(
                "a",
                json!({"scope": "s1", "time": "2026-08-06", "claim": "true"}),
            ),
            artifact(
                "b",
                json!({"scope": "s1", "time": "2026-08-06", "claim": "false"}),
            ),
        ];
        let envelope =
            strategy_join_contradiction_matrix(values, "scope", "claim", "time", true).unwrap();
        assert_eq!(envelope["certification"], "quarantine");
        assert_eq!(envelope["contradictions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn contradiction_matrix_temporal_mismatch_is_not_contradictory() {
        let values = vec![
            artifact(
                "a",
                json!({"scope": "s1", "time": "2026-08-06", "claim": "true"}),
            ),
            artifact(
                "b",
                json!({"scope": "s1", "time": "2026-08-07", "claim": "false"}),
            ),
        ];
        let envelope =
            strategy_join_contradiction_matrix(values, "scope", "claim", "time", true).unwrap();
        assert_eq!(envelope["certification"], "pass");
        assert!(envelope["contradictions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn minority_report_preserves_dissent() {
        let values = vec![
            artifact("a", json!({"dissent": false, "claim": "x"})),
            artifact("b", json!({"dissent": true, "claim": "y"})),
        ];
        let envelope = strategy_join_minority_report(values, "dissent").unwrap();
        assert_eq!(envelope["value"].as_array().unwrap().len(), 1);
        assert_eq!(envelope["minority_report"].as_array().unwrap().len(), 1);
        assert_eq!(envelope["minority_report"][0]["claim"], "y");
    }

    #[test]
    fn proof_carrying_passes_valid_and_quarantines_invalid_evidence() {
        let valid = vec![artifact(
            "a",
            json!({
                "evidence": [{"witness_id": "w1", "digest": "sha256:abc"}],
                "checks": [{"status": "passed"}],
                "receipt": "receipt:r1",
            }),
        )];
        let envelope = strategy_join_proof_carrying(
            valid,
            &["evidence".into(), "checks".into(), "receipt".into()],
        )
        .unwrap();
        assert_eq!(envelope["certification"], "pass");

        let invalid = vec![artifact(
            "b",
            json!({
                "evidence": [{"locator": "https://example.com/x"}],
                "checks": [{"status": "passed"}],
                "receipt": "receipt:r2",
            }),
        )];
        let envelope = strategy_join_proof_carrying(
            invalid,
            &["evidence".into(), "checks".into(), "receipt".into()],
        )
        .unwrap();
        assert_eq!(envelope["certification"], "quarantine");
        assert!(envelope["value"].as_array().unwrap().is_empty());
        assert!(envelope["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("quarantined")));
    }

    #[test]
    fn strategy_joins_reject_empty_inputs() {
        assert!(strategy_join_dedupe(Vec::new(), None).is_err());
        assert!(strategy_join_contradiction_matrix(Vec::new(), "s", "c", "t", false).is_err());
        assert!(strategy_join_minority_report(Vec::new(), "dissent").is_err());
        assert!(strategy_join_proof_carrying(Vec::new(), &[]).is_err());
    }

    fn test_run_context() -> RunContext {
        RunContext {
            cancelled: Arc::new(AtomicBool::new(false)),
            cancellation: Arc::new(tokio::sync::Notify::new()),
            llm_calls: Arc::new(AtomicU64::new(0)),
            max_llm_calls: None,
            llm_invocations: Arc::new(Mutex::new(Vec::new())),
            executor: crate::model_executor::ExecutorHandle::none(),
            subgraph: None,
            subgraph_depth: Arc::new(AtomicU32::new(0)),
            subgraph_limit: 4,
        }
    }

    fn test_state(pairs: Vec<(&str, Value)>) -> AgentState {
        AgentState::with_data_and_limits(
            pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
            ri_agent_graph::state::StateLimits {
                max_keys: 1000,
                max_value_bytes: 256 * 1024,
                max_history_len: 100,
                lock_timeout: std::time::Duration::from_secs(5),
            },
        )
    }

    #[test]
    fn loop_node_navigates_entry_until_exhausted() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let node = crate::nodes::LoopNode {
            entry: "body".into(),
            exit: "END".into(),
            max_iterations: 2,
            counter_key: "__loop__:lp".into(),
            ctx: test_run_context(),
        };
        // First pass: counter 0 -> navigate to entry, counter incremented.
        let out = rt
            .block_on(node.execute(&test_state(vec![]), &GraphConfig::default()))
            .unwrap();
        let ri_agent_graph::command::NodeOutput::Command(cmd) = out else {
            panic!("expected Command");
        };
        assert!(matches!(cmd.goto, Navigation::Node(n) if n == "body"));
        assert_eq!(cmd.update.unwrap()["__loop__:lp"], 1);
        // Second pass: counter 2 == max -> navigate END.
        let state = test_state(vec![("__loop__:lp", json!(2))]);
        let out = rt
            .block_on(node.execute(&state, &GraphConfig::default()))
            .unwrap();
        let ri_agent_graph::command::NodeOutput::Command(cmd) = out else {
            panic!("expected Command");
        };
        assert!(matches!(cmd.goto, Navigation::End));
    }

    #[test]
    fn loop_node_exit_targets_named_node() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let node = crate::nodes::LoopNode {
            entry: "body".into(),
            exit: "synthesis".into(),
            max_iterations: 1,
            counter_key: "__loop__:lp".into(),
            ctx: test_run_context(),
        };
        let state = test_state(vec![("__loop__:lp", json!(1))]);
        let out = rt
            .block_on(node.execute(&state, &GraphConfig::default()))
            .unwrap();
        let ri_agent_graph::command::NodeOutput::Command(cmd) = out else {
            panic!("expected Command");
        };
        assert!(matches!(cmd.goto, Navigation::Node(n) if n == "synthesis"));
    }

    #[test]
    fn subgraph_node_writes_runner_output_and_returns_done() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let runner = crate::nodes::SubgraphExecutor {
            inner: Arc::new(|name: String, input: Value, _depth: u32| {
                Box::pin(async move {
                    assert_eq!(name, "inner-graph");
                    assert_eq!(input, json!({"topic": "x"}));
                    Ok(json!({"answer": "42"}))
                })
            }),
        };
        let mut ctx = test_run_context();
        ctx.subgraph = Some(runner);
        let node = crate::nodes::SubgraphNode {
            graph_name: "inner-graph".into(),
            input_key: "__input__".into(),
            output_key: "sub_result".into(),
            ctx,
        };
        let state = test_state(vec![("__input__", json!({"topic": "x"}))]);
        let out = rt
            .block_on(node.execute(&state, &GraphConfig::default()))
            .unwrap();
        assert!(matches!(out, ri_agent_graph::command::NodeOutput::Done));
        let value = rt.block_on(state.get_opt::<Value>("sub_result")).unwrap();
        assert_eq!(value, Some(json!({"answer": "42"})));
    }

    #[test]
    fn subgraph_node_fails_without_executor() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let node = crate::nodes::SubgraphNode {
            graph_name: "inner-graph".into(),
            input_key: "__input__".into(),
            output_key: "sub_result".into(),
            ctx: test_run_context(),
        };
        let err = rt
            .block_on(node.execute(&test_state(vec![]), &GraphConfig::default()))
            .unwrap_err();
        assert!(err.to_string().contains("subgraph executor unavailable"));
    }

    #[test]
    fn loop_spec_validation_enforces_contract() {
        use crate::spec::{validate, GraphSpec};
        let base = serde_json::from_value::<GraphSpec>(json!({
            "spec_version": "2",
            "name": "loop-test",
            "entry": "lp",
            "nodes": [
                {"id": "lp", "type": "loop", "config": {"entry": "body", "exit": "END", "max_iterations": 3}},
                {"id": "body", "type": "state_transform", "config": {"operations": [{"op": "set", "path": "x", "value": 1}]}}
            ],
            "edges": [{"from": "lp", "to": "body"}, {"from": "body", "to": "lp"}],
            "reducers": {}
        }))
        .unwrap();
        assert!(validate(&base).is_ok(), "valid loop spec rejected");
        let bad = serde_json::from_value::<GraphSpec>(json!({
            "spec_version": "2",
            "name": "loop-test-bad",
            "entry": "lp",
            "nodes": [
                {"id": "lp", "type": "loop", "config": {"entry": "missing", "exit": "END", "max_iterations": 3}}
            ],
            "edges": [],
            "reducers": {}
        }))
        .unwrap();
        let err = validate(&bad).unwrap_err();
        assert!(err.contains("loop 'lp' requires config.entry"));
        let bad_iter = serde_json::from_value::<GraphSpec>(json!({
            "spec_version": "2",
            "name": "loop-test-bad-iter",
            "entry": "lp",
            "nodes": [
                {"id": "lp", "type": "loop", "config": {"entry": "body", "exit": "END", "max_iterations": 0}},
                {"id": "body", "type": "passthrough"}
            ],
            "edges": [],
            "reducers": {}
        }))
        .unwrap();
        let err = validate(&bad_iter).unwrap_err();
        assert!(err.contains("max_iterations in 1..=32"));
    }
}
