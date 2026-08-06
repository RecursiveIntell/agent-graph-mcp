use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command as StdCommand, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc, Mutex,
};

use async_trait::async_trait;
use llm_pipeline::payload::Payload;
use llm_pipeline::{ExecCtx, LlmCall, LlmConfig};
use ri_agent_graph::command::{Command, Navigation, NodeOutput};
use ri_agent_graph::config::GraphConfig;
use ri_agent_graph::error::{AgentGraphError, Result};
use ri_agent_graph::node::Node;
use ri_agent_graph::state::AgentState;
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::sync::Notify;

use crate::evidence::validate_research_evidence;
use crate::model_executor::ExecutorHandle;
use crate::tool_exec::ToolExecContext;

#[derive(Clone)]
pub struct RunContext {
    pub cancelled: Arc<AtomicBool>,
    pub cancellation: Arc<Notify>,
    /// Shared pre-invocation attempt counter. Incremented atomically for every
    /// provider attempt, including attempts that later fail or time out.
    pub llm_calls: Arc<AtomicU64>,
    /// Run-scoped cap on provider attempts; None means unlimited.
    pub max_llm_calls: Option<u64>,
    /// Optional model invocation executor for local backends.
    pub executor: ExecutorHandle,
    /// Shared typed ledger of observed invocations for the terminal receipt.
    pub llm_invocations: Arc<Mutex<Vec<Value>>>,
    /// Optional subgraph executor provided by the host (server/run manager).
    /// When None, subgraph nodes fail with an explicit error.
    pub subgraph: Option<SubgraphExecutor>,
    /// Shared nesting depth counter for subgraph-in-subgraph recursion guards.
    pub subgraph_depth: Arc<AtomicU32>,
    /// Maximum allowed subgraph nesting depth.
    pub subgraph_limit: u32,
}

/// Host-provided subgraph runner: (graph_name, input_value, depth) -> terminal output value.
/// The runner returns a future so sub-executions can be awaited inside the engine's
/// async runtime without blocking (block_on is illegal on a runtime worker thread).
#[derive(Clone)]
pub struct SubgraphExecutor {
    pub inner: Arc<
        dyn Fn(
                String,
                Value,
                u32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = std::result::Result<Value, String>> + Send>,
            > + Send
            + Sync,
    >,
}

impl std::fmt::Debug for SubgraphExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubgraphExecutor").finish_non_exhaustive()
    }
}

impl RunContext {
    fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(AgentGraphError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Atomically reserve one provider-attempt slot BEFORE any provider call,
    /// so failed and timed-out attempts still count and concurrent nodes cannot
    /// race past the budget. A denied attempt leaves the counter unchanged.
    pub fn reserve_llm_attempt(&self) -> Result<u64> {
        match self
            .llm_calls
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                if self.max_llm_calls.is_some_and(|limit| used >= limit) {
                    None
                } else {
                    Some(used + 1)
                }
            }) {
            // fetch_update returns the PREVIOUS value; the stored counter was
            // already advanced to used+1, so the attempt number is used+1.
            Ok(used) => Ok(used.saturating_add(1)),
            Err(_) => Err(AgentGraphError::PayloadError("BUDGET_EXHAUSTED".to_owned())),
        }
    }
}

async fn cancellation_requested(
    cancelled: Arc<AtomicBool>,
    cancellation: Arc<Notify>,
) -> Result<()> {
    if cancelled.load(Ordering::SeqCst) {
        return Err(AgentGraphError::Cancelled);
    }
    cancellation.notified().await;
    Err(AgentGraphError::Cancelled)
}

pub struct PassthroughNode {
    pub ctx: RunContext,
}
#[async_trait]
impl Node for PassthroughNode {
    async fn execute(&self, _: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;
        Ok(NodeOutput::Done)
    }
}

pub struct LlmNode {
    pub id: String,
    pub base_url: String,
    pub default_model: String,
    /// Provider API key for http(s) llm-pipeline calls (Bearer header).
    pub api_key: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub json_mode: bool,
    pub evidence_required: bool,
    pub max_tokens: Option<usize>,
    pub timeout_ms: u64,
    pub input_key: String,
    pub output_key: String,
    /// Optional file path to prepend to the prompt for project-specific context.
    /// Supports "AGENTS.md" shorthand (resolved from daemon working directory)
    /// and absolute/relative paths. Content is read once before template expansion.
    pub context_file: Option<String>,
    /// Tool definitions exposed to this LLM node during execution.
    /// Each entry is a JSON tool definition in OpenAI function-calling format.
    /// When non-empty, the provider receives tool definitions and may return
    /// tool_calls in the response. The node loops until a text response is
    /// received or max_tool_calls (default 5) is reached.
    pub tools: Vec<serde_json::Value>,
    pub ctx: RunContext,
}

#[async_trait]
impl Node for LlmNode {
    async fn execute(&self, state: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;
        let input = state
            .get_opt::<Value>(&self.input_key)
            .await?
            .unwrap_or(Value::Null);
        let input_json = serde_json::to_string(&input)?;
        // Prepend context file content if configured. This allows graph specs
        // to reference AGENTS.md or project files instead of embedding massive
        // context in every prompt. Large files (>32KB) are truncated.
        let mut rendered = if let Some(ref cf) = self.context_file {
            let resolved = match cf.as_str() {
                "AGENTS.md" => {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    cwd.join("AGENTS.md")
                }
                p if p.starts_with('/') => std::path::PathBuf::from(p),
                p => {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    cwd.join(p)
                }
            };
            match std::fs::read_to_string(&resolved) {
                Ok(file_content) => {
                    let truncated: String = file_content.chars().take(32768).collect();
                    format!(
                        "[CONTEXT FILE: {}]

{}

---

{}",
                        cf, truncated, self.prompt
                    )
                }
                Err(_) => self.prompt.clone(),
            }
        } else {
            self.prompt.clone()
        };
        // Expand the input placeholder. Council specs reference the input key
        // by name (e.g. {brief}, {plan}, {input}) — the daemon previously only
        // expanded {input}, so prompts using {<input_key>} sent the literal
        // placeholder to the model and analysts never received the coordinator
        // output (observed 2026-08-03: analysts returned empty content).
        let mut rendered = self.prompt.replace("{input}", &input_json);
        if !self.input_key.is_empty() {
            rendered = rendered.replace(&format!("{{{}}}", self.input_key), &input_json);
        }
        // Expand any remaining {keyname} placeholders from agent state.
        // Council specs use cross-node references like {attack} and {cross_reviewed}
        // which are separate state keys, not the current node's input_key.
        let state_snapshot = state.snapshot().await;
        for (key, value) in &state_snapshot.data {
            if key.starts_with("__") {
                continue; // skip internal keys
            }
            let placeholder = format!("{{{}}}", key);
            if rendered.contains(&placeholder) {
                if let Ok(json_str) = serde_json::to_string(value) {
                    rendered = rendered.replace(&placeholder, &json_str);
                }
            }
        }
        let model = self.model.as_deref().unwrap_or(&self.default_model);
        // Reserve the provider attempt before any invocation; the limit cannot
        // be bypassed by parallel nodes and failed calls still count.
        let attempt = self.ctx.reserve_llm_attempt()?;
        // Check for a local model executor before falling through to the
        // default provider path. Lease material never enters the prompt,
        // graph state, or provider body — only the lease digest is safe
        // for receipts.
        let _executor_handle = self.ctx.executor.try_acquire(&self.id, &self.id, attempt);
        // When a backend is acquired, the provider path below is skipped
        // and the local executor handles the invocation. The handle is
        // released on drop or after the provider call completes.
        // Currently always None — the default path is unchanged.
        let mut config = LlmConfig::default().with_json_mode(self.json_mode);
        if let Some(tokens) = self.max_tokens {
            config = config.with_max_tokens(tokens as u32);
        }
        // Pass tool definitions to the provider when tools are configured.
        let has_tools = !self.tools.is_empty();
        let result: std::result::Result<Value, AgentGraphError> = if has_tools {
            let tool_ctx =
                ToolExecContext::new(&self.tools).map_err(AgentGraphError::PayloadError)?;
            let runner = tool_ctx.runner();
            let mut request = llm_pipeline::ToolLoopRequest::new(model, rendered.clone());
            request.config = config;
            request.max_round_trips = 5;
            request.api_key = self.api_key.clone();
            let exec_ctx = ExecCtx::builder(&self.base_url).build();
            let use_openai =
                self.base_url.starts_with("http://") || self.base_url.starts_with("https://");
            let response = tokio::task::spawn_blocking(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    if use_openai {
                        runner.run_openai_responses(&exec_ctx, request).await
                    } else {
                        runner.run_ollama(&exec_ctx, request).await
                    }
                })
            })
            .await
            .map_err(|e| AgentGraphError::PayloadError(format!("tool loop task failed: {e}")))?;
            response
                .map(|r| Value::String(r.final_text))
                .map_err(|e| AgentGraphError::PayloadError(e.to_string()))
        } else if self.base_url == "codex-app-server://" {
            let model = model.to_owned();
            let prompt = rendered.clone();
            let timeout = std::time::Duration::from_millis(self.timeout_ms);
            let cwd = std::env::current_dir().map_err(|e| {
                AgentGraphError::PayloadError(format!("codex working directory unavailable: {e}"))
            })?;
            tokio::select! {
                result = tokio::task::spawn_blocking(move || {
                    crate::codex_app_server::run_turn("codex", &model, &cwd, &prompt, timeout)
                }) => {
                    let text = result
                        .map_err(|e| AgentGraphError::PayloadError(format!("codex app-server task failed: {e}")))?
                        .map_err(AgentGraphError::PayloadError)?;
                    Ok(Value::String(text))
                }
                _ = cancellation_requested(self.ctx.cancelled.clone(), self.ctx.cancellation.clone()) => {
                    Err(AgentGraphError::Cancelled)
                }
            }
        } else {
            let call = LlmCall::new(&self.id, rendered)
                .with_model(model)
                .with_timeout(std::time::Duration::from_millis(self.timeout_ms))
                .with_config(config);
            let mut exec_builder = ExecCtx::builder(&self.base_url);
            // Remote OpenAI-compatible endpoints (http/https) must use the
            // OpenAI backend (/v1/chat/completions). Without this, the
            // llm-pipeline default is OllamaBackend, which sends an Ollama
            // body to /api/generate and is rejected by OpenAI-compatible
            // providers (observed 2026-08-03: maas returned HTTP 400 "Role
            // must be in [user, assistant, ...]"). codex-app-server:// is
            // handled in the branch above; local Ollama keeps the default.
            if self.base_url.starts_with("http://") || self.base_url.starts_with("https://") {
                exec_builder = match self.api_key.as_deref() {
                    Some(key) => exec_builder.openai_with_key(key),
                    None => exec_builder.openai(),
                };
            }
            if let Some(key) = self.api_key.as_deref() {
                // Attach the provider key as a Bearer header on the http(s)
                // llm-pipeline path only. The key is never logged or echoed.
                let mut headers = reqwest::header::HeaderMap::new();
                let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
                    .map_err(|e| {
                        AgentGraphError::PayloadError(format!("invalid api key header: {e}"))
                    })?;
                headers.insert(reqwest::header::AUTHORIZATION, value);
                let client = reqwest::Client::builder()
                    .default_headers(headers)
                    .build()
                    .map_err(|e| {
                        AgentGraphError::PayloadError(format!("http client build failed: {e}"))
                    })?;
                exec_builder = exec_builder.client(client);
            }
            let exec_ctx = exec_builder.build();
            tokio::select! {
                result = call.invoke(&exec_ctx, input) => result
                    .map_err(|e| AgentGraphError::PayloadError(e.to_string()))
                    .map(|mut payload| {
                        // Qwen models on Ollama output to the `reasoning`
                        // field (thinking mode) with empty `content`. When
                        // content is empty and reasoning is populated, use
                        // reasoning as the content so graph nodes receive
                        // usable output.
                        if let Some(content) = payload.value.get("content") {
                            if content.as_str().map_or(false, |s| s.is_empty()) {
                                if let Some(reasoning) = payload.value.get("reasoning") {
                                    if reasoning.as_str().map_or(false, |s| !s.is_empty()) {
                                        payload.value["content"] = reasoning.clone();
                                    }
                                }
                            }
                        }
                        payload.value
                    }),
                _ = cancellation_requested(self.ctx.cancelled.clone(), self.ctx.cancellation.clone()) => {
                    Err(AgentGraphError::Cancelled)
                }
            }
        };
        let output = match result {
            Ok(output) => {
                self.record_invocation(attempt, model, "succeeded");
                output
            }
            Err(error) => {
                self.record_invocation(attempt, model, "failed");
                return Err(error);
            }
        };
        self.ctx.check()?;
        if self.evidence_required {
            validate_research_evidence(&output).map_err(AgentGraphError::PayloadError)?;
        }
        // A node may write only its declared output key. `__input__` is reserved
        // for ingress and explicit legacy/router nodes; mirroring every LLM
        // result there made parallel branches race for the graph's final state.
        state.set_raw(&self.output_key, output).await?;
        Ok(NodeOutput::Done)
    }
}

impl LlmNode {
    /// Append one typed invocation record for the terminal receipt. The record
    /// is derived from the observed attempt, never from graph metadata alone.
    fn record_invocation(&self, attempt: u64, model: &str, outcome: &str) {
        if let Ok(mut invocations) = self.ctx.llm_invocations.lock() {
            invocations.push(serde_json::json!({
                "attempt": attempt,
                "node_id": self.id,
                "configured_model": model,
                "outcome": outcome,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cancellation_requested;
    use std::sync::{atomic::AtomicBool, Arc};
    use tokio::sync::Notify;

    #[tokio::test]
    async fn cancellation_primitive_wakes_a_pending_wait() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = Arc::new(Notify::new());
        let waiter = tokio::spawn(cancellation_requested(cancelled, cancellation.clone()));
        tokio::task::yield_now().await;
        cancellation.notify_waiters();
        assert!(waiter
            .await
            .expect("cancellation waiter completed")
            .is_err());
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransformConfig {
    pub operations: Vec<TransformOp>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransformOp {
    pub op: String,
    pub path: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub template: Option<String>,
}

pub struct TransformNode {
    pub config: TransformConfig,
    pub ctx: RunContext,
}

#[async_trait]
impl Node for TransformNode {
    async fn execute(&self, state: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;
        for op in &self.config.operations {
            apply_transform(state, op).await?;
        }
        Ok(NodeOutput::Done)
    }
}

async fn apply_transform(state: &AgentState, op: &TransformOp) -> Result<()> {
    let current = state
        .get_opt::<Value>(&op.path)
        .await?
        .unwrap_or(Value::Null);
    match op.op.as_str() {
        "set" => state.set_raw(&op.path, op.value.clone()).await?,
        "copy" => {
            let from = op
                .from
                .as_deref()
                .ok_or_else(|| AgentGraphError::StateError("copy requires from".into()))?;
            let v = state.get_opt::<Value>(from).await?.unwrap_or(Value::Null);
            state.set_raw(&op.path, v).await?;
        }
        "delete" => {
            state.remove(&op.path).await;
        }
        "increment" => {
            let a = current.as_f64().unwrap_or(0.0);
            let b = op.value.as_f64().unwrap_or(1.0);
            state.set_raw(&op.path, serde_json::json!(a + b)).await?;
        }
        "append" => {
            let mut out = match current {
                Value::Array(v) => v,
                Value::Null => vec![],
                v => vec![v],
            };
            out.push(op.value.clone());
            state.set_raw(&op.path, Value::Array(out)).await?;
        }
        "merge" | "merge_object" => {
            let mut out = current.as_object().cloned().unwrap_or_default();
            let add = op
                .value
                .as_object()
                .ok_or_else(|| AgentGraphError::StateError("merge value must be object".into()))?;
            out.extend(add.clone());
            state.set_raw(&op.path, Value::Object(out)).await?;
        }
        "select" => {
            let mut out = Map::new();
            for key in &op.values {
                if let Some(v) = state.get_opt::<Value>(key).await? {
                    out.insert(key.clone(), v);
                }
            }
            state.set_raw(&op.path, Value::Object(out)).await?;
        }
        "compare" => {
            state
                .set_raw(&op.path, Value::Bool(current == op.value))
                .await?
        }
        "format" => {
            let mut text = op.template.clone().unwrap_or_default();
            for key in &op.values {
                let v = state.get_opt::<Value>(key).await?.unwrap_or(Value::Null);
                text = text.replace(&format!("{{{key}}}"), value_text(&v).as_str());
            }
            state.set_raw(&op.path, Value::String(text)).await?;
        }
        other => {
            return Err(AgentGraphError::StateError(format!(
                "unsupported transform operation '{other}'"
            )))
        }
    }
    Ok(())
}

fn value_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouterConfig {
    pub rules: Vec<Rule>,
    pub default: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub path: String,
    pub op: String,
    #[serde(default)]
    pub value: Value,
    pub targets: Vec<String>,
}

pub struct RouterNode {
    pub config: RouterConfig,
    pub ctx: RunContext,
}

#[async_trait]
impl Node for RouterNode {
    async fn execute(&self, state: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;
        let mut targets = None;
        for rule in &self.config.rules {
            if predicate(state, rule).await? {
                targets = Some(rule.targets.clone());
                break;
            }
        }
        let targets = targets.unwrap_or_else(|| self.config.default.clone());
        let goto = if targets.is_empty() || targets == ["END"] {
            Navigation::End
        } else if targets.len() == 1 {
            Navigation::Node(targets[0].clone())
        } else {
            Navigation::Nodes(targets)
        };
        let mut update = HashMap::new();
        update.insert(
            "__route__".into(),
            serde_json::to_value(goto_label(&goto)).unwrap_or(Value::Null),
        );
        Ok(NodeOutput::Command(Command {
            update: Some(update),
            goto,
        }))
    }
}

fn goto_label(goto: &Navigation) -> Value {
    match goto {
        Navigation::End => Value::String("END".into()),
        Navigation::Node(v) => Value::String(v.clone()),
        Navigation::Nodes(v) => serde_json::json!(v),
        _ => Value::Null,
    }
}

async fn predicate(state: &AgentState, rule: &Rule) -> Result<bool> {
    let value = state
        .get_opt::<Value>(&rule.path)
        .await?
        .unwrap_or(Value::Null);
    Ok(match rule.op.as_str() {
        "equals" | "eq" => value == rule.value,
        "exists" => !value.is_null(),
        "contains" => value_text(&value).contains(&value_text(&rule.value)),
        "lt" => value
            .as_f64()
            .zip(rule.value.as_f64())
            .is_some_and(|(a, b)| a < b),
        "lte" => value
            .as_f64()
            .zip(rule.value.as_f64())
            .is_some_and(|(a, b)| a <= b),
        "gt" => value
            .as_f64()
            .zip(rule.value.as_f64())
            .is_some_and(|(a, b)| a > b),
        "gte" => value
            .as_f64()
            .zip(rule.value.as_f64())
            .is_some_and(|(a, b)| a >= b),
        "regex" => rule
            .value
            .as_str()
            .and_then(|pat| regex::Regex::new(pat).ok())
            .is_some_and(|re| re.is_match(&value_text(&value))),
        _ => false,
    })
}

pub fn legacy_router(routes: &std::collections::BTreeMap<String, String>) -> RouterConfig {
    RouterConfig {
        rules: routes
            .iter()
            .map(|(pattern, target)| Rule {
                path: "__input__".into(),
                op: "contains".into(),
                value: Value::String(pattern.clone()),
                targets: vec![target.clone()],
            })
            .collect(),
        default: vec!["END".into()],
    }
}

// ── HumanApprovalNode ──────────────────────────────────────────────────

/// A node that signals an approval gate: writes the approval request to state
/// and returns an error that the caller can handle.
pub struct HumanApprovalNode {
    pub prompt_key: String,
    pub output_key: String,
    pub audience: Vec<String>,
    pub allowed_decisions: Vec<String>,
    pub expiry_ms: u64,
    pub ctx: RunContext,
}

#[async_trait]
impl Node for HumanApprovalNode {
    async fn execute(&self, state: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;
        let prompt = state
            .get_opt::<Value>(&self.prompt_key)
            .await?
            .unwrap_or(Value::Null);

        let approval_request = serde_json::json!({
            "prompt": prompt,
            "audience": self.audience,
            "allowed_decisions": self.allowed_decisions,
            "expiry_ms": self.expiry_ms,
            "issued_at": chrono::Utc::now().to_rfc3339(),
            "status": "pending"
        });
        state
            .set_raw("__approval_request__", approval_request)
            .await?;

        // Check if a prior decision was already injected (e.g., via resume)
        if let Some(decision) = state.get_opt::<Value>(&self.output_key).await? {
            if !decision.is_null() {
                return Ok(NodeOutput::Done);
            }
        }

        // Signal interrupt by returning a recognizable error.
        // The graph engine's execute_with_interrupt will catch this.
        Err(AgentGraphError::InterruptError {
            node: "human_approval".into(),
            value: Some(
                serde_json::json!({"approval_required": true, "prompt_key": self.prompt_key}),
            ),
        })
    }
}

// ── ToolNode ───────────────────────────────────────────────────────────

/// A graph node that spawns the Hermes tools MCP broker as a child process
/// and invokes tools through MCP JSON-RPC over stdio.
pub struct ToolNode {
    pub id: String,
    pub python: String,
    pub hermes_source: String,
    pub lease: Value,
    pub receipt_dir: String,
    pub timeout_ms: u64,
    pub ctx: RunContext,
}

#[async_trait]
impl Node for ToolNode {
    async fn execute(&self, state: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;

        // Write lease to a temp file the broker can read.
        let lease_path = format!("{}/lease.json", self.receipt_dir);
        std::fs::create_dir_all(&self.receipt_dir)
            .map_err(|e| AgentGraphError::PayloadError(format!("receipt dir: {e}")))?;
        std::fs::write(
            &lease_path,
            serde_json::to_string(&self.lease)
                .map_err(|e| AgentGraphError::PayloadError(format!("lease serialize: {e}")))?,
        )
        .map_err(|e| AgentGraphError::PayloadError(format!("lease write: {e}")))?;

        // Build the read-only tool call from graph state.
        let tool_name = state
            .get_opt::<Value>("__tool_name__")
            .await?
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "read_file".to_owned());
        let tool_args: Value = state
            .get_opt::<Value>("__tool_args__")
            .await?
            .unwrap_or(Value::Null);

        // Spawn the Hermes MCP broker.
        let mut child = StdCommand::new(&self.python)
            .args(["-m", "agent.transports.hermes_tools_mcp_server"])
            .env("AGENT_GRAPH_LINEAGE", "1")
            .env("AGENT_GRAPH_LINEAGE_LEASE_PATH", &lease_path)
            .env("AGENT_GRAPH_LINEAGE_RECEIPT_DIR", &self.receipt_dir)
            .env("PYTHONPATH", &self.hermes_source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AgentGraphError::PayloadError(format!("broker spawn: {e}")))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentGraphError::PayloadError("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentGraphError::PayloadError("no stdout".into()))?;
        let stderr = child.stderr.take();

        // MCP initialize handshake.
        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "agent-graph-tool-node", "version": "0.1"}
            }
        });
        let init_line = serde_json::to_string(&init_req)
            .map_err(|e| AgentGraphError::PayloadError(format!("init: {e}")))?;
        writeln!(stdin, "{init_line}")
            .map_err(|e| AgentGraphError::PayloadError(format!("write init: {e}")))?;
        stdin
            .flush()
            .map_err(|e| AgentGraphError::PayloadError(format!("flush init: {e}")))?;

        let mut reader = BufReader::new(stdout);
        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|e| AgentGraphError::PayloadError(format!("read init: {e}")))?;

        // Send initialized notification.
        let notified = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        writeln!(
            stdin,
            "{}",
            serde_json::to_string(&notified)
                .map_err(|e| AgentGraphError::PayloadError(format!("notify: {e}")))?
        )
        .map_err(|e| AgentGraphError::PayloadError(format!("write notify: {e}")))?;
        stdin
            .flush()
            .map_err(|e| AgentGraphError::PayloadError(format!("flush notify: {e}")))?;

        // Make the tool call.
        let call_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": tool_args
            }
        });
        let call_line = serde_json::to_string(&call_req)
            .map_err(|e| AgentGraphError::PayloadError(format!("call: {e}")))?;
        writeln!(stdin, "{call_line}")
            .map_err(|e| AgentGraphError::PayloadError(format!("write call: {e}")))?;
        stdin
            .flush()
            .map_err(|e| AgentGraphError::PayloadError(format!("flush call: {e}")))?;

        response.clear();
        reader
            .read_line(&mut response)
            .map_err(|e| AgentGraphError::PayloadError(format!("read result: {e}")))?;

        // Drop stdin to signal EOF.
        drop(stdin);

        // Wait for child with timeout.
        let timeout_dur = std::time::Duration::from_millis(self.timeout_ms);
        let status = tokio::time::timeout(
            timeout_dur,
            tokio::task::spawn_blocking(move || child.wait()),
        )
        .await
        .map_err(|_| AgentGraphError::PayloadError("broker timed out".into()))?
        .map_err(|e| AgentGraphError::PayloadError(format!("join: {e}")))?
        .map_err(|e| AgentGraphError::PayloadError(format!("wait: {e}")))?;

        // Capture stderr for diagnostics.
        let stderr_output = if let Some(stderr) = stderr {
            let mut buf = String::new();
            let _ = BufReader::new(stderr).read_line(&mut buf);
            buf
        } else {
            String::new()
        };

        // Parse MCP response.
        let result: Value = serde_json::from_str(&response).map_err(|e| {
            AgentGraphError::PayloadError(format!(
                "parse result (exit={status:?}, stderr={stderr_output:?}): {e}"
            ))
        })?;

        let tool_output = result
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .cloned()
            .unwrap_or(Value::Null);

        // Check for errors.
        if let Some(err) = result.get("error") {
            return Err(AgentGraphError::PayloadError(format!(
                "tool '{tool_name}' failed: {err}"
            )));
        }
        if !status.success() {
            return Err(AgentGraphError::PayloadError(format!(
                "broker exited {status}: {stderr_output}"
            )));
        }

        // Read receipt ledger for verification.
        let ledger_path = format!("{}/ledger.jsonl", self.receipt_dir);
        let receipt_evidence = if let Ok(contents) = std::fs::read_to_string(&ledger_path) {
            let receipts: Vec<Value> = contents
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            receipts.into()
        } else {
            Value::Null
        };

        state
            .set_raw("__tool_result__", tool_output.clone())
            .await?;
        state.set_raw("__tool_receipts__", receipt_evidence).await?;
        state.set_raw("__tool_success__", Value::Bool(true)).await?;

        Ok(NodeOutput::Done)
    }
}

/// Explicit bounded loop: re-enters `entry` until `max_iterations` is reached,
/// then navigates to `exit` (or END). The engine's graph-level max_iterations
/// remains the outer safety net. The loop node always navigates explicitly so
/// multi-out-edge fan-out cannot fire during a cycle.
pub struct LoopNode {
    pub entry: String,
    pub exit: String,
    pub max_iterations: u64,
    pub counter_key: String,
    pub ctx: RunContext,
}

#[async_trait]
impl Node for LoopNode {
    async fn execute(&self, state: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;
        let count: u64 = state.get_opt::<u64>(&self.counter_key).await?.unwrap_or(0);
        let mut update = HashMap::new();
        if count < self.max_iterations {
            update.insert(self.counter_key.clone(), Value::from(count + 1));
            Ok(NodeOutput::Command(Command {
                goto: Navigation::Node(self.entry.clone()),
                update: Some(update),
            }))
        } else {
            update.insert(self.counter_key.clone(), Value::Null);
            let goto = if self.exit == "END" {
                Navigation::End
            } else {
                Navigation::Node(self.exit.clone())
            };
            Ok(NodeOutput::Command(Command {
                goto,
                update: Some(update),
            }))
        }
    }
}

/// Subgraph reference: executes another registered graph through the host-provided
/// runner, then writes the referenced graph's terminal output under `output_key`
/// and returns Done (engine follows this node's out-edges).
pub struct SubgraphNode {
    pub graph_name: String,
    pub input_key: String,
    pub output_key: String,
    pub ctx: RunContext,
}

#[async_trait]
impl Node for SubgraphNode {
    async fn execute(&self, state: &AgentState, _: &GraphConfig) -> Result<NodeOutput> {
        self.ctx.check()?;
        let depth = self.ctx.subgraph_depth.fetch_add(1, Ordering::SeqCst) + 1;
        if depth > self.ctx.subgraph_limit {
            self.ctx.subgraph_depth.fetch_sub(1, Ordering::SeqCst);
            return Err(AgentGraphError::ExecutionError(format!(
                "subgraph depth exceeded (limit {})",
                self.ctx.subgraph_limit
            )));
        }
        let input = state
            .get_opt::<Value>(&self.input_key)
            .await?
            .unwrap_or(Value::Null);
        let runner = self.ctx.subgraph.clone().ok_or_else(|| {
            AgentGraphError::ExecutionError("subgraph executor unavailable".into())
        })?;
        let future = (runner.inner)(self.graph_name.clone(), input, depth);
        let result = future.await;
        self.ctx.subgraph_depth.fetch_sub(1, Ordering::SeqCst);
        let output = result.map_err(AgentGraphError::ExecutionError)?;
        state.set_raw(&self.output_key, output).await?;
        Ok(NodeOutput::Done)
    }
}
