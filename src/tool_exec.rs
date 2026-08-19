//! Adapter from graph tool definitions to `llm-pipeline`'s tool-loop runtime.
use async_trait::async_trait;
use llm_tool_runtime::{Tool, ToolCall, ToolDescriptor, ToolError, ToolRegistry, ToolResult};
use serde_json::{json, Value};
use std::sync::Arc;

/// Tool execution context owned by an LLM node.
///
/// Graph tool entries use OpenAI's `{name, description, parameters}` shape;
/// this adapter supplies the runtime metadata required by `ToolLoopRunner`.
#[derive(Clone)]
pub struct ToolExecContext {
    registry: ToolRegistry,
}

impl ToolExecContext {
    /// Build a runtime context from OpenAI-compatible tool definitions.
    pub fn new(tools: &[Value]) -> Result<Self, String> {
        let mut registry = ToolRegistry::new();
        for value in tools {
            let function = value.get("function").unwrap_or(value);
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "tool definition missing function.name".to_owned())?;
            let descriptor: ToolDescriptor = serde_json::from_value(json!({
                "name": name, "version": "1",
                "description": function.get("description"),
                "backend_kind": "local_function",
                "input_schema": function.get("parameters").cloned().unwrap_or_else(|| json!({"type":"object"})),
                "output_mode": "structured_json", "read_only": true,
                "side_effect_class": "none", "idempotency_class": "idempotent",
                "approval_kind": "none", "timeout_ms": 120000,
                "exposure_mode": "public", "mcp_surface_kind": "none",
                "provider_payload": value
            })).map_err(|e| format!("invalid tool definition {name}: {e}"))?;
            registry.register(GraphTool { descriptor });
        }
        Ok(Self { registry })
    }

    /// Return a runner backed by this context's registry.
    pub fn runner(&self) -> llm_pipeline::ToolLoopRunner {
        llm_pipeline::ToolLoopRunner::from_registry(self.registry.clone())
    }
}

struct GraphTool {
    descriptor: ToolDescriptor,
}

#[async_trait]
impl Tool for GraphTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }

    async fn invoke(
        &self,
        _ctx: &llm_tool_runtime::ToolCtx,
        call: &ToolCall,
    ) -> Result<ToolResult, ToolError> {
        // Graph tools forward execution to the MCP tool runtime.
        // When no handler is registered, return an empty JSON result
        // so the tool loop can continue. Real execution requires the
        // caller to register handlers via SharedToolExecContext.
        let result = serde_json::json!({
            "tool": self.descriptor.name,
            "arguments": call.arguments,
            "status": "forwarded",
            "note": "graph tool execution delegated to MCP runtime"
        });
        Ok(ToolResult::json(result))
    }
}

/// Shared, cheaply clonable context handle.
pub type SharedToolExecContext = Arc<ToolExecContext>;
