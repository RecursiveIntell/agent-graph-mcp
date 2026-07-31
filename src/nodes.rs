use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
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

#[derive(Clone)]
pub struct RunContext {
    pub cancelled: Arc<AtomicBool>,
    pub cancellation: Arc<Notify>,
}

impl RunContext {
    fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(AgentGraphError::Cancelled)
        } else {
            Ok(())
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
    pub prompt: String,
    pub model: Option<String>,
    pub json_mode: bool,
    pub evidence_required: bool,
    pub max_tokens: Option<usize>,
    pub timeout_ms: u64,
    pub input_key: String,
    pub output_key: String,
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
        let rendered = self
            .prompt
            .replace("{input}", &serde_json::to_string(&input)?);
        let model = self.model.as_deref().unwrap_or(&self.default_model);
        let mut config = LlmConfig::default().with_json_mode(self.json_mode);
        if let Some(tokens) = self.max_tokens {
            config = config.with_max_tokens(tokens as u32);
        }
        let output = if self.base_url == "codex-app-server://" {
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
                    Value::String(text)
                }
                _ = cancellation_requested(self.ctx.cancelled.clone(), self.ctx.cancellation.clone()) => {
                    return Err(AgentGraphError::Cancelled);
                }
            }
        } else {
            let call = LlmCall::new(&self.id, rendered)
                .with_model(model)
                .with_timeout(std::time::Duration::from_millis(self.timeout_ms))
                .with_config(config);
            let exec_ctx = ExecCtx::builder(&self.base_url).build();
            tokio::select! {
                result = call.invoke(&exec_ctx, input) => result
                    .map_err(|e| AgentGraphError::PayloadError(e.to_string()))?
                    .value,
                _ = cancellation_requested(self.ctx.cancelled.clone(), self.ctx.cancellation.clone()) => {
                    return Err(AgentGraphError::Cancelled);
                }
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
