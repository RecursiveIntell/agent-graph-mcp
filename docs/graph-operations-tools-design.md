# Graph operations MCP tools design

These 12 tools use the existing rmcp convention:

```rust
#[tool(description = "...")]
fn tool_name(
    &self,
    Parameters(params): Parameters<ToolParams>,
) -> Result<Json<StructuredOutput>, ErrorData>
```

All responses use the existing `StructuredOutput` envelope. Successful `data` payloads are described below; failures should use stable `error_code` values and retain the envelope's `ok: false` shape.

## Shared typed enums

These enums are intentionally string-compatible in the wire schema while preventing invalid values in Rust. Add `serde`'s `rename_all = "snake_case"` and `schemars::JsonSchema` derives.

```rust
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Reject,
    RequestChanges,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    Mermaid,
    Json,
}
```

## 1. `graph_approval_list`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphApprovalListParams {
    /// Optional run ID; when omitted, list approvals across all retained runs.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Optional status filter. Defaults to `pending`.
    #[serde(default)]
    pub status: Option<String>,
    /// Maximum number of records, default 100 and hard-capped by the server.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Pagination cursor returned by the previous call.
    #[serde(default)]
    pub cursor: Option<String>,
}
```

Description: `List human-approval requests, optionally filtered by run ID and status, with bounded pagination.`

Implementation: query `PersistentStore`'s `approval_requests` table when configured; otherwise use an approval index owned by `RunManager` (the current runtime has no approval index, so add one). Return `data: { approvals: [...], next_cursor, truncated }`, including approval ID, run ID, node ID, audience, prompt, allowed decisions, status, expiry, and decision metadata. Do not expose unrelated run state.

## 2. `graph_approval_get`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphApprovalGetParams {
    /// Approval request identifier.
    pub approval_id: String,
}
```

Description: `Fetch one approval request by approval_id, including its current status and decision metadata.`

Implementation: read the approval row from `PersistentStore`; volatile mode should look up the run manager's approval registry. Return `data: { approval: {...} }`. Missing IDs should return `APPROVAL_NOT_FOUND`; expired pending requests should be reported as `expired`, not silently converted to rejection.

## 3. `graph_approval_decide`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphApprovalDecideParams {
    /// Approval request identifier.
    pub approval_id: String,
    /// Decision to apply: approve, reject, or request_changes.
    pub decision: ApprovalDecision,
    /// Human-readable rationale or requested changes.
    #[serde(default)]
    pub decision_note: Option<String>,
    /// Stable caller/principal identity recorded in the audit trail.
    pub decided_by: String,
    /// Optional idempotency key for safe retries.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}
```

Description: `Record an approval decision atomically and resume or terminate the associated run according to the decision.`

Implementation: validate the approval is pending, unexpired, and that the decision is in `allowed_decisions`; atomically update the store row with decision, note, decided_by, and timestamp. Then signal the run's approval/resume mechanism. `approve` resumes, `reject` terminates, and `request_changes` returns the run to a waiting/rework state. Use `APPROVAL_ALREADY_DECIDED`, `APPROVAL_EXPIRED`, and `APPROVAL_DECISION_NOT_ALLOWED` as stable errors. Persist before signaling to make retries restart-safe.

## 4. `graph_run_start`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRunStartParams {
    /// Registered graph ID/name.
    pub graph_id: String,
    /// Input passed to the graph entry node.
    #[serde(default)]
    pub input: Option<Value>,
    /// Optional exact graph topology/version digest to pin.
    #[serde(default)]
    pub graph_version: Option<String>,
    /// Optional caller correlation/thread identifier.
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Optional idempotency key; retries return the original run.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}
```

Description: `Start asynchronous execution of a registered graph and return its run_id without waiting for completion.`

Implementation: resolve and version-check `self.graphs`, enforce input limits, check `store` idempotency, allocate/admit in `self.runs`, persist an execution row, and call `RunManager::start`. Return `data: { run_id, status: "accepted", graph_id, graph_version, thread_id }` immediately. Capacity failure must not leave an accepted orphan run.

## 5. `graph_run_wait`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRunWaitParams {
    /// Run identifier.
    pub run_id: String,
    /// Maximum wait in milliseconds; default 30_000, hard-capped by policy.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Poll interval in milliseconds; server may clamp this value.
    #[serde(default)]
    pub poll_interval_ms: Option<u64>,
}
```

Description: `Wait until a run reaches a terminal state or the bounded timeout expires.`

Implementation: repeatedly read `self.runs.get(run_id)` (and durable execution state if enabled) without holding the outer mutex across sleeps. Terminal states are `completed`, `failed`, and `cancelled`; timeout returns `ok: true` with `status: "timeout"` and current run data, not a false terminal result. Return `data: { run, terminal, timed_out }`.

## 6. `graph_run_cancel`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRunCancelParams {
    /// Run identifier.
    pub run_id: String,
    /// Why cancellation was requested; retained in the audit/event record.
    pub reason: String,
    /// Optional idempotency key for repeated cancellation requests.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}
```

Description: `Request cooperative cancellation of a running graph execution.`

Implementation: call `self.runs.cancel`, which sets the cancellation flag observed at node boundaries; append a cancellation-request event and update durable execution status if configured. Repeated calls are idempotent. Return `data: { run_id, status: "cancellation_requested", reason, effective_at_boundary: true }`. Reject terminal runs with `RUN_ALREADY_TERMINAL` or return their unchanged terminal status consistently.

## 7. `graph_run_get`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRunGetParams {
    /// Run identifier.
    pub run_id: String,
    /// Include the full state/final output when true; default false for bounded status reads.
    #[serde(default)]
    pub include_state: Option<bool>,
    /// Include approval records associated with this run; default true.
    #[serde(default)]
    pub include_approvals: Option<bool>,
}
```

Description: `Get current run status, resource/budget usage, and pending approvals.`

Implementation: read `RunManager::get`; merge durable execution/checkpoint counters where available. Derive budget usage from recorded node/attempt/token counters (or return explicit `unknown` fields rather than inventing values). Include `pending_approvals` from the approval store/index. Return `data: { run_id, status, success, graph_id, graph_version, started_at, finished_at, usage, pending_approvals, state?, final_state? }`.

## 8. `graph_run_state`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRunStateParams {
    /// Run identifier.
    pub run_id: String,
    /// Optional JSON-pointer projection, e.g. `/user/result`.
    #[serde(default)]
    pub path: Option<String>,
    /// If true, include state history/checkpoint metadata when available.
    #[serde(default)]
    pub include_history: Option<bool>,
}
```

Description: `Read the current state projection for a run, optionally selecting a JSON-pointer path.`

Implementation: use the live `RunRecord.state`; if absent in memory, load the latest durable checkpoint/state projection from `PersistentStore`. Apply RFC 6901 JSON Pointer and return `STATE_PATH_NOT_FOUND` for an invalid path. Return `data: { run_id, cursor/state_version, value, path, history? }`; redact secrets using the existing evidence redaction helper.

## 9. `graph_run_events`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRunEventsParams {
    /// Run identifier.
    pub run_id: String,
    /// Inclusive event cursor from which to replay.
    #[serde(default)]
    pub cursor: Option<u64>,
    /// Maximum events, default 100 and hard-capped at 200.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Wait briefly for events when the cursor is at the live tail.
    #[serde(default)]
    pub wait_ms: Option<u64>,
}
```

Description: `Replay run events from an inclusive cursor with restart-safe pagination and gap detection.`

Implementation: use `RunManager::events` for volatile records; use the durable `events(run_id, seq)` table when configured, preferring durable records after restart. Preserve `next_cursor`, `gap`, `truncated`, and `dropped` semantics. Return `data: { run_id, events, next_cursor, gap, truncated, dropped }`; never reuse a cursor after compaction without reporting `gap: true`.

## 10. `graph_run_receipt`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRunReceiptParams {
    /// Run identifier.
    pub run_id: String,
    /// If true, include the evidence bundle/artifact reference as well as the receipt.
    #[serde(default)]
    pub include_bundle: Option<bool>,
    /// Optional digest expected by the caller for integrity verification.
    #[serde(default)]
    pub expected_digest: Option<String>,
}
```

Description: `Fetch the canonical execution receipt for a run and optionally verify its evidence bundle digest.`

Implementation: read `RunRecord.receipt` or the durable receipt artifact. Compare `expected_digest` against the canonical serialized receipt/bundle digest; mismatch returns `RECEIPT_DIGEST_MISMATCH`. Return `data: { receipt, bundle?, integrity: { verified, digest } }`; missing receipts for nonterminal runs should be `RECEIPT_NOT_READY`, not an empty success.

## 11. `graph_policy_check`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphPolicyCheckParams {
    /// Graph ID to check, unless an inline spec is supplied.
    #[serde(default)]
    pub graph_id: Option<String>,
    /// Inline graph spec for pre-registration checks.
    #[serde(default)]
    pub spec: Option<Value>,
    /// Optional policy profile name; defaults to the server's baseline policy.
    #[serde(default)]
    pub policy_profile: Option<String>,
    /// Optional caller-provided policy overrides, subject to server allowlisting.
    #[serde(default)]
    pub policy: Option<Value>,
}
```

Description: `Run a fail-closed preflight policy check against a registered graph or inline graph specification.`

Implementation: resolve exactly one source (`graph_id` xor `spec`), call `parse_and_validate`, then evaluate node types, graph size/iterations/parallelism, model allowlists, prompt/input limits, approval requirements, and unsupported capabilities. Return `data: { decision: "allow"|"deny"|"review", graph_id?, graph_version?, violations, warnings, effective_policy }`. A missing policy profile or unknown override must deny rather than silently weaken policy.

## 12. `graph_render`

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphRenderParams {
    /// Registered graph ID/name, unless an inline spec is supplied.
    #[serde(default)]
    pub graph_id: Option<String>,
    /// Inline graph spec to render without registration.
    #[serde(default)]
    pub spec: Option<Value>,
    /// Output representation: `mermaid` or `json`.
    pub format: RenderFormat,
    /// Include node prompts/configuration in JSON output; default false.
    #[serde(default)]
    pub include_details: Option<bool>,
}
```

Description: `Render a registered or inline graph as Mermaid or a normalized JSON topology.`

Implementation: resolve one source, validate inline specs through `parse_and_validate`, and use the existing `AgentGraphServer::mermaid` helper for Mermaid. JSON should expose only normalized topology by default: `{ graph_id, graph_version, entry, nodes, edges, topology_hash }`; add prompt/config details only when requested and redact secrets. Return `RENDER_SOURCE_REQUIRED` when neither source is supplied and `RENDER_SOURCE_AMBIGUOUS` when both are supplied.

## Cross-cutting implementation notes

- Add all parameter structs and enums to `src/tools.rs`; import `serde::{Deserialize, Serialize}` as needed and keep `Value` for extensible policy/graph payloads.
- Register methods inside the existing `#[tool_router] impl AgentGraphServer`; each method returns `Result<Json<StructuredOutput>, ErrorData>`.
- Validate IDs, enum values, body sizes, and limits before acquiring long-lived locks.
- Prefer durable store reads when `self.store.is_some()`, but keep volatile mode explicit in response metadata.
- Approval lifecycle requires a real in-memory approval index and run wake/resume primitive; the existing schema alone does not provide those semantics.
- Current `RunManager` has status/state/events/receipt/cancel primitives, but no timestamps, usage counters, durable receipt loading, or wait notification. Add those as implementation prerequisites rather than fabricating fields in tool responses.
