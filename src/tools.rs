//! Tool parameter structs for agent-graph MCP tools.
//! Each struct derives schemars::JsonSchema so rmcp auto-generates
//! the JSON Schema for the tool's inputSchema.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── Shared typed enums ────────────────────────────────────────────────

/// Valid approval decisions.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Reject,
    RequestChanges,
    Escalate,
}

/// Valid render formats for graph visualization.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    Mermaid,
    Json,
}

// ─── Graph lifecycle ──────────────────────────────────────────────────

/// Parameters for creating, validating, or deleting a graph.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphCreateParams {
    /// JSON graph specification. Required for 'create' and 'validate' actions.
    #[serde(default)]
    pub spec: Option<Value>,
    /// Action: 'create' (register graph), 'validate' (validate without registering), 'delete' (remove graph).
    #[serde(default)]
    pub action: Option<String>,
    /// Graph ID (name) — used for delete action.
    #[serde(default)]
    pub graph_id: Option<String>,
    /// Optional idempotency key. Reusing a key returns the existing result.
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// Template instantiation: { "id": "council", "name": "my-council" }
    #[serde(default)]
    pub template: Option<Value>,
    /// When true, overwrite an existing graph with the same name.
    #[serde(default)]
    pub overwrite: Option<bool>,
}

/// Parameters for listing registered graphs.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphListParams {
    /// Optional filter: only show graphs whose name contains this string.
    #[serde(default)]
    pub query: Option<String>,
    /// Maximum number of graphs to return (default 50).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Parameters for getting a specific graph's details.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphInspectParams {
    /// The graph ID (name) to inspect.
    pub graph_id: String,
}

/// Read-only durable lifecycle inventory. Existing graphs without a record appear as `unclassified`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRetentionReviewParams {
    /// Restrict the report to a specific graph ID.
    #[serde(default)]
    pub graph_id: Option<String>,
    /// Optional lifecycle-state filter.
    #[serde(default)]
    pub state: Option<String>,
    /// Maximum report entries (default 100, maximum 256).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Set one explicit lifecycle state. Deletion requires delete_candidate followed by delete_approved.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRetentionSetParams {
    /// Graph ID to classify.
    pub graph_id: String,
    /// One of active, pinned, archived, delete_candidate, or delete_approved.
    pub state: String,
    /// Why this lifecycle transition is appropriate.
    pub reason: String,
    /// Operator or service principal recording the decision.
    pub actor: String,
    /// Optional ISO-8601 review deadline.
    #[serde(default)]
    pub review_after: Option<String>,
}

/// Parameters for deleting a graph.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphDeleteParams {
    /// The graph ID (name) to delete.
    pub graph_id: String,
}

// ─── Execution ────────────────────────────────────────────────────────

/// Parameters for executing a graph.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphExecuteParams {
    /// The graph ID (name) to execute.
    pub graph_id: String,
    /// Input value to pass to the graph's entry node.
    #[serde(default)]
    pub input: Option<Value>,
    /// Optional pinned graph version hash.
    #[serde(default)]
    pub graph_version: Option<String>,
    /// Optional thread ID for checkpointing (future use).
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Execution mode: 'sync' blocks until completion, 'async' returns immediately.
    #[serde(default)]
    pub mode: Option<String>,
    /// Optional idempotency key. Reusing a key returns the existing run result.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

// ─── Status ───────────────────────────────────────────────────────────

/// Parameters for querying server or execution status.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphStatusParams {
    /// Resource type: 'server', 'graph', 'run', 'events', 'receipt', 'templates'.
    /// Omit for server-level summary.
    #[serde(default)]
    pub resource: Option<String>,
    /// Graph ID (required when resource='graph').
    #[serde(default)]
    pub graph_id: Option<String>,
    /// Run ID (required when resource='run', 'events', or 'receipt').
    #[serde(default)]
    pub run_id: Option<String>,
    /// Event cursor (for resource='events', start from this sequence number).
    #[serde(default)]
    pub cursor: Option<u64>,
    /// Maximum events to return (for resource='events', default 100).
    #[serde(default)]
    pub limit: Option<u64>,
}

// ─── Structured output ────────────────────────────────────────────────

/// Standard response envelope for all tools.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StructuredOutput {
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Human-readable status.
    #[serde(default)]
    pub status: Option<String>,
    /// Primary response data.
    #[serde(default)]
    pub data: Option<Value>,
    /// Error message (only present when ok=false).
    #[serde(default)]
    pub error: Option<String>,
    /// Stable error code (when applicable).
    #[serde(default)]
    pub error_code: Option<String>,
    /// Graph ID (when applicable).
    #[serde(default)]
    pub graph_id: Option<String>,
    /// Graph version / digest.
    #[serde(default)]
    pub graph_version: Option<String>,
    /// Run ID (when applicable).
    #[serde(default)]
    pub run_id: Option<String>,
}

// ─── Approval lifecycle ────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequestParams {
    /// The immutable deterministic-local checkpoint to which this approval is bound.
    pub checkpoint_id: String,
    /// Human audience label; this is metadata and grants no execution authority.
    pub audience: String,
    /// Approval prompt. It is stored only as a digest and is never returned by approval reads.
    pub prompt: String,
    /// Non-empty subset of `approve` and `reject`.
    pub allowed_decisions: Vec<String>,
    /// RFC3339 expiration timestamp.
    pub expiration: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovalListParams {
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovalGetParams {
    pub approval_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovalDecideParams {
    pub approval_id: String,
    pub decision: String,
    /// Caller-provided label is metadata only; it is never an authority identity.
    #[serde(alias = "actor")]
    pub claimed_actor_label: String,
}

// ─── Async run lifecycle ───────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunStartParams {
    pub graph_id: String,
    #[serde(default)]
    pub input: Option<Value>,
    #[serde(default)]
    pub graph_version: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub budgets: Option<Value>,
    /// When true, persist an intentional deterministic pre-execution checkpoint
    /// and leave the run paused until graph_run_resume consumes it.
    #[serde(default)]
    pub checkpoint: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunWaitParams {
    pub run_id: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunCancelParams {
    pub run_id: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunGetParams {
    pub run_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunStateParams {
    pub run_id: String,
    #[serde(default)]
    pub checkpoint_id: Option<String>,
    #[serde(default)]
    pub json_pointer: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunEventsParams {
    pub run_id: String,
    #[serde(default)]
    pub cursor: Option<u64>,
    #[serde(default)]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunReceiptParams {
    pub run_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunCheckpointParams {
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub checkpoint_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunResumeParams {
    #[serde(default)]
    pub checkpoint_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
}

// ─── Local source witnesses ──────────────────────────────────────────

/// Caller-supplied local capture. This endpoint never dereferences the locator.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WitnessCaptureParams {
    pub locator: String,
    pub content: String,
    pub media_type: String,
    pub authority_class: String,
    #[serde(default)]
    pub retrieved_at: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WitnessGetParams {
    pub witness_id: String,
}

// ─── Policy + render ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PolicyCheckParams {
    pub graph_id: String,
    #[serde(default)]
    pub input: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderParams {
    pub graph_id: String,
    #[serde(default)]
    pub format: Option<String>,
}

// ─── Templates ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateListParams {
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateInstantiateParams {
    pub template_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateCandidatesParams {
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateOutcomesParams {
    pub template_id: String,
}
