# Agent Collaboration Protocol and Implementation Plan

**Status:** Proposed, implementation-ready  
**Date:** 2026-08-08  
**Repository:** `agent-graph-mcp-release`  
**Protocol:** `agent_graph.collaboration.v1`  
**Primary owner:** Agent Graph daemon/runtime  
**First release posture:** disabled by default, run-scoped, read-only collaboration, no recursive orchestration

## 1. Decision

Build collaboration as a **daemon-mediated, run-scoped, typed mailbox and task protocol**.

Agents do not call one another directly and do not receive peer network handles. They submit bounded proposals to a trusted collaboration broker. The broker:

1. derives sender identity from the executing graph node and attempt;
2. validates the proposal against the pinned graph version and collaboration policy;
3. durably commits messages, task transitions, delivery rows, artifacts, and transition receipts;
4. returns bounded inbox/task projections to graph state;
5. references existing signed tool leases and receipts without transporting or minting authority;
6. preserves dissent, conflicts, late results, denials, and uncertainty as typed evidence.

The collaboration log in SQLite is canonical. `AgentState`, prompts, summaries, embeddings, UI views, and terminal collaboration summaries are projections.

### Why this shape

- Free-form peer chat makes identity, ordering, deduplication, authority, replay, and termination ambiguous.
- A single growing array in shared graph state conflicts with current value/output limits and multiplies across parallel state forks.
- Direct agent-to-agent tool calls would permit authority laundering, recursion explosions, circular waits, and duplicated effects.
- Existing graph nodes, routers, joins, leases, approvals, receipts, budgets, and cancellation remain the execution authority.

## 2. Source-grounded current state

### Verified in current source

- `GraphSpec` supports nodes, edges, reducers, bounded iterations, and bounded parallelism in `src/spec.rs`.
- Existing node classes include LLM, router, passthrough, state transform, join, parallel, subgraph, human approval, tool, and loop in `src/spec.rs`.
- `AgentState` is shared within a branch, deep-forked for parallel branches, and bounded by key/value/history/lock limits in `vendor/ri-agent-graph/src/state.rs`.
- A state value is capped at 1 MiB in the vendored core; the daemon additionally caps graph state/output in `src/spec.rs`.
- Reducers are last-write-wins, append, add, and merge. Graphs using reducers are currently outside the deterministic local resume subset in `src/spec.rs`.
- Parallel branches fork full state and merge through joins; collaboration must not require every branch to retain an unbounded mailbox.
- Graph events include run, node, token, interrupt, state update, superstep, and parallel cancellation variants in `vendor/ri-agent-graph/src/event_sink.rs`.
- The local event collector is bounded; durable `events` rows are written with the terminal projection in `src/store.rs`, not as a general live collaboration journal.
- SQLite already owns executions, checkpoints, events, terminal receipts, approvals, retention, and operator receipts in `src/store.rs` and `src/migrations.rs`.
- Current migration version is 4 in `src/migrations.rs`.
- Signed `ToolLease` already binds graph, graph version, run, node, expiry, allowlists, effect classes, recursion, depth, children, counters, and parent receipt digest in `src/tool_runtime.rs`.
- Existing effect classes are `ReadOnly`, `LocalMutation`, `ExternalEffect`, `AuthorityChange`, and `RecursiveOrchestration`.
- Tool reservations verify lease binding, effect classification, quotas, recursion cycles, argument digests, and receipt chains in `src/tool_runtime.rs`.
- `ToolNode` invokes the Hermes tool broker and reads lineage receipts in `src/nodes.rs`.
- Human approval currently interrupts execution and writes an approval request projection; durable approval authority is controlled outside model-facing MCP mutation tools.
- `RunManager` owns run lifecycle, budgets, cancellation, bounded events, terminal state, receipt, and persistence status in `src/run_manager.rs`.

### Evidence caveat

A three-lane Agent Graph council completed protocol, authority, and runtime advisory analyses in `run-19fe2e705d8-2`. Its original synthesis node did not receive joined input. A corrected synthesis run, `run-19fe2ece9ef-3`, failed at the 120-second LLM timeout. Those outputs are advisory only. This document is the controller reconciliation against inspected source.

## 3. Scope

### In scope

- run-scoped agent identity and membership;
- typed direct and topic messages;
- durable inbox delivery and acknowledgment;
- tasks, claims, releases, handoffs, review, completion, blocking, cancellation, and expiry;
- immutable bounded artifacts and references to existing receipts/witnesses;
- causal lineage, idempotency, ordering, and collision detection;
- bounded collaboration rounds and deterministic join inputs;
- first-class dissent and review outcomes;
- integration with existing tool leases, approvals, budgets, cancellation, and terminal receipts;
- read-only MCP inspection surfaces;
- crash consistency, recovery visibility, telemetry, resource limits, and rollback.

### Explicit non-goals for v1

- cross-run or cross-daemon messaging;
- open network peer discovery;
- agents minting identities, leases, approvals, tools, children, or graph versions;
- arbitrary peer RPC or agent-as-tool recursion;
- shared mutable files as coordination state;
- embedding raw secrets, credentials, lease tokens, or approval tokens in messages;
- automatically resolving dissent with majority vote;
- repeating uncertain external effects;
- claiming deterministic resume for LLM/reducer collaboration graphs before the core supports it;
- modifying vendored `ri-agent-graph` in the first vertical slice.

## 4. Trust and authority model

### Principals

- **Operator:** authenticated control-plane authority.
- **Daemon:** trusted identity, policy, persistence, scheduling, and receipt owner.
- **Agent member:** a graph-declared logical role mapped to one node ID.
- **Agent attempt:** one host-observed execution of that node.
- **Tool adapter:** capability-bearing executor governed by a signed tool lease.
- **Model output:** untrusted proposal data, never a principal.

### Hard rules

1. Sender identity is host-stamped from `(graph_id, graph_version, run_id, node_id, attempt)`.
2. A model proposal schema has no sender, run, signature, lease, approval, or authority fields.
3. Unknown proposal fields fail validation; they are not ignored.
4. A message may request work or an effect. It cannot authorize either.
5. Effective tool authority is the intersection of:
   - graph policy;
   - recipient node's static tool policy;
   - parent lease remaining scope and budget, if any;
   - effect policy;
   - exact approval binding, when required;
   - current cancellation and quota state.
6. Authority never uses a union of sender and recipient capabilities.
7. `AuthorityChange` is unavailable to agents.
8. `RecursiveOrchestration` is disabled in v1 and requires a separate gated phase.
9. External effects require an exact request digest, idempotency key, durable checkpoint, and operator approval by default.
10. Secrets remain adapter-owned references and never enter collaboration rows, graph state, prompts, events, or receipts.

## 5. Top-level graph contract

Add an optional collaboration policy to `GraphSpec`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationPolicySpec {
    pub protocol: String,                 // exactly agent_graph.collaboration.v1
    pub members: BTreeMap<String, CollaborationMemberSpec>,
    pub max_rounds: u32,
    pub max_messages: u32,
    pub max_message_bytes: usize,
    pub max_artifacts: u32,
    pub max_artifact_bytes: usize,
    pub max_inbox_batch: u32,
    pub message_ttl_ms: u64,
    #[serde(default)]
    pub topics: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationMemberSpec {
    pub node_id: String,
    #[serde(default)]
    pub subscriptions: Vec<String>,
}
```

Defaults for the first release:

- protocol disabled unless explicitly declared;
- 12 members maximum for the admitted direction-setting council;
- 8 rounds;
- 256 messages per run;
- 16 KiB message body;
- 32 artifacts;
- 64 KiB inline artifact;
- 16 messages per inbox fetch;
- 15-minute message TTL;
- no wildcard topic subscription;
- no tool grants in collaboration policy.

Validation must reject:

- duplicate agent IDs or node mappings;
- unknown node IDs;
- empty membership;
- invalid or wildcard recipients;
- limits outside hard daemon ceilings;
- a collaboration node when the top-level policy is absent;
- members mapped to `External` nodes;
- topic members not present in the member map;
- protocol versions other than the supported exact value.

Do not duplicate tool capability declarations here. Tool authority stays with existing tool policy/lease ownership.

## 6. Identity and proposals

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRef {
    pub agent_id: String,
    pub node_id: String,
    pub attempt: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageProposal {
    pub idempotency_key: String,
    pub kind: MessageKind,
    pub recipient: Recipient,
    pub task_id: Option<String>,
    pub round: u32,
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub causal_parent_ids: Vec<String>,
    pub body: Value,
    #[serde(default)]
    pub artifact_refs: Vec<ArtifactRefProposal>,
    pub expires_at: Option<DateTime<Utc>>,
}
```

The broker creates the admitted envelope:

```rust
pub struct CollaborationMessage {
    pub protocol: String,
    pub message_id: String,
    pub run_id: String,
    pub graph_id: String,
    pub graph_version: String,
    pub sequence: u64,
    pub sender: AgentRef,
    pub recipient: Recipient,
    pub kind: MessageKind,
    pub task_id: Option<String>,
    pub round: u32,
    pub in_reply_to: Option<String>,
    pub causal_parent_ids: Vec<String>,
    pub body: Value,
    pub body_digest: String,
    pub artifact_refs: Vec<ArtifactRef>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub disposition: MessageDisposition,
    pub previous_transition_digest: Option<String>,
    pub transition_digest: String,
    pub signature: String,
}
```

`message_id` is deterministic from the host-stamped identity, idempotency key, kind, recipient, task, round, causal parents, and redacted payload digest.

Same identity plus same idempotency key and digest returns the prior disposition. Same identity plus same idempotency key and a different digest fails with `COLLABORATION_IDEMPOTENCY_COLLISION`.

## 7. Message kinds

Initial enum:

- `TaskOffer`
- `TaskClaim`
- `TaskRelease`
- `WorkRequest`
- `WorkResult`
- `ReviewRequest`
- `ReviewResponse`
- `Challenge`
- `Dissent`
- `Handoff`
- `Blocked`
- `CancelRequest`
- `Complete`
- `Acknowledge`
- `RoundCloseProposal`

The host, not an agent message, decides authoritative cancellation, task ownership, round closure, and run termination.

### Dissent

Dissent is never overwritten by synthesis. It carries:

```rust
pub struct DissentBody {
    pub target_message_id: String,
    pub claim: String,
    pub reasons: Vec<String>,
    pub severity: DissentSeverity, // Informational, Material, Blocking
    pub evidence: Vec<ArtifactRef>,
    pub proposed_resolution: Option<String>,
}
```

A blocking dissent prevents task or round completion only when the pinned graph policy declares that gate. Otherwise it remains visible in the terminal receipt as unresolved dissent.

## 8. Tasks and state machine

Canonical task transitions:

```text
Offered -> Claimed -> InProgress -> ReviewRequested -> Completed
   |          |            |               |
   |          +-> Released -+               +-> InProgress (changes requested)
   +-> Cancelled
   +-> Expired
   +-> Blocked
Claimed/InProgress/ReviewRequested -> Cancelled | Blocked | Expired
```

Rules:

- Task state transitions occur in one SQLite transaction using expected version compare-and-swap.
- The first valid claim wins.
- A duplicate claim by the same attempt is idempotent.
- A competing claim receives a durable rejection receipt.
- Only the current owner may release, hand off, request review, or submit completion.
- Handoff is two-phase: owner proposes a named recipient; recipient accepts through claim CAS.
- Expired ownership must transition to `Released` or `Expired` before reassignment.
- Cancellation stops new claims and messages that could mutate task state.
- A task result references artifacts and tool receipts; it does not embed unrestricted tool output.
- A model-proposed `Complete` cannot terminate the run. The deterministic task/round controller admits completion.

## 9. Ordering and delivery

- SQLite assigns a monotonic `sequence` per run inside the write transaction.
- Topic recipients are expanded to concrete member deliveries at publish time using the pinned member snapshot.
- Inbox reads order by run sequence, then message ID.
- Delivery is at-least-once; logical processing is exactly-once through message ID/idempotency deduplication.
- Acknowledgment is explicit and idempotent.
- Cross-agent arrival order has no semantic authority.
- Joins sort accepted artifacts by `(round, sender.agent_id, message.sequence, message_id)`.
- Late messages are durably recorded with `LateAfterRound` or `LateAfterTerminal` disposition and cannot change terminal projections.

## 10. Artifacts

Artifacts are immutable and content-addressed.

```rust
pub struct CollaborationArtifact {
    pub artifact_id: String,
    pub run_id: String,
    pub digest: String,
    pub media_type: String,
    pub schema: Option<String>,
    pub size_bytes: usize,
    pub producer: AgentRef,
    pub source_message_id: Option<String>,
    pub source_tool_receipt_digest: Option<String>,
    pub content: Option<Value>,
    pub external_ref: Option<TypedArtifactReference>,
    pub created_at: DateTime<Utc>,
}
```

Rules:

- Inline JSON is capped at 64 KiB by default.
- Larger content must use a typed daemon-owned reference with known digest and verification method.
- Plain arbitrary filesystem paths and URLs are rejected as artifact authority.
- Existing tool receipt and witness IDs may be referenced; they remain owned by their canonical stores.
- Artifact content is immutable. Corrections create a new artifact with a `supersedes` relation.
- Secret-tainted or unredacted content is rejected before persistence.

## 11. Persistence model

Raise `CURRENT_VERSION` from 4 to 5 and add forward-only, additive tables.

```sql
CREATE TABLE collaboration_runs (
    run_id TEXT PRIMARY KEY,
    graph_id TEXT NOT NULL,
    graph_version TEXT NOT NULL,
    protocol TEXT NOT NULL,
    policy_json TEXT NOT NULL,
    policy_digest TEXT NOT NULL,
    next_sequence INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL,
    message_count INTEGER NOT NULL DEFAULT 0,
    artifact_count INTEGER NOT NULL DEFAULT 0,
    round_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    closed_at TEXT,
    FOREIGN KEY (run_id) REFERENCES executions(run_id)
);

CREATE TABLE collaboration_messages (
    run_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    message_id TEXT NOT NULL,
    sender_agent_id TEXT NOT NULL,
    sender_node_id TEXT NOT NULL,
    sender_attempt INTEGER NOT NULL,
    kind TEXT NOT NULL,
    recipient_json TEXT NOT NULL,
    task_id TEXT,
    round INTEGER NOT NULL,
    in_reply_to TEXT,
    causal_json TEXT NOT NULL,
    body_json TEXT NOT NULL,
    body_digest TEXT NOT NULL,
    artifact_refs_json TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    disposition TEXT NOT NULL,
    issued_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    previous_transition_digest TEXT,
    transition_digest TEXT NOT NULL,
    signature TEXT NOT NULL,
    PRIMARY KEY (run_id, sequence),
    UNIQUE (run_id, message_id),
    UNIQUE (run_id, sender_node_id, sender_attempt, idempotency_key),
    FOREIGN KEY (run_id) REFERENCES collaboration_runs(run_id)
);

CREATE TABLE collaboration_deliveries (
    run_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    recipient_agent_id TEXT NOT NULL,
    status TEXT NOT NULL,
    delivered_at TEXT NOT NULL,
    acknowledged_at TEXT,
    PRIMARY KEY (run_id, message_id, recipient_agent_id),
    FOREIGN KEY (run_id, message_id)
      REFERENCES collaboration_messages(run_id, message_id)
);

CREATE TABLE collaboration_tasks (
    run_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    state TEXT NOT NULL,
    version INTEGER NOT NULL,
    owner_agent_id TEXT,
    owner_node_id TEXT,
    owner_attempt INTEGER,
    lease_expires_at TEXT,
    offer_message_id TEXT NOT NULL,
    result_message_id TEXT,
    review_message_id TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (run_id, task_id),
    FOREIGN KEY (run_id) REFERENCES collaboration_runs(run_id)
);

CREATE TABLE collaboration_artifacts (
    run_id TEXT NOT NULL,
    artifact_id TEXT NOT NULL,
    digest TEXT NOT NULL,
    media_type TEXT NOT NULL,
    schema_name TEXT,
    size_bytes INTEGER NOT NULL,
    producer_json TEXT NOT NULL,
    source_message_id TEXT,
    source_tool_receipt_digest TEXT,
    content_json TEXT,
    external_ref_json TEXT,
    supersedes_artifact_id TEXT,
    created_at TEXT NOT NULL,
    PRIMARY KEY (run_id, artifact_id),
    UNIQUE (run_id, digest),
    FOREIGN KEY (run_id) REFERENCES collaboration_runs(run_id)
);

CREATE INDEX idx_collaboration_inbox
ON collaboration_deliveries(run_id, recipient_agent_id, status);

CREATE INDEX idx_collaboration_task_state
ON collaboration_tasks(run_id, state);
```

### Persistence invariants

- Message admission, sequence allocation, deliveries, task transition, counters, and transition receipt commit atomically.
- The collaboration append log is canonical; task and inbox rows are transactional projections.
- A projection rebuild from admitted messages must reproduce task ownership, deliveries, unresolved dissent, and digest roots.
- Migration failure does not advance schema version.
- Rollback never drops v5 tables. Older binaries must be prevented from opening a newer schema unless explicitly compatible.

## 12. Runtime ownership

### New module

Create `src/collaboration.rs` for:

- protocol/domain types;
- proposal validation;
- host identity stamping;
- message and artifact canonicalization;
- task transition rules;
- collaboration policy validation;
- broker trait and errors;
- receipt root calculation;
- bounded state projection types.

Do not implement a second persistence owner in this module.

### Existing owners

- `src/spec.rs`: optional `CollaborationPolicySpec`, member validation, `NodeType::Collaboration`.
- `src/store.rs`: canonical SQLite writes/reads, transactions, projection rebuild, terminal roots.
- `src/migrations.rs`: migration v5 only.
- `src/run_manager.rs`: create/close run collaboration context, cancellation binding, terminal summary, broker handle.
- `src/compiler.rs`: inject the trusted broker and compile collaboration nodes.
- `src/nodes.rs`: deterministic `CollaborationNode`; no database access and no identity fields supplied by state.
- `src/tool_runtime.rs`: reuse leases; add only narrowly required delegation-intersection helpers in the later tool phase.
- `src/server.rs`: read-only collaboration query methods and terminal projection exposure.
- `src/tools.rs`: typed MCP parameters for read-only inspection.
- `src/lib.rs`: module exports.
- `README.md`: protocol use, limits, authority model, example, and operational caveats.

Do not patch `vendor/` for phases 0–5.

## 13. Collaboration node

Add one deterministic node class with explicit operations:

```rust
pub enum CollaborationOperation {
    Publish { proposal_key: String, output_key: String },
    Receive { cursor_key: String, output_key: String, limit: u32 },
    Acknowledge { message_ids_key: String, output_key: String },
    ClaimTask { task_id_key: String, expected_version_key: String, output_key: String },
    TransitionTask { transition_key: String, output_key: String },
    PublishArtifact { artifact_key: String, output_key: String },
    RoundView { round_key: String, output_key: String },
}
```

`CollaborationNode` gets a broker handle plus immutable execution binding. It must not accept `sender_agent_id`, `run_id`, `graph_version`, a signature, lease, or approval from graph state.

Each operation writes only a bounded response projection to `AgentState`, such as:

```json
{
  "cursor": 14,
  "messages": [
    {
      "message_id": "...",
      "sender": "researcher",
      "kind": "work_result",
      "task_id": "T1",
      "round": 1,
      "body": {"summary": "..."},
      "artifact_refs": [{"artifact_id": "...", "digest": "sha256:..."}],
      "trust": "untrusted_agent_content"
    }
  ],
  "truncated": false
}
```

A recipient prompt must label this as untrusted peer content. It is never concatenated into system policy or treated as authority.

## 14. Tool-enabled collaboration

Tool execution remains a separate graph step.

Recommended flow:

```text
peer request -> deterministic validation -> scheduler decision
-> existing ToolNode under recipient-bound lease
-> tool receipt -> immutable artifact
-> WorkResult message referencing receipt/artifact
```

### Lease intersection

Add a pure function and tests before enabling delegated tools:

```rust
pub fn intersect_delegated_lease(
    parent: &SignedToolLease,
    recipient_policy: &NodeToolPolicy,
    request: &DelegationRequest,
    run_budget: &RemainingBudget,
    approval: Option<&ApprovedCheckpoint>,
) -> Result<SignedToolLease, ToolPolicyError>;
```

Required properties:

- tool allowlist can only narrow;
- effect ceiling can only narrow;
- expiry cannot exceed parent or run deadline;
- counters and child budget come from remaining parent budget;
- graph/run/version binding is unchanged;
- node binding changes only to the declared recipient;
- active recursion stack is extended and cycle-checked;
- external effects require exact approval digest and idempotency key;
- authority change always rejects;
- recursive orchestration rejects in v1;
- a collaboration message or artifact can never be used as a lease token.

## 15. Read-only MCP/API surface

Expose inspection only in the first release:

- `graph_collaboration_status(run_id)`
- `graph_collaboration_messages(run_id, cursor, limit, agent_id?, task_id?, kind?)`
- `graph_collaboration_tasks(run_id, state?, limit)`
- `graph_collaboration_artifact_get(run_id, artifact_id)`
- `graph_collaboration_verify(run_id)`

Limits:

- pagination is mandatory;
- raw secret-tainted fields are never returned;
- artifact reads enforce size and media/schema metadata;
- status reports protocol/policy digest, counts, root digests, unresolved dissent, and projection verification;
- these tools do not publish, claim, approve, issue leases, or mutate state;
- internal mutation APIs require an execution-bound broker handle unavailable through generic MCP dispatch.

## 16. Terminal receipt extension

Add an additive `collaboration` section:

```json
{
  "protocol": "agent_graph.collaboration.v1",
  "policy_digest": "sha256:...",
  "members_digest": "sha256:...",
  "message_root": "hmac-sha256:...",
  "task_root": "hmac-sha256:...",
  "artifact_root": "hmac-sha256:...",
  "counts": {
    "messages": 12,
    "tasks": 1,
    "artifacts": 3,
    "denied": 1,
    "late": 0,
    "dissent": 1
  },
  "unresolved_dissent_ids": ["..."],
  "incomplete_deliveries": [],
  "verification": "verified"
}
```

The terminal receipt references collaboration roots. It does not duplicate all message bodies. Collaboration verification recomputes row digests and projections from SQLite.

Current graph receipts advertise structural/integrity limitations. Collaboration must not claim stronger evidence authority than the underlying terminal receipt and store actually provide.

## 17. Smallest real vertical slice

Use three declared members:

- `researcher`
- `reviewer`
- `synthesizer`

Graph sequence:

```text
seed_task
  -> researcher_receive
  -> researcher_llm
  -> researcher_publish_artifact
  -> researcher_publish_result
  -> reviewer_receive
  -> reviewer_llm
  -> reviewer_publish_review_or_dissent
  -> synthesizer_receive
  -> synthesizer_llm
  -> synthesizer_publish_completion_proposal
  -> deterministic_round_close
  -> END
```

Acceptance:

1. Task `T1` is durably offered.
2. Researcher claims `T1` through CAS.
3. Researcher publishes one immutable artifact and one result referencing it.
4. Reviewer receives exactly one logical result and publishes a review or dissent.
5. Synthesizer receives the admitted result and review/dissent.
6. Deterministic controller closes the task/round; model text alone cannot close it.
7. Terminal receipt exposes member, message, task, artifact, and dissent roots.
8. Replaying a publish idempotency key creates no duplicate effect.
9. Restart after terminal state preserves and verifies the collaboration log.
10. No tool execution is enabled in this first slice.

### 17.1 Twelve-agent unfinished-work direction council

The first production-shaped collaboration graph has exactly twelve stable member identities. Its purpose is to determine the highest-leverage next direction from unfinished work. The controller/judge is external to the twelve and retains final authority.

Members:

1. `source_truth_auditor` — reconciles current files, branches, builds, deployed state, receipts, and stale claims.
2. `unfinished_work_mapper` — inventories incomplete, blocked, abandoned, and partially integrated work.
3. `dependency_critical_path` — maps prerequisites, blockers, sequencing, and shortest credible path to proof.
4. `architecture_integrator` — identifies canonical owners, integration seams, duplication, and shadow-truth risk.
5. `product_roi_analyst` — ranks user value, leverage, reuse, cost, reversibility, and time to proof.
6. `reliability_security_auditor` — finds correctness, safety, credential, authority, recovery, and operational risks.
7. `test_release_auditor` — evaluates verification gaps, migration readiness, CI, release, and rollback evidence.
8. `performance_resource_analyst` — evaluates compute, memory, latency, concurrency, and operating-cost envelopes.
9. `evidence_claims_auditor` — separates verified, observed, inferred, proposed, blocked, and public-claim-safe states.
10. `portfolio_consolidator` — finds reusable primitives, project combinations, dead ends, and consolidation opportunities.
11. `contrarian_red_team` — challenges consensus, searches for omitted options, and writes the strongest dissent.
12. `council_synthesizer` — builds the candidate decision matrix and final council recommendation without deleting dissent.

All twelve receive the same bounded evidence manifest, but only tool results and artifact references admitted by the broker become council evidence. Initial tool leases are read-only. No agent may modify a repository, issue a lease, approve an effect, spawn another agent, or widen the evidence scope silently.

Council rounds:

1. **Reconciliation:** agents 1–11 publish source-grounded findings, candidate directions, blockers, confidence, and explicit unknowns. Agent 12 publishes an independent proposal before seeing peer conclusions.
2. **Cross-examination:** each member reviews two deterministically assigned peers in a ring. Every material challenge references a message, claim, artifact, or receipt digest.
3. **Candidate matrix:** the broker produces a deterministic matrix across leverage, evidence confidence, reversibility, cost, time to proof, dependency burden, safety, and compounding value.
4. **Synthesis:** agent 12 recommends one primary direction, one bounded fallback, the first experiment, acceptance gates, rollback, and unresolved dissent.
5. **Controller judgment:** the external judge independently inspects the council artifact and canonical evidence. The judge returns `accept`, `accept_with_changes`, `quarantine`, or `reject`; the council cannot self-certify.

Required output contract:

```rust
pub struct DirectionRecommendation {
    pub evidence_cutoff: DateTime<Utc>,
    pub inventory_digest: String,
    pub primary_direction: DirectionCandidate,
    pub fallback_direction: DirectionCandidate,
    pub ranked_alternatives: Vec<DirectionCandidate>,
    pub first_experiment: ProofExperiment,
    pub acceptance_gates: Vec<AcceptanceGate>,
    pub rollback: RollbackPlan,
    pub unresolved_dissent: Vec<DissentRef>,
    pub blocked_unknowns: Vec<Unknown>,
    pub council_receipt_digest: String,
}
```

Every candidate must include exact source references, current evidence state, expected leverage, estimated cost band, reversibility, dependencies, time-to-proof band, failure conditions, and the smallest next gate. Numeric scores may sort candidates but cannot override failed evidence or safety gates.

External judgment rubric:

- source coverage and freshness;
- support for every material claim;
- no conflation of source-reported and locally reproduced results;
- canonical ownership and dependency correctness;
- leverage and compounding ROI;
- feasibility and bounded time to proof;
- reversibility and rollback quality;
- security, authority, and external-effect safety;
- preservation of dissent and uncertainty;
- a concrete first experiment with rerunnable acceptance gates.

The judge must reject or quarantine an output that invents completion, hides contradictions, depends on unavailable authority, proposes an unbounded rewrite, silently widens scope, or lacks a falsifiable first experiment.

## 18. Implementation phases

### Phase 0 — branch and scope gate

**Goal:** establish a clean implementation lane without absorbing unrelated workspace changes.

- Start from commit `ba9fba16c6093e5410f2f54613fb254dc5819248` or a verified newer `origin/main`.
- Use a dedicated branch/worktree.
- Preserve unrelated changes currently present in `Cargo.toml`, `src/main.rs`, `src/proxy.rs`, `src/spec.rs`, `tests/mcp_integration.rs`, `.hermes/`, and `vendor/` until individually reconciled.
- Record baseline tests and database schema version.

Gate:

```bash
git status --short --branch
cargo test --lib
cargo test --test mcp_integration -- --test-threads=1
cargo test --test daemon_recovery -- --test-threads=1
git diff --check
```

Stop if the clean branch cannot reproduce baseline behavior.

### Phase 1 — domain contracts, validation, and state machine

**Files:**

- Create `src/collaboration.rs`.
- Update `src/lib.rs`.
- Update `src/spec.rs` only for optional policy types and validation.
- Create `tests/collaboration_protocol.rs`.

RED tests first:

- unknown protocol/version rejects;
- agent IDs map uniquely to existing nodes;
- proposal rejects host-owned fields;
- unknown member/recipient rejects;
- message body/parents/artifacts/TTL exceed bounds;
- deterministic message ID is stable;
- same idempotency key plus different digest collides;
- invalid task transitions reject;
- first claim wins and duplicate winner is idempotent;
- non-owner completion/release rejects;
- blocking dissent gate behaves exactly as configured;
- terminal/late messages cannot mutate projections.

GREEN gate:

```bash
cargo test --test collaboration_protocol -- --test-threads=1
cargo check --lib
git diff --check
```

No SQLite, nodes, MCP tools, or provider calls in this phase.

### Phase 2 — SQLite migration and canonical store

**Files:**

- Update `src/migrations.rs` to version 5.
- Update `src/store.rs` with collaboration row types and transactions.
- Create `tests/collaboration_store.rs`.
- Extend `tests/daemon_recovery.rs` with v5 crash fixtures.

RED tests first:

- fresh database creates all v5 tables/indexes;
- migration 4 -> 5 is atomic and idempotent;
- interrupted migration does not advance version;
- older active executions remain valid;
- publish transaction atomically allocates sequence, message, deliveries, task transition, counters, and receipt;
- concurrent claims produce one owner;
- duplicate publish returns the prior disposition;
- idempotency collision leaves no mutation;
- message sequence is monotonic per run;
- projection rebuild matches live task/delivery projections;
- digest/signature tampering is detected;
- database busy/failure returns typed persistence failure and no partial acknowledgment;
- cancelled/terminal run rejects state-changing collaboration writes;
- additive tables survive rollback to collaboration-disabled operation.

GREEN gate:

```bash
cargo test --test collaboration_store -- --test-threads=1
cargo test --test daemon_recovery collaboration -- --test-threads=1
cargo test --lib store:: -- --test-threads=1
```

### Phase 3 — runtime broker and deterministic node

**Files:**

- Update `src/run_manager.rs` with run collaboration context.
- Update `src/compiler.rs` with broker injection.
- Update `src/nodes.rs` with `CollaborationNode`.
- Update `src/spec.rs` with `NodeType::Collaboration` and config validation.
- Create `tests/collaboration_runtime.rs`.

RED tests first:

- graph without policy cannot compile collaboration node;
- model/state-supplied sender/run/lease fields reject;
- broker stamps current node and attempt;
- cross-run, cross-version, wrong-node operations reject;
- receive projection is bounded and cursor-based;
- topic expansion is pinned and deterministic;
- cancellation between validation and commit prevents admission;
- state projection contains no signature, lease, approval, or secret;
- parallel publish permutations produce canonical join input;
- collaboration rows remain canonical when branch state forks;
- collaboration node cannot execute a tool.

GREEN gate:

```bash
cargo test --test collaboration_runtime -- --test-threads=1
cargo test --lib compiler:: -- --test-threads=1
cargo test --lib nodes:: -- --test-threads=1
```

### Phase 4 — three-agent read-only vertical slice

**Files:**

- Add a fixture/template under the repository's canonical template owner.
- Add `tests/collaboration_three_agent.rs`.
- Update `README.md` with the exact graph.

Use deterministic/mock LLM outputs in CI. Run one live provider demonstration only after mock conformance passes.

Acceptance gates are the ten vertical-slice conditions in section 17 plus:

- no tool broker process starts;
- all peer content is labeled untrusted;
- terminal collaboration roots verify;
- SQLite projection survives daemon restart;
- existing non-collaboration graphs remain byte-compatible at normalized spec/output boundaries.

Commands:

```bash
cargo test --test collaboration_three_agent -- --test-threads=1
cargo test --test mcp_integration collaboration -- --test-threads=1
cargo test --test daemon_recovery collaboration -- --test-threads=1
```

### Phase 5 — claims, reviews, dissent, handoff, and rounds

**Files:** reuse the owners established above; do not add a competing scheduler.

RED tests first:

- claim race and release/reassignment;
- two-phase handoff;
- review changes requested returns task to in-progress;
- blocking dissent prevents configured close;
- non-blocking dissent remains in receipt;
- quorum/all-members policies are deterministic under every delivery permutation;
- missing, denied, expired, and late members remain visible;
- deadlocked round fails visibly instead of succeeding;
- max rounds/messages/artifacts stop progress with typed budget errors;
- no-progress loop breaker terminates boundedly.

Gate:

```bash
cargo test --test collaboration_protocol rounds -- --test-threads=1
cargo test --test collaboration_runtime rounds -- --test-threads=1
```

### Phase 6 — read-only tool work with leases

**Files:**

- Update `src/tool_runtime.rs` only after lease-intersection RED tests exist.
- Integrate existing `ToolNode`; do not let collaboration node dispatch tools.
- Add `tests/collaboration_tool_authority.rs`.

First allowed mode: recipient executes an existing `ReadOnly` tool under its own/static intersected lease and publishes an artifact plus receipt reference.

RED security tests:

- message cannot carry or mint lease;
- sender's broader authority does not widen recipient;
- read-only request cannot become local/external mutation;
- forged parent receipt or lease digest rejects;
- wrong graph/run/version/node binding rejects;
- expired/revoked lease rejects;
- concurrent calls cannot exceed lease counters;
- child budget cannot be charged twice;
- tool output instructions cannot change policy;
- result without verified receipt cannot claim executed evidence;
- cancellation before dispatch prevents call;
- cancellation after uncertain external dispatch is never labeled success.

Gate:

```bash
cargo test --test collaboration_tool_authority -- --test-threads=1
cargo test --lib tool_runtime:: -- --test-threads=1
cargo test --test tool_nodes -- --test-threads=1
```

Do not enable `LocalMutation`, `ExternalEffect`, or `RecursiveOrchestration` yet.

### Phase 7 — approval-gated effects

Add effects one class at a time:

1. local mutation limited to run-owned workspace/state;
2. external effect with exact digest, idempotency, checkpoint, and operator approval;
3. recursive orchestration only as a separate later decision.

Required tests:

- approval for request digest A cannot authorize B;
- changed recipient/path/payload invalidates approval;
- approval max uses is one;
- model cannot approve;
- pending approval survives restart;
- mutation pre-image/checkpoint is durable before commit;
- uncertain external result is not retried automatically;
- compensation is never claimed without verification;
- authority change always rejects;
- recursive orchestration remains disabled unless independently admitted.

This phase is blocked until durable approval and checkpoint behavior is proven for the collaboration graph shape. The current deterministic resume subset excludes LLM/reducer graphs; do not claim resumable effect workflows before that gap is closed.

### Phase 8 — inspection, verification, and operations

**Files:**

- Update `src/tools.rs`.
- Update `src/server.rs`.
- Extend `tests/mcp_integration.rs`.
- Add operator/read-only documentation.

Tests:

- pagination and hard limits;
- filters cannot escape run scope;
- verification detects gaps, collisions, tampering, and projection drift;
- raw secrets and lease material never render;
- mutation methods are absent from model-facing MCP tools;
- terminal and collaboration roots agree;
- metrics cardinality is bounded.

### Phase 9 — load, crash, and release proof

Controlled envelopes:

1. 3 members, 1 task, 1 round, 8 messages;
2. 12 members, 12 tasks, 4 rounds, 256 messages;
3. crash injection during publish, claim, artifact commit, cancellation, and terminal projection.

Measure:

- p50/p95 publish transaction latency;
- inbox query latency;
- projection rebuild time;
- SQLite size/event growth;
- peak cgroup memory and process RSS;
- graph state bytes per branch;
- cancellation latency;
- rejected/duplicate/late counts;
- OOM/high/max events and service restarts;
- post-run worker/process cleanup.

Acceptance:

- no correctness change under scheduling permutations;
- no message/task/artifact limit bypass;
- no orphaned tool process;
- no duplicate committed effect;
- collaboration projection rebuild matches canonical rows;
- twelve-member run stays inside an explicitly approved memory envelope;
- existing non-collaboration conformance suite remains green.

Full gate:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --lib
cargo test --tests -- --test-threads=1
cargo clippy --all-targets -- -D warnings
git diff --check
```

If current unrelated warnings block `-D warnings`, record them as baseline and require no new warnings in changed files; do not silently widen scope.

## 19. Failure semantics

Stable top-level classes:

- `COLLABORATION_DISABLED`
- `COLLABORATION_PROTOCOL_UNSUPPORTED`
- `COLLABORATION_POLICY_INVALID`
- `COLLABORATION_IDENTITY_MISMATCH`
- `COLLABORATION_RECIPIENT_UNKNOWN`
- `COLLABORATION_SCHEMA_INVALID`
- `COLLABORATION_LIMIT_EXCEEDED`
- `COLLABORATION_IDEMPOTENCY_COLLISION`
- `COLLABORATION_CAUSAL_PARENT_MISSING`
- `COLLABORATION_TASK_CONFLICT`
- `COLLABORATION_TASK_NOT_OWNER`
- `COLLABORATION_ROUND_CLOSED`
- `COLLABORATION_RUN_TERMINAL`
- `COLLABORATION_ARTIFACT_INVALID`
- `COLLABORATION_AUTHORITY_DENIED`
- `COLLABORATION_CANCELLED`
- `COLLABORATION_PERSISTENCE_FAILED`
- `COLLABORATION_INTEGRITY_FAILED`

Failures return typed, redacted data and receive denial/transition receipts. Persistence or integrity uncertainty fails closed.

## 20. Telemetry

Low-cardinality metrics:

- admitted, denied, duplicate, collided, late messages;
- task transitions and conflicts;
- inbox batch size and query latency;
- publish transaction latency;
- artifact bytes and rejects;
- unresolved dissent count;
- round duration and close reason;
- collaboration state projection bytes;
- store verification failures;
- tool requests by effect class and outcome;
- approval waits;
- cancellation latency.

Trace fields:

- run ID;
- graph/version;
- node/attempt;
- agent ID;
- protocol;
- message/task/artifact IDs;
- sequence and round;
- disposition/error code;
- digests, never raw secret-bearing body content.

Do not use agent IDs, task IDs, message IDs, or artifact IDs as metric labels when that creates unbounded cardinality. Keep them in traces/receipts.

## 21. Rollout and rollback

### Rollout

1. Merge domain/store code with collaboration disabled.
2. Enable only deterministic read-only fixtures in CI.
3. Enable one local three-agent live graph.
4. Run restart/integrity verification.
5. Run three consecutive twelve-member read-only envelopes.
6. Enable read-only tool evidence for a pinned test graph.
7. Admit mutation/effect phases only through separate approvals and release gates.

### Rollback

- Disable collaboration admission in daemon configuration.
- Pin new runs to graph versions without collaboration nodes.
- Cancel active collaboration runs through normal run cancellation.
- Revoke active tool leases.
- Preserve v5 rows, receipts, artifacts, and terminal evidence.
- Do not downgrade the database by dropping collaboration tables.
- Restore the prior binary only if it is schema-aware or opens the store read-only/refuses safely.
- Mark any already-dispatched external effect `Indeterminate` unless independently verified or compensated.

Rollback trigger examples:

- identity spoofing or cross-run access;
- lease widening;
- secret material in messages/state/events/receipts;
- projection/root mismatch;
- duplicate non-idempotent effect;
- collaboration persistence acknowledged before durable commit;
- cancellation cannot stop an uncommitted action;
- loop/round/message bounds bypassed;
- resource envelope exceeded materially;
- old graphs change behavior.

## 22. Definition of done

The collaboration system is complete only when:

- protocol, policy, message, task, artifact, and receipt schemas are versioned and documented;
- daemon-stamped identity and deny-unknown proposal validation are proven;
- SQLite append/task/delivery/artifact transactions survive race and crash tests;
- bounded inbox projections work across parallel state forks;
- the three-agent vertical slice runs end-to-end with deterministic mock agents;
- one live provider run is recorded as source-reported execution, not conflated with mock conformance;
- task claims, handoff, review, dissent, rounds, cancellation, and late messages pass permutation tests;
- read-only tool collaboration cannot widen authority and carries verified receipts;
- mutation/external effects remain gated until their independent phase passes;
- terminal collaboration roots verify against canonical rows;
- non-collaboration graphs pass existing tests unchanged;
- migration, deployment, rollback, limits, telemetry, and incident behavior are documented;
- changed files, commands, results, skipped checks, unresolved risks, and rerunnable verification are captured in closeout receipts.

## 23. First implementation gate

Start with **Phases 0–2 only**:

1. clean branch/worktree;
2. pure collaboration types and state machine under RED/GREEN tests;
3. additive SQLite v5 migration and atomic canonical store;
4. no runtime node, provider, tool, MCP mutation, or deployment yet.

This is the smallest slice that proves the contracts and concurrency semantics before model behavior or tool authority can obscure defects.
