use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Runtime configuration for graph execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Thread ID for checkpointing and state management
    pub thread_id: Option<String>,
    /// Trace ID for cross-crate correlation.
    /// If None, one is generated automatically at execution start.
    ///
    /// ## Phase status: compatibility / migration-only
    ///
    /// This field uses `String` for backward compatibility. The canonical
    /// replacement is [`trace_ctx`](Self::trace_ctx). Use
    /// [`trace_ctx()`](Self::trace_ctx) to obtain the canonical form.
    ///
    /// **Removal condition**: removed when all callers migrate to `trace_ctx`.
    pub trace_id: Option<String>,
    /// Canonical trace context for cross-crate correlation.
    ///
    /// When set, takes precedence over the legacy `trace_id` field.
    /// If both are `None`, a new `TraceCtx` is generated at execution start.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trace_ctx: Option<stack_ids::TraceCtx>,
    /// Maximum recursion depth (default: 100)
    pub recursion_limit: usize,
    /// Maximum number of parallel nodes in a single superstep.
    /// Hard-capped at 100. Default: 8.
    pub max_parallelism: usize,
    /// Tags for filtering and organization
    pub tags: Vec<String>,
    /// Metadata attached to the execution
    pub metadata: HashMap<String, Value>,
    /// User-provided configurable values accessible by nodes
    pub configurable: HashMap<String, Value>,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            thread_id: None,
            trace_id: None,
            trace_ctx: None,
            recursion_limit: 100,
            max_parallelism: 8,
            tags: Vec::new(),
            metadata: HashMap::new(),
            configurable: HashMap::new(),
        }
    }
}

impl GraphConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_thread_id(mut self, id: impl Into<String>) -> Self {
        self.thread_id = Some(id.into());
        self
    }

    pub fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.recursion_limit = limit;
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn with_configurable(mut self, key: impl Into<String>, value: Value) -> Self {
        self.configurable.insert(key.into(), value);
        self
    }

    pub fn with_trace_id(mut self, id: impl Into<String>) -> Self {
        self.trace_id = Some(id.into());
        self
    }

    pub fn with_max_parallelism(mut self, n: usize) -> Self {
        self.max_parallelism = n.clamp(1, 100);
        self
    }

    /// Resolve the canonical `stack_ids::TraceCtx` for this config.
    ///
    /// Resolution order:
    /// 1. `self.trace_ctx` if set (canonical path).
    /// 2. `self.trace_id` converted via `TraceCtx::from_legacy_trace_id` (compat path).
    /// 3. Generates a new `TraceCtx` if neither is set.
    pub fn resolve_trace_ctx(&self) -> stack_ids::TraceCtx {
        if let Some(ref ctx) = self.trace_ctx {
            return ctx.clone();
        }
        match &self.trace_id {
            Some(id) => stack_ids::TraceCtx::from_legacy_trace_id(id),
            None => stack_ids::TraceCtx::generate(),
        }
    }

    /// Set the canonical trace context.
    ///
    /// Also back-fills the legacy `trace_id` field for compat consumers.
    pub fn with_trace_ctx(mut self, ctx: stack_ids::TraceCtx) -> Self {
        self.trace_id = Some(ctx.to_legacy_trace_id().to_string());
        self.trace_ctx = Some(ctx);
        self
    }
}
