//! Payload trait and PayloadNode for integrating external work units.
//!
//! The [`Payload`] trait is the boundary between the graph orchestrator and
//! external execution logic (e.g., LLM calls from `llm-pipeline`).
//! The graph runtime never implements payload logic — it only orchestrates execution.

use crate::command::NodeOutput;
use crate::config::GraphConfig;
use crate::error::AgentGraphError;
use crate::node::Node;
use crate::state::AgentState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Error type for payload invocations.
pub type PayloadError = Box<dyn std::error::Error + Send + Sync>;

/// Token callback type for streaming.
pub type TokenCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// Function that extracts payload input from graph state.
pub type InputSelector = Box<dyn Fn(&Value) -> Value + Send + Sync>;

/// Function that maps payload output back into graph state.
pub type OutputMapper = Box<dyn Fn(&Value, &PayloadOutput) -> Value + Send + Sync>;

/// Output from a payload execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadOutput {
    /// The primary output value.
    pub value: Value,
    /// Additional metadata from the execution (e.g., token counts, latency).
    #[serde(default)]
    pub meta: HashMap<String, Value>,
}

/// Context passed to payloads during execution.
///
/// Provides a token sink for streaming and metadata about the current execution.
pub struct PayloadContext {
    /// Callback for streaming tokens. Called by the payload during execution.
    /// The runtime sets this up to forward tokens to the [`EventSink`](crate::event_sink::EventSink).
    pub on_token: Option<TokenCallback>,
    /// The current run ID.
    pub run_id: String,
    /// The current node ID.
    pub node_id: String,
}

/// A unit of work that can be executed by the graph runtime.
///
/// This is the canonical interface between the orchestrator and external
/// payload implementations (e.g., `llm-pipeline`). Implementors provide
/// the actual computation; the graph runtime handles scheduling, checkpointing,
/// and event emission.
///
/// Uses boxed futures instead of async-trait.
pub trait Payload: Send + Sync {
    /// Execute this payload with the given input.
    ///
    /// - `input`: extracted from graph state via `input_selector`.
    /// - `ctx`: provides token streaming callback and execution metadata.
    fn invoke(
        &self,
        input: Value,
        ctx: &PayloadContext,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<PayloadOutput, PayloadError>> + Send + '_>>;
}

/// A node that wraps a [`Payload`] for execution within the graph.
///
/// `input_selector` extracts what to pass to the payload from graph state.
/// `output_mapper` folds the payload output back into graph state.
///
/// Defaults: pass entire state as input; replace entire state with `output.value`.
pub struct PayloadNode {
    name: Option<String>,
    payload: Box<dyn Payload>,
    /// Extracts node input from the current state (as JSON object).
    /// `fn(state_value) -> payload_input`
    /// Default: passes the entire state.
    input_selector: Option<InputSelector>,
    /// Maps the payload output back into state.
    /// `fn(current_state, payload_output) -> new_state`
    /// Default: deep-merge output.value into state.
    output_mapper: Option<OutputMapper>,
}

impl PayloadNode {
    /// Create a new PayloadNode wrapping a payload.
    pub fn new(payload: Box<dyn Payload>) -> Self {
        Self {
            name: None,
            payload,
            input_selector: None,
            output_mapper: None,
        }
    }

    /// Set a name for this node (for debugging and events).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the input selector function.
    ///
    /// Receives the entire state as a JSON value and returns the value
    /// to pass to the payload's `invoke` method.
    pub fn with_input_selector(
        mut self,
        selector: impl Fn(&Value) -> Value + Send + Sync + 'static,
    ) -> Self {
        self.input_selector = Some(Box::new(selector));
        self
    }

    /// Set the output mapper function.
    ///
    /// Receives the current state and the payload output, returns the new state.
    pub fn with_output_mapper(
        mut self,
        mapper: impl Fn(&Value, &PayloadOutput) -> Value + Send + Sync + 'static,
    ) -> Self {
        self.output_mapper = Some(Box::new(mapper));
        self
    }
}

#[async_trait::async_trait]
impl Node for PayloadNode {
    async fn execute(
        &self,
        state: &AgentState,
        _config: &GraphConfig,
    ) -> crate::Result<NodeOutput> {
        // Build state value
        let state_data = state.export().await;
        let state_value = serde_json::to_value(&state_data)
            .map_err(|e| AgentGraphError::StateError(e.to_string()))?;

        // Select input
        let input = match &self.input_selector {
            Some(selector) => selector(&state_value),
            None => state_value.clone(),
        };

        // Build payload context (no token sink in basic execution path;
        // the GraphExecutor sets this up when EventSink is configured)
        let ctx = PayloadContext {
            on_token: None,
            run_id: String::new(),
            node_id: self.name.clone().unwrap_or_default(),
        };

        // Invoke payload
        let output = self
            .payload
            .invoke(input, &ctx)
            .await
            .map_err(|e| AgentGraphError::PayloadError(e.to_string()))?;

        // Map output back to state
        let new_state_value = match &self.output_mapper {
            Some(mapper) => mapper(&state_value, &output),
            None => {
                // Default: merge output.value into state
                merge_value(&state_value, &output.value)
            }
        };

        // Apply the new state
        if let Value::Object(map) = new_state_value {
            for (key, value) in map {
                state.set_raw(&key, value).await?;
            }
        }

        Ok(NodeOutput::Done)
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Default merge: if output is an object, merge keys into state.
/// Otherwise, replace the entire state with the output.
fn merge_value(state: &Value, output: &Value) -> Value {
    match (state, output) {
        (Value::Object(s), Value::Object(o)) => {
            let mut result = s.clone();
            for (k, v) in o {
                result.insert(k.clone(), v.clone());
            }
            Value::Object(result)
        }
        _ => output.clone(),
    }
}

impl std::fmt::Debug for PayloadNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayloadNode")
            .field("name", &self.name)
            .finish()
    }
}
