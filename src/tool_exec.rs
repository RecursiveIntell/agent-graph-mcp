//! Adapter boundary for graph tool definitions.
//!
//! Tool execution is deliberately fail-closed until this crate has an
//! owner-authorized handler registry. A parsed function call is not an effect.
use llm_tool_runtime::ToolRegistry;
use serde_json::Value;
use std::sync::Arc;

/// Tool execution context owned by an LLM node.
///
/// The graph currently has no authorized handler bridge for LLM-supplied tools.
/// Non-empty tool definitions are rejected before provider execution.
#[derive(Clone)]
pub struct ToolExecContext {
    registry: ToolRegistry,
}

impl ToolExecContext {
    /// Build a runtime context only when no executable tools were supplied.
    pub fn new(tools: &[Value]) -> Result<Self, String> {
        if !tools.is_empty() {
            return Err(
                "UNSUPPORTED_TOOL_EXECUTION: no authorized handler is registered".to_owned(),
            );
        }
        Ok(Self {
            registry: ToolRegistry::new(),
        })
    }

    /// Return a runner backed by this context's registry.
    pub fn runner(&self) -> llm_pipeline::ToolLoopRunner {
        llm_pipeline::ToolLoopRunner::from_registry(self.registry.clone())
    }
}

/// Shared, cheaply clonable context handle.
pub type SharedToolExecContext = Arc<ToolExecContext>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn llm_tool_context_rejects_unhandled_tools() {
        let definition = json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write a file",
                "parameters": {"type": "object"}
            }
        });

        let error = ToolExecContext::new(&[definition])
            .err()
            .expect("unhandled tool accepted");
        assert_eq!(
            error,
            "UNSUPPORTED_TOOL_EXECUTION: no authorized handler is registered"
        );
    }
}
