# Provenance-Bound Hermes Tool Nodes Implementation Plan

> **For Hermes:** Implement task-by-task under RED/GREEN discipline. Do not commit or deploy without independent review.

**Goal:** Make Agent Graph `tool` nodes execute a real Hermes worker with the same dynamic tool catalog as a normal Hermes session, while provenance leases, durable receipts, cycle detection, budgets, and approval policy bound recursion and side effects.

**Architecture:** Agent Graph remains the deterministic orchestrator. A `tool` node launches `hermes chat -q -Q`—never `hermes --oneshot`—with a daemon-generated lineage lease and an isolated receipt directory. A Hermes plugin intercepts `pre_tool_call` and `post_tool_call`: it verifies the lease, atomically reserves lineage budget, classifies recursive/effectful calls, fails closed when policy or receipt persistence is unavailable, and writes an append-only hash chain. The Rust node accepts output only when a terminal worker receipt verifies against the lease and receipt chain.

**Tech Stack:** Rust 2021, Tokio process execution, serde/serde_json, SHA-256/HMAC, SQLite terminal projection, Python 3 Hermes plugin hooks, pytest, cargo test.

---

## Current evidence

- Isolated worktree: `/home/sikmindz/.cache/agent-graph-tool-runtime-20260801`
- Branch: `feat/provenance-tool-nodes-20260801`
- Baseline HEAD: `4ead448308d7f09dfe9116ac8becf05d0af2334f`
- Candidate diff was replicated from `/home/sikmindz/.cache/agent-graph-capacity-20260731`; source and isolated tracked-diff SHA-256 both equal `d320dd4a9fca143b721a62f04d7543e4db343fba2d2330532f87a2d84aa0df1d` at isolation time.
- `src/spec.rs` already declares `NodeType::Tool` but `GraphSpec::executable_node_type` rejects it.
- `src/compiler.rs` rejects `NodeType::Tool` at compilation.
- Hermes canonical tool catalog is `model_tools.get_tool_definitions`; canonical dispatch is the full `AIAgent` loop. `todo`, `memory`, `session_search`, and `delegate_task` are agent-loop tools and cannot be reached by plain `registry.dispatch`.
- Hermes `--oneshot` says approvals are auto-bypassed; it is forbidden for this runtime. `hermes chat -q -Q` is the worker surface.
- Hermes plugin hooks `pre_tool_call` and `post_tool_call` cover registry-dispatched and agent-loop-owned tools.

## Hard invariants

1. **Full catalog, bounded authority:** Worker agents can see their normal configured tool catalog. Visibility never implies unconditional execution authority.
2. **Hermes owns tool semantics:** Agent Graph never reimplements or directly dispatches Hermes tools.
3. **No `--oneshot`:** The worker command must reject `--oneshot` and `-z` configurations.
4. **Fail closed:** Missing, malformed, expired, unverifiable, or exhausted leases block all tool calls.
5. **Receipt before effect:** A pre-call reservation is durably appended before the tool executes. If reservation persistence fails, execution is blocked.
6. **Terminal closure:** Rust accepts worker success only with a valid terminal receipt matching graph/run/node/attempt/lease/output digests.
7. **Lineage budget:** All descendants share a lineage ID and atomically consume common budgets.
8. **Recursive calls are explicit:** `delegate_task`, `cronjob`, `execute_code`, Agent Graph execution/start/resume tools, and tool-search bridge invocation of those tools consume recursive budget and are cycle-checked.
9. **No silent widening:** Child processes inherit the same or narrower lease. A model cannot raise tool, effect, depth, call, wall-clock, or child-count limits.
10. **Human approval remains distinct:** Effectful/external/authority-changing tools require a valid approval capability or are blocked. A model-produced string is never approval.
11. **Replay-safe calls:** Call IDs bind graph version, run, node, attempt, tool, argument digest, parent receipt digest, and lease digest. Replays return the existing terminal receipt or a typed incomplete/indeterminate state; they do not execute twice.
12. **Secrets excluded:** Receipts store argument/result digests and redacted summaries, not raw credentials or unrestricted tool output.
13. **Tool output is untrusted:** Worker output cannot mutate policy/lease fields in graph state.
14. **Live rollout is separate:** Passing isolated tests does not authorize replacing the production daemon or enabling the plugin.

## Recursion policy

- `max_graph_depth`: maximum nested Agent Graph lineage depth.
- `max_agent_depth`: maximum Hermes worker/delegate lineage depth.
- `max_tool_calls`: total calls across the lineage.
- `max_recursive_calls`: total calls to orchestration-capable tools across the lineage.
- `max_children`: total worker/delegate children across the lineage.
- `max_wall_clock_ms`: lineage deadline measured from signed lease issuance.
- `active_stack`: ordered digests of graph/worker/tool identities. Re-entry of the same identity without an explicit loop allowance returns `RECURSION_CYCLE_DETECTED`.
- Default for unattended graph workers: recursive tools visible but blocked (`max_recursive_calls = 0`). A separately issued operator lease may raise this within hard daemon ceilings.
- `cronjob create/update/resume/run`, Agent Graph run/start/resume, `delegate_task`, and `execute_code` are recursive/effectful. Read-only cron/list or graph/list/get operations may be separately classified but still consume tool-call budget.

## Phase 1 — Policy and receipt primitives

### Task 1.1: RED tests for lease parsing and fail-closed policy

**Files:**
- Create: `tests/tool_runtime.rs`
- Create: `src/tool_runtime.rs`
- Modify: `src/lib.rs`

**RED cases:** expired lease; missing HMAC; wrong graph/run/node binding; tool not granted; effect class not granted; exhausted tool count; exhausted recursive count; cycle in active stack; attempted scope widening.

**GREEN:** typed `ToolLease`, `ToolInvocation`, `ToolEffect`, `ToolDecision`, and deterministic digest/HMAC verification functions. No process execution yet.

**Gate:** `cargo test -p agent-graph-mcp --test tool_runtime`

### Task 1.2: RED tests for receipt chain

**Files:**
- Modify: `tests/tool_runtime.rs`
- Modify: `src/tool_runtime.rs`

**RED cases:** wrong parent digest; mismatched arguments; wrong attempt; missing pre-call reservation; forged terminal result; reordered receipts; redaction failure.

**GREEN:** `ToolCallIntent`, `ToolCallReceipt`, `WorkerTerminalReceipt`, canonical serialization, digest/HMAC verification, and bounded redacted summaries.

**Gate:** focused test plus `cargo test -p agent-graph-mcp --lib`.

## Phase 2 — Hermes lineage plugin

### Task 2.1: Build plugin in repository fixture

**Files:**
- Create: `integrations/hermes-agent-graph-lineage/plugin.yaml`
- Create: `integrations/hermes-agent-graph-lineage/__init__.py`
- Create: `integrations/hermes-agent-graph-lineage/lineage.py`
- Create: `integrations/hermes-agent-graph-lineage/tests/test_lineage.py`

**RED cases:** plugin absent outside graph worker environment; bad lease blocks; atomic budget reservation; concurrent callers cannot overspend; recursive tool detection includes direct and `mcp__agent_graph__*` names; `tool_call` bridge resolves underlying recursive name; failure to append intent blocks; post-call receipt closes reserved call; secrets are not persisted.

**GREEN:** register `pre_tool_call`, `post_tool_call`, `on_session_start`, and `on_session_finalize`. Operate only when `AGENT_GRAPH_LINEAGE_LEASE_PATH` is set. Use an OS-locked append-only JSONL ledger under a daemon-created private directory.

**Gate:** run plugin tests using the active Hermes Python environment.

### Task 2.2: Add worker-terminal closure

**Files:**
- Modify: plugin files/tests.

**GREEN:** terminal receipt binds session, graph, run, node, attempt, output digest, tool-call chain head, counters, status, and timestamp. Incomplete reservations produce `indeterminate`, never success.

## Phase 3 — Executable Tool node

### Task 3.1: Validate `tool` node config

**Files:**
- Modify: `src/spec.rs`
- Modify: `tests/mcp_integration.rs`

**Config fields:** `prompt_key`, `output_key`, `cwd`, optional `toolsets`, optional `skills`, `timeout_ms`. The graph may narrow but not widen daemon policy. Command path and hard ceilings are daemon-owned and absent from graph JSON.

**RED:** missing keys, absolute/relative cwd policy violations, forbidden `--oneshot`, graph-supplied executable path, toolset widening, executable tool node while runtime disabled.

### Task 3.2: Compile and run Hermes worker

**Files:**
- Modify: `src/compiler.rs`, `src/nodes.rs`, `src/run_manager.rs`, `Cargo.toml`
- Modify: daemon/server wiring and tests.

**GREEN:** `ToolNode` receives a daemon-owned `ToolRuntime`, creates a lease/receipt directory, starts `hermes chat -q -Q --source tool --max-turns N`, forwards cancellation/deadline, captures bounded stdout/stderr, verifies terminal receipt, writes only declared output/receipt state keys, and removes secret-bearing environment from public state.

**Gate:** fake-worker integration tests, cancellation, timeout, oversized output, missing receipt, forged receipt, nonzero exit, and valid success.

### Task 3.3: Add real Hermes smoke test

**Files:**
- Create: `tests/hermes_tool_worker.rs` or a gated shell fixture.

**Smoke:** install plugin into a temporary `HERMES_HOME`, run a deterministic local/read-only worker tool, verify intent + terminal receipts and output. No network/model dependency in canonical tests; an opt-in live test may exercise the configured model.

## Phase 4 — Durable graph receipts and restart behavior

### Task 4.1: Persist tool lineage projection

**Files:**
- Modify: `src/store.rs`, `src/migrations.rs`, `src/run_manager.rs`, tests.

**GREEN:** terminal graph receipt includes tool lineage ID, lease digest, tool receipt chain head, counters, and incomplete-call disposition. Raw arguments/results stay outside SQLite unless explicitly retained under a bounded replay mode.

### Task 4.2: Restart/replay gates

**RED:** daemon restart with reserved-but-unclosed call; duplicate call ID; worker completed but graph crashed before state update; lease expired during restart; receipt chain mismatch.

**GREEN:** replay returns verified prior result when safe, otherwise `TOOL_EFFECT_INDETERMINATE` and requires human/operator reconciliation.

## Phase 5 — Cleanup graph and rollout

### Task 5.1: Create tool-capable cleanup graph

Read-only default graph stages:

```text
catalog/probe → worktree discovery → parallel Git/file inventory
→ evidence join → LLM classification → hostile review
→ human approval → scoped mutation worker → independent verification
```

The initial deployed graph receives only read-only tools and `max_recursive_calls = 0`. Mutation graph versions are registered separately and require explicit approval.

### Task 5.2: Verification gauntlet

- `cargo fmt --check` on touched files.
- `cargo test -p agent-graph-mcp --tests --no-fail-fast`.
- `cargo check -p agent-graph-mcp --bin agent-graph-mcpd`.
- Python plugin tests.
- Isolated daemon + fake worker smoke.
- Isolated daemon + temporary-Hermes real read-only smoke.
- `git diff --check`.
- Secret scan of changed files.

### Task 5.3: Staged rollout

No live replacement until:

1. candidate/source diff is reviewed;
2. plugin install is explicit and config-gated;
3. protected backup exists;
4. daemon and plugin versions/digests are recorded;
5. read-only cleanup graph passes against a disposable repository;
6. live cleanup graph starts in review-only mode;
7. rollback restores prior binary/config/plugin state.

## Claim boundary

Passing Phase 3 licenses: “Agent Graph can run a provenance-bound Hermes worker with its normal tool catalog in an isolated test.”

It does **not** license: unrestricted autonomy, safe arbitrary side effects, production deployment, perfect recursion prevention across uninstrumented external processes, or deletion/commit/push authority.
