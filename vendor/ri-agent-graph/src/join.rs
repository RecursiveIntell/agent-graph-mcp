//! JoinNode for deterministic fan-in merging.
//!
//! When parallel branches converge, a [`JoinNode`] provides explicit
//! merge logic. It reads specified keys from state (set by parallel branches),
//! applies a merge function, and writes the result to an output key.

use crate::command::NodeOutput;
use crate::config::GraphConfig;
use crate::error::AgentGraphError;
use crate::node::Node;
use crate::state::AgentState;
use serde_json::Value;

/// Merge function signature for JoinNode.
pub type MergeFn = Box<dyn Fn(Vec<(String, Value)>) -> crate::Result<Value> + Send + Sync>;

/// A node that merges results from parallel branches.
///
/// After fan-out, parallel branches write their results to known state keys.
/// The JoinNode reads those keys, applies a merge function, and writes
/// the merged result to an output key.
pub struct JoinNode {
    name: Option<String>,
    /// State keys to read from parallel branches.
    input_keys: Vec<String>,
    /// State key to write the merged result to.
    output_key: String,
    /// Merge function: receives `Vec<(key, value)>` and produces the merged value.
    merge_fn: MergeFn,
}

impl JoinNode {
    /// Create a new JoinNode.
    ///
    /// - `input_keys`: state keys to collect from parallel branches.
    /// - `output_key`: state key to write the merged result to.
    /// - `merge_fn`: function that merges the collected values.
    pub fn new(
        input_keys: Vec<String>,
        output_key: impl Into<String>,
        merge_fn: impl Fn(Vec<(String, Value)>) -> crate::Result<Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: None,
            input_keys,
            output_key: output_key.into(),
            merge_fn: Box::new(merge_fn),
        }
    }

    /// Set a name for this node.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Convenience: create a JoinNode that collects values into an array.
    pub fn collect_array(input_keys: Vec<String>, output_key: impl Into<String>) -> Self {
        Self::new(input_keys, output_key, |values| {
            let arr: Vec<Value> = values.into_iter().map(|(_, v)| v).collect();
            Ok(Value::Array(arr))
        })
    }

    /// Convenience: create a JoinNode that merges objects (shallow).
    pub fn merge_objects(input_keys: Vec<String>, output_key: impl Into<String>) -> Self {
        Self::new(input_keys, output_key, |values| {
            let mut result = serde_json::Map::new();
            for (key, value) in values {
                if let Value::Object(map) = value {
                    for (k, v) in map {
                        result.insert(k, v);
                    }
                } else {
                    result.insert(key, value);
                }
            }
            Ok(Value::Object(result))
        })
    }
}

#[async_trait::async_trait]
impl Node for JoinNode {
    async fn execute(
        &self,
        state: &AgentState,
        _config: &GraphConfig,
    ) -> crate::Result<NodeOutput> {
        // Collect values from input keys
        let mut inputs = Vec::new();
        for key in &self.input_keys {
            let Some(value) = state.get_opt::<Value>(key).await? else {
                return Err(AgentGraphError::ExecutionError(format!(
                    "JoinNode: required input key '{key}' is missing"
                )));
            };
            if value.is_null() {
                return Err(AgentGraphError::ExecutionError(format!(
                    "JoinNode: required input key '{key}' is null"
                )));
            }
            inputs.push((key.clone(), value));
        }

        // Apply merge function
        let merged = (self.merge_fn)(inputs)?;

        // Write to output key
        state.set_raw(&self.output_key, merged).await?;

        Ok(NodeOutput::Done)
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

impl std::fmt::Debug for JoinNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinNode")
            .field("name", &self.name)
            .field("input_keys", &self.input_keys)
            .field("output_key", &self.output_key)
            .finish()
    }
}
