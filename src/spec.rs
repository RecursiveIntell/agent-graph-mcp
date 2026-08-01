use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Historical default for registered graphs when no explicit capacity is configured.
pub const DEFAULT_MAX_GRAPHS: usize = 64;
/// Absolute bound for a single daemon instance. This prevents a configuration typo
/// from silently admitting an unbounded graph registry.
pub const MAX_GRAPHS_HARD_LIMIT: usize = 1024;
pub const MAX_GRAPH_BYTES: usize = 64 * 1024;
pub const MAX_NODES: usize = 128;
pub const MAX_EDGES: usize = 512;
pub const MAX_ITERATIONS: usize = 64;
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 128 * 1024;
pub const MAX_STATE_BYTES: usize = 2 * 1024 * 1024;

pub fn validate_max_graphs(max_graphs: usize) -> Result<usize, String> {
    if (1..=MAX_GRAPHS_HARD_LIMIT).contains(&max_graphs) {
        Ok(max_graphs)
    } else {
        Err(format!(
            "--max-graphs must be between 1 and {MAX_GRAPHS_HARD_LIMIT}"
        ))
    }
}

fn default_version() -> String {
    "1".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSpec {
    #[serde(default = "default_version")]
    pub spec_version: String,
    pub name: String,
    pub entry: String,
    /// Explicit state key returned as `final_state`. When absent, legacy graphs
    /// continue to expose `__input__` as their terminal output.
    #[serde(default)]
    pub output_key: Option<String>,
    pub nodes: Vec<NodeSpec>,
    #[serde(default)]
    pub edges: Vec<EdgeSpec>,
    #[serde(default, alias = "recursion_limit")]
    pub max_iterations: Option<usize>,
    #[serde(default)]
    pub max_parallelism: Option<usize>,
    #[serde(default)]
    pub reducers: BTreeMap<String, ReducerKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub json_mode: bool,
    #[serde(default)]
    pub evidence_required: bool,
    #[serde(default)]
    pub max_tokens: Option<usize>,
    #[serde(default)]
    pub routes: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Llm,
    Router,
    Passthrough,
    StateTransform,
    Join,
    /// Parallel fan-out: execute multiple branches concurrently.
    /// Requires `config.branches` (array of {id, entry, input}), `config.max_parallelism`,
    /// `config.join` (target join node), and optional `config.fail_policy` and `config.timeout_ms`.
    Parallel,
    /// Reference another registered graph as a subgraph node.
    /// Requires `config.graph_name`, optional `config.input_key` and `config.output_key`.
    Subgraph,
    /// Human approval gate: interrupt execution for human decision.
    /// Requires `config.prompt_key`, `config.audience`, `config.allowed_decisions`,
    /// and optional `config.expiry_ms` and `config.output_key`.
    HumanApproval,
    /// Reserved effectful class. It is accepted for truthful classification
    /// but is not executable by this local runtime.
    External,
    /// Reserved tool class. It is accepted for truthful classification but is
    /// not executable by this local runtime.
    Tool,
    /// Explicit loop class; deterministic resume does not support it.
    Loop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeEligibility {
    pub next_node_cursor: String,
    pub chain: Vec<String>,
    pub dependency_summary: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReducerKind {
    LastWriteWins,
    Append,
    Add,
    Merge,
}

impl GraphSpec {
    /// Return the executable contract for every declared node type.
    /// Reserved effectful classes are deliberately rejected before registration.
    pub fn executable_node_type(node_type: &NodeType) -> Result<&'static str, String> {
        match node_type {
            NodeType::Llm => Ok("llm"),
            NodeType::Router => Ok("router"),
            NodeType::Passthrough => Ok("passthrough"),
            NodeType::StateTransform => Ok("state_transform"),
            NodeType::Join => Ok("join"),
            NodeType::Parallel => Ok("parallel"),
            NodeType::Subgraph => Ok("subgraph"),
            NodeType::HumanApproval => Ok("human_approval"),
            NodeType::External => Err("UNSUPPORTED_NODE_TYPE: external".into()),
            NodeType::Tool => Ok("tool"),
            NodeType::Loop => Err("UNSUPPORTED_NODE_TYPE: loop".into()),
        }
    }

    pub fn normalize(mut self) -> Self {
        self.spec_version = "2".into();
        if self.max_iterations.is_none() {
            self.max_iterations = Some(64);
        }
        if self.max_parallelism.is_none() {
            self.max_parallelism = Some(8);
        }
        self
    }

    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.nodes.iter().any(|n| n.routes.is_some()) {
            warnings.push("legacy route maps are normalized in lexicographic pattern order; use config.rules for explicit first-match order".into());
        }
        warnings
    }

    /// Classify resume support from the declarative graph only. Runtime
    /// observations never upgrade an ineligible graph into the supported lane.
    pub fn resume_eligibility(&self) -> Result<ResumeEligibility, String> {
        if !self.reducers.is_empty() {
            return Err("reducers are outside the deterministic local resume subset".into());
        }

        for node in &self.nodes {
            match node.node_type {
                NodeType::Passthrough => {
                    if node.evidence_required {
                        return Err(format!(
                            "node '{}' declares an evidence dependency",
                            node.id
                        ));
                    }
                    let empty_config = node.config.is_null()
                        || node
                            .config
                            .as_object()
                            .is_some_and(|object| object.is_empty());
                    if !empty_config {
                        return Err(format!(
                            "passthrough node '{}' has unsupported config",
                            node.id
                        ));
                    }
                }
                NodeType::StateTransform => {
                    if node.evidence_required {
                        return Err(format!(
                            "node '{}' declares an evidence dependency",
                            node.id
                        ));
                    }
                    let Some(object) = node.config.as_object() else {
                        return Err(format!("transform node '{}' config is not local", node.id));
                    };
                    if object.keys().any(|key| key != "operations") {
                        return Err(format!(
                            "transform node '{}' has unsupported config",
                            node.id
                        ));
                    }
                }
                NodeType::Llm => return Err(format!("node '{}' is an LLM node", node.id)),
                NodeType::Router => return Err(format!("node '{}' is a router", node.id)),
                NodeType::Join => return Err(format!("node '{}' is a join", node.id)),
                NodeType::Parallel => return Err(format!("node '{}' is parallel", node.id)),
                NodeType::Subgraph => return Err(format!("node '{}' is a subgraph", node.id)),
                NodeType::HumanApproval => {
                    return Err(format!("node '{}' is an approval node", node.id));
                }
                NodeType::External => {
                    return Err(format!("node '{}' is an external node", node.id));
                }
                NodeType::Tool => return Err(format!("node '{}' is a tool node", node.id)),
                NodeType::Loop => return Err(format!("node '{}' is a loop node", node.id)),
            }
        }

        let mut successors: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut predecessors: BTreeMap<&str, usize> = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), 0))
            .collect();
        for edge in &self.edges {
            successors
                .entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
            if edge.to != "END" {
                *predecessors
                    .get_mut(edge.to.as_str())
                    .expect("validated edge target") += 1;
            }
        }
        for node in &self.nodes {
            let count = successors.get(node.id.as_str()).map_or(0, Vec::len);
            if count != 1 {
                return Err(format!(
                    "linear resume requires exactly one successor for node '{}'",
                    node.id
                ));
            }
        }
        if predecessors.get(self.entry.as_str()).copied().unwrap_or(0) != 0 {
            return Err("resume entry must have no predecessor".into());
        }
        for node in &self.nodes {
            if node.id != self.entry && predecessors.get(node.id.as_str()).copied() != Some(1) {
                return Err(format!(
                    "linear resume requires one predecessor for node '{}'",
                    node.id
                ));
            }
        }

        let mut chain = Vec::with_capacity(self.nodes.len());
        let mut current = self.entry.as_str();
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(current) {
                return Err("loops are outside the deterministic local resume subset".into());
            }
            chain.push(current.to_owned());
            let next = successors
                .get(current)
                .and_then(|targets| targets.first())
                .copied()
                .expect("successor count checked");
            if next == "END" {
                break;
            }
            if !predecessors.contains_key(next) {
                return Err("linear resume successor is not a graph node".into());
            }
            current = next;
        }
        if chain.len() != self.nodes.len() {
            return Err("linear resume requires every node to be on the entry chain".into());
        }

        Ok(ResumeEligibility {
            next_node_cursor: self.entry.clone(),
            chain: chain.clone(),
            dependency_summary: serde_json::json!({
                "classification": "deterministic_local_resume",
                "eligible": true,
                "node_types": ["passthrough", "state_transform"],
                "chain": chain,
                "source_witnesses": {"required": false, "validated": []},
                "external_dependencies": false,
            }),
        })
    }
}

pub fn parse_and_validate(raw: &Value) -> Result<GraphSpec, String> {
    ensure_size(raw, MAX_GRAPH_BYTES, "serialized graph spec")?;
    reject_dangerous_keys(raw)?;
    let spec: GraphSpec =
        serde_json::from_value(raw.clone()).map_err(|e| format!("invalid graph spec: {e}"))?;
    validate(&spec)?;
    Ok(spec.normalize())
}

pub fn validate(spec: &GraphSpec) -> Result<(), String> {
    if !valid_id(&spec.name) {
        return Err("graph name must match [A-Za-z0-9_.-]{1,64}".into());
    }
    if spec.nodes.is_empty() || spec.nodes.len() > MAX_NODES {
        return Err(format!("graph nodes must be 1..={MAX_NODES}"));
    }
    if spec.edges.len() > MAX_EDGES {
        return Err(format!("graph edge limit ({MAX_EDGES}) exceeded"));
    }
    let iterations = spec.max_iterations.unwrap_or(MAX_ITERATIONS);
    if iterations == 0 || iterations > MAX_ITERATIONS {
        return Err(format!("max_iterations must be 1..={MAX_ITERATIONS}"));
    }
    if spec.max_parallelism.unwrap_or(8) == 0 || spec.max_parallelism.unwrap_or(8) > 32 {
        return Err("max_parallelism must be 1..=32".into());
    }
    let ids: BTreeSet<_> = spec.nodes.iter().map(|n| n.id.as_str()).collect();
    if ids.len() != spec.nodes.len() {
        return Err("duplicate node ID".into());
    }
    if !ids.contains(spec.entry.as_str()) {
        return Err(format!("entry node '{}' not found", spec.entry));
    }
    if spec.output_key.as_deref().is_some_and(str::is_empty) {
        return Err("output_key must not be empty when provided".into());
    }
    for node in &spec.nodes {
        if !valid_id(&node.id) {
            return Err(format!("invalid node ID '{}'", node.id));
        }
        validate_node(node, &ids)?;
    }
    for edge in &spec.edges {
        if !ids.contains(edge.from.as_str()) {
            return Err(format!("edge source '{}' not found", edge.from));
        }
        if edge.to != "END" && !ids.contains(edge.to.as_str()) {
            return Err(format!("edge target '{}' not found", edge.to));
        }
    }
    validate_state_write_conflicts(spec)?;
    Ok(())
}

fn validate_state_write_conflicts(spec: &GraphSpec) -> Result<(), String> {
    let ids: Vec<&str> = spec.nodes.iter().map(|node| node.id.as_str()).collect();
    let mut reach = vec![vec![false; ids.len()]; ids.len()];
    for edge in &spec.edges {
        if edge.to != "END" {
            if let (Some(from), Some(to)) = (
                ids.iter().position(|id| *id == edge.from),
                ids.iter().position(|id| *id == edge.to),
            ) {
                reach[from][to] = true;
            }
        }
    }
    for k in 0..ids.len() {
        for i in 0..ids.len() {
            for j in 0..ids.len() {
                reach[i][j] = reach[i][j] || (reach[i][k] && reach[k][j]);
            }
        }
    }

    let mut writers: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, node) in spec.nodes.iter().enumerate() {
        let mut keys = Vec::new();
        match node.node_type {
            NodeType::Llm | NodeType::HumanApproval | NodeType::Subgraph => {
                if let Some(key) = node
                    .config
                    .get(if node.node_type == NodeType::Llm {
                        "output_key"
                    } else if node.node_type == NodeType::HumanApproval {
                        "output_key"
                    } else {
                        "output_key"
                    })
                    .and_then(Value::as_str)
                    .filter(|key| !key.is_empty())
                {
                    keys.push(key.to_owned());
                }
            }
            NodeType::StateTransform => {
                if let Some(operations) = node.config.get("operations").and_then(Value::as_array) {
                    keys.extend(operations.iter().filter_map(|operation| {
                        operation
                            .get("path")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    }));
                }
            }
            NodeType::Join => {
                if let Some(key) = node.config.get("output").and_then(Value::as_str) {
                    keys.push(key.to_owned());
                }
            }
            _ => {}
        }
        for key in keys {
            writers.entry(key).or_default().push(index);
        }
    }
    for (key, nodes) in writers {
        if spec.reducers.contains_key(&key) {
            continue;
        }
        for left in 0..nodes.len() {
            for right in (left + 1)..nodes.len() {
                let a = nodes[left];
                let b = nodes[right];
                if reach[a][b] || reach[b][a] {
                    continue;
                }
                let shared_ancestor = (0..ids.len()).any(|ancestor| {
                    ancestor != a && ancestor != b && reach[ancestor][a] && reach[ancestor][b]
                });
                if shared_ancestor {
                    return Err(format!(
                        "state key '{}' is written by unordered parallel nodes '{}' and '{}'; declare reducers.{}",
                        key, ids[a], ids[b], ""
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_node(node: &NodeSpec, ids: &BTreeSet<&str>) -> Result<(), String> {
    if node.node_type == NodeType::Router {
        let targets: Vec<String> = if let Some(routes) = &node.routes {
            routes.values().cloned().collect()
        } else {
            node.config
                .get("rules")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .flat_map(|r| {
                    r.get("targets")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                })
                .filter_map(|v| v.as_str().map(str::to_owned))
                .chain(
                    node.config
                        .get("default")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str().map(str::to_owned)),
                )
                .collect()
        };
        if targets.is_empty() {
            return Err(format!(
                "router node '{}' must define routes/rules and default",
                node.id
            ));
        }
        if node.routes.is_none() {
            if node
                .config
                .get("default")
                .and_then(Value::as_array)
                .is_none()
            {
                return Err(format!(
                    "router node '{}' requires explicit default",
                    node.id
                ));
            }
            for rule in node
                .config
                .get("rules")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let op = rule.get("op").and_then(Value::as_str).unwrap_or("");
                if ![
                    "equals", "eq", "exists", "contains", "lt", "lte", "gt", "gte",
                ]
                .contains(&op)
                {
                    return Err(format!(
                        "router node '{}' has unsupported predicate '{op}'",
                        node.id
                    ));
                }
            }
        }
        for target in targets {
            if target != "END" && !ids.contains(target.as_str()) {
                return Err(format!(
                    "router node '{}' target '{}' not found",
                    node.id, target
                ));
            }
        }
    }
    if node.node_type == NodeType::Llm {
        if node.evidence_required {
            if !node.json_mode {
                return Err(format!(
                    "LLM node '{}' with evidence_required requires json_mode=true",
                    node.id
                ));
            }
            if node
                .config
                .get("output_key")
                .and_then(Value::as_str)
                .map_or(true, str::is_empty)
            {
                return Err(format!(
                    "LLM node '{}' with evidence_required requires config.output_key",
                    node.id
                ));
            }
        }
        if node
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.len() > 16 * 1024)
        {
            return Err("LLM prompt exceeds 16384 bytes".into());
        }
        if node.max_tokens.unwrap_or(1024) > 8192 {
            return Err("LLM max_tokens exceeds 8192".into());
        }
        if node.model.as_ref().is_some_and(|m| !valid_model_alias(m)) {
            return Err("model must be a conservative server alias".into());
        }
        let timeout = node
            .config
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(120_000);
        if timeout == 0 || timeout > 120_000 {
            return Err("LLM timeout_ms must be 1..=120000".into());
        }
        if let Some(retry) = node.config.get("retry") {
            let attempts = retry
                .get("max_attempts")
                .and_then(Value::as_u64)
                .unwrap_or(3);
            if attempts == 0 || attempts > 5 {
                return Err("retry max_attempts must be 1..=5".into());
            }
        }
    }
    if node.node_type == NodeType::StateTransform {
        let operations = node
            .config
            .get("operations")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("state_transform '{}' requires operations", node.id))?;
        if operations.is_empty() || operations.len() > 64 {
            return Err("transform operations must be 1..=64".into());
        }
        for operation in operations {
            let op = operation.get("op").and_then(Value::as_str).unwrap_or("");
            if ![
                "set",
                "copy",
                "delete",
                "increment",
                "append",
                "merge",
                "merge_object",
                "select",
                "compare",
                "format",
            ]
            .contains(&op)
            {
                return Err(format!("unsupported transform operation '{op}'"));
            }
        }
    }
    if node.node_type == NodeType::Join {
        let mode = node
            .config
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("collect_array");
        if ![
            "collect_array",
            "merge_objects",
            "first_non_null",
            "all_success",
            "quorum",
        ]
        .contains(&mode)
        {
            return Err(format!("unsupported join mode '{mode}'"));
        }
        if node
            .config
            .get("inputs")
            .and_then(Value::as_array)
            .is_none()
            || node.config.get("output").and_then(Value::as_str).is_none()
        {
            return Err(format!("join '{}' requires inputs and output", node.id));
        }
    }
    if node.node_type == NodeType::Parallel {
        let branches = node
            .config
            .get("branches")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("parallel '{}' requires branches array", node.id))?;
        if branches.is_empty() || branches.len() > 16 {
            return Err(format!("parallel '{}' branches must be 1..=16", node.id));
        }
        for branch in branches {
            let entry = branch
                .get("entry")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("parallel '{}' branch missing entry", node.id))?;
            if !ids.contains(entry) {
                return Err(format!(
                    "parallel '{}' branch entry '{}' not found",
                    node.id, entry
                ));
            }
        }
        let join = node
            .config
            .get("join")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("parallel '{}' requires join target", node.id))?;
        if join != "END" && !ids.contains(join) {
            return Err(format!(
                "parallel '{}' join target '{}' not found",
                node.id, join
            ));
        }
        if let Some(policy) = node.config.get("fail_policy").and_then(Value::as_str) {
            if !["fail_fast", "collect_partial", "ignore"].contains(&policy) {
                return Err(format!("unsupported fail_policy '{policy}'"));
            }
        }
    }
    if node.node_type == NodeType::Subgraph {
        if node
            .config
            .get("graph_name")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(format!("subgraph '{}' requires config.graph_name", node.id));
        }
    }
    if node.node_type == NodeType::HumanApproval {
        if node
            .config
            .get("prompt_key")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(format!(
                "human_approval '{}' requires config.prompt_key",
                node.id
            ));
        }
        if node
            .config
            .get("audience")
            .and_then(Value::as_array)
            .is_none()
        {
            return Err(format!(
                "human_approval '{}' requires config.audience array",
                node.id
            ));
        }
    }
    Ok(())
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

fn valid_model_alias(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 128
        && !model.contains("://")
        && !model.starts_with('/')
        && !model.contains("..")
        && model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:/-".contains(&b))
}

pub fn ensure_size(value: &Value, limit: usize, label: &str) -> Result<(), String> {
    let len = serde_json::to_vec(value).map_err(|e| e.to_string())?.len();
    if len > limit {
        Err(format!("{label} exceeds {limit} bytes"))
    } else {
        Ok(())
    }
}

fn reject_dangerous_keys(value: &Value) -> Result<(), String> {
    const DENY: &[&str] = &[
        "command",
        "shell",
        "script",
        "filesystem",
        "secret",
        "env",
        "environment",
        "base_url",
        "provider_url",
    ];
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let normalized = key.to_ascii_lowercase();
                if DENY.contains(&normalized.as_str()) {
                    return Err(format!("policy denied field '{key}'"));
                }
                reject_dangerous_keys(value)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_dangerous_keys(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_and_validate;
    use serde_json::{json, Value};

    fn parallel(reducers: Value) -> Value {
        json!({
            "name":"conflict", "entry":"fork", "reducers": reducers,
            "nodes":[
                {"id":"fork","type":"passthrough"},
                {"id":"left","type":"state_transform","config":{"operations":[{"op":"set","path":"shared","value":"left"}]}},
                {"id":"right","type":"state_transform","config":{"operations":[{"op":"set","path":"shared","value":"right"}]}}
            ],
            "edges":[{"from":"fork","to":"left"},{"from":"fork","to":"right"},{"from":"left","to":"END"},{"from":"right","to":"END"}]
        })
    }

    #[test]
    fn unordered_parallel_writes_require_reducer() {
        let error = parse_and_validate(&parallel(json!({}))).expect_err("conflict rejected");
        assert!(error.contains("unordered parallel nodes"));
        assert!(parse_and_validate(&parallel(json!({"shared":"append"}))).is_ok());
    }

    #[test]
    fn sequential_repeated_write_is_allowed() {
        let spec = json!({
            "name":"sequential", "entry":"left",
            "nodes":[
                {"id":"left","type":"state_transform","config":{"operations":[{"op":"set","path":"shared","value":"left"}]}},
                {"id":"right","type":"state_transform","config":{"operations":[{"op":"set","path":"shared","value":"right"}]}}
            ], "edges":[{"from":"left","to":"right"},{"from":"right","to":"END"}]
        });
        assert!(parse_and_validate(&spec).is_ok());
    }
}
