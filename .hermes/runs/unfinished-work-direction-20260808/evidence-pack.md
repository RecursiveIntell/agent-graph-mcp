# Unfinished Work Evidence Pack

Generated from live local repositories for the 12-agent direction council.

## User decision context

- Near-term goal from canonical USER.md: start a business and/or get a good job in the next 30–60 days.
- Working preference: evidence-led, local-first, reproducible, bounded, rollback-aware systems.
- Council task: choose the highest-leverage next direction from unfinished work; do not equate dirty state with value or completion.

## Portfolio inventory

- Live scan: 41 Git repositories under `/home/sikmindz/Coding`; 13 reported dirty at capture time.
- This pack intentionally samples 12 active/relevant workstreams. Omitted repositories are missing evidence, not negative evidence.

## Workstream 1: /home/sikmindz/Coding/agent-graph-mcp-release

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
M Cargo.toml
 M src/main.rs
 M src/proxy.rs
 M src/spec.rs
 M tests/mcp_integration.rs
?? .hermes/plans/agent-collaboration-protocol-20260808.md
?? .hermes/plans/luna-next-path-load-envelope-20260808.md
?? .hermes/receipts/
?? .hermes/runs/
?? vendor/
```
- Diff stat:
```text
Cargo.toml               |  1 +
 src/main.rs              | 18 +++++++++++-------
 src/proxy.rs             |  3 +++
 src/spec.rs              |  4 ++--
 tests/mcp_integration.rs |  9 ++++++---
 5 files changed, 23 insertions(+), 12 deletions(-)
```
- Five latest commits:
```text
2026-08-08T14:41:32-05:00	ba9fba1	fix: bound and isolate concurrent Codex workers
2026-08-06T21:03:44-05:00	aaaa52f	feat: token accounting — record provider prompt/completion/total tokens in LLM invocation receipts (local llm-pipeline via [patch.crates-io])
2026-08-06T20:41:44-05:00	9761331	feat: forward-compatible token-usage hook in LLM invocation records (TODO gate for llm-pipeline upgrade; spec: [REDACTED]
2026-08-06T17:39:55-05:00	13258e3	feat: operator DecideApproval action, operator.sock served by daemon, approval-aware resume (APPROVAL_REJECTED gate, approved-consumed resume)
2026-08-06T17:15:55-05:00	ca2821f	feat: loop + subgraph nodes, swarm strategy joins, effect gate, docs; include pending workspace changes (bridge_config, lib, server, tool_exec, provekv_executor)
```
### Tracked source excerpt: `AGENTS.md`

```text
1|# AGENTS.md — agent-graph-mcp
2|
3|Instructions for AI coding agents (Claude Code, Codex, Cursor, Copilot, etc.) working on this repository.
4|
5|## Project identity
6|
7|`agent-graph-mcp` is an MCP server that exposes the `ri-agent-graph` runtime engine as 25 typed tools. It compiles declarative JSON workflow specs, executes LLM graphs with parallel fan-out, checkpoint/resume, human-in-the-loop approvals, source witnessing, and HMAC-authenticated receipts.
8|
9|**Stack:** Rust (edition 2021, MSRV 1.75), Tokio async, rmcp, SQLite.
10|
11|## Build, test, lint
12|
13|```bash
14|cargo build                    # debug build
15|cargo build --release          # release binary
16|cargo test --lib               # 58 lib tests (1 known fixture-path failure)
17|cargo test --test daemon_recovery --test mcp_integration  # integration tests
18|cargo fmt --check              # formatting
19|cargo clippy --all-targets -- -D warnings  # lint (must pass clean)
20|cargo deny check               # dependency auditing
21|```
22|
23|The binary installs to `~/.cargo/bin/agent-graph-mcp`. There is also a daemon binary (`agent-graph-mcpd`) for persistent multi-client mode.
24|
25|## Project structure
26|
27|```
28|src/
29|├── main.rs              # CLI entry point (direct mode, daemon client)
30|├── cli.rs               # Argument parsing
31|├── server.rs            # MCP tool router (25 tools)
32|├── tools.rs             # Tool parameter types + JSON schemas
33|├── daemon.rs             # Daemon process (agent-graph-mcpd)
34|├── run_manager.rs       # Graph execution lifecycle
35|├── store.rs             # SQLite persistence
36|├── migrations.rs        # Schema migrations
37|├── compiler.rs          # JSON spec → executable graph compilation
38|├── spec.rs              # Graph spec types
39|├── nodes.rs             # Node type definitions (LLM, join, passthrough, etc.)
40|├── lifecycle.rs         # Create, validate, delete graph operations
41|├── templates.rs         # Built-in templates (council_deliberation, etc.)
42|├── evidence.rs          # Source witnessing, HMAC receipts
43|├── policy.rs            # Graph execution policy checks
44|├── promotion.rs         # Template promotion to built-in status
45|├── transport.rs         # Daemon transport layer (Unix socket)
46|├── proxy.rs             # MCP proxy between client and daemon
47|├── owner_lock.rs        # Single-owner daemon lock
48|├── operator.rs          # Operator IPC
49|├── operator_auth.rs     # Operator authentication
50|├── operator_ipc.rs      # Operator IPC protocol
51|├── auth.rs              # Client authentication
52|├── fs_security.rs       # Filesystem security controls
53|├── codex_app_server.rs  # Codex app server integration
54|└── lib.rs               # Module declarations + re-exports
55|tests/
56|├── daemon_recovery.rs   # Daemon crash recovery tests
57|├── mcp_integration.rs   # MCP protocol integration tests
58|├── lifecycle.rs         # Graph lifecycle tests
59|├── operator_authority.rs # Operator permission tests
60|├── migrations.rs        # Schema migration tests
61|├── template_promotion.rs # Template promotion tests
62|└── ...                  # Additional integration tests
63|```
64|
65|## Coding conventions
66|
67|- **No `unwrap()` or `expect()` in library code.** Use `anyhow::Result` or `thiserror` for error handling.
68|- **All public items need `///` doc comments.**
69|- **Tool handlers return `Result<Json<Output>, ErrorData>`** — the `Output` struct carries the JSON schema required by MCP spec.
70|- **Tests go in `#[cfg(test)] mod tests` at file bottom** (unit) or in `tests/` (integration).
71|- **Do not add new dependencies without a clear reason.** Prefer extending the existing stack.
72|- **Schema migrations** go in `src/migrations.rs` with versioned migration functions.
73|- **Graph spec validation** happens at `graph_create` time — invalid specs are rejected before execution.
74|
75|## What NOT to do
76|
77|- **Do not fabricate tool capabilities.** Tools must match actual rmcp `#[tool]` handlers.
78|- **Do not add speculative features.** New tools, node types, or templates need a concrete consumer.
79|- **Do not break the daemon protocol.** The Unix socket framed transport between proxy and daemon is a stability boundary.
80|- **Do not modify SQLite schema without a versioned migration.** Schema changes must be backward-compatible or gated behind a migration.
81|- **Do not expose internal errors to MCP clients.** Tool errors should be descriptive but must not leak stack traces or internal state.
82|- **Do not merge PRs with failing tests.** The known `evidence::tests::[LONG_TOKEN_REDACTED]` failure is tracked but should not be joined by new failures.
83|
84|## Security boundaries
85|
86|- **HMAC receipts** in `src/evidence.rs` use SHA-256 HMAC for content authentication. Do not weaken or bypass.
87|- **Daemon authentication** via Unix socket peer credentials (`src/auth.rs`). Do not add unauthenticated TCP listeners.
88|- **Operator IPC** requires explicit authorization (`src/operator_auth.rs`). Never skip operator permission checks.
89|- **Source witnessing** captures caller-supplied content with HMAC verification. Never weaken the authentication tag check.
90|- **Dependencies** are audited via `cargo deny`. New dependencies must pass advisory, ban, license, and source checks.
91|
92|## Publication
93|
94|- **crates.io:** `cargo publish -p agent-graph-mcp`
95|- **npm:** `npm publish` (package includes prebuilt binaries)
96|- Version bumps follow the existing `Cargo.toml` version. Update both crates.io and npm on release.
97|
98|## License
99|
100|MIT. All contributions are under the same license.
101|
```
### Tracked source excerpt: `README.md`

```text
1|# agent-graph-mcp
2|
3|**Run 9 agents at once.** MCP server for graph-orchestrated LLM workflows — dispatch up to 16 LLM nodes in parallel fan-out with typed joins, checkpoint/resume, human-in-the-loop approvals, and HMAC-authenticated execution receipts. 27 typed tools.
4|
5|[![Crates.io](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
6|[![docs.rs](https://img.shields.io/docsrs/agent-graph-mcp)](https://docs.rs/agent-graph-mcp)
7|[![MCP Badge](https://lobehub.com/badge/mcp-full/recursiveintell-agent-graph-mcp?theme=light)](https://lobehub.com/mcp/recursiveintell-agent-graph-mcp)
8|[![npm](https://img.shields.io/npm/v/@recursiveintell/agent-graph-mcp)](https://www.npmjs.com/package/@recursiveintell/agent-graph-mcp)
9|[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
10|
11|![Architecture diagram showing MCP client connecting via stdin/stdout to the agent-graph-mcp proxy, which communicates over Unix socket to the agent-graph-mcpd daemon backed by SQLite](assets/architecture.svg)
12|
13|> **Expose the `ri-agent-graph` runtime engine over MCP.** Compile declarative JSON workflow specs, execute synchronously or asynchronously, checkpoint/resume, request human approval, capture source witnesses, and get cryptographic receipts — all through 27 typed MCP tools. Normal execution is synchronous. Durable approval is supported only as a SQLite-backed decision.
14|
15|## Who is this for?
16|
17|**AI agent operators** who need multi-node LLM orchestration (parallel research sweeps, council deliberation, plan→critique→refine pipelines) through their existing MCP client (Hermes Agent, Claude Desktop, Cursor). **Not for** simple single-call LLM usage — use a direct provider integration for that.
18|
19|## Quick start
20|
21|### Prerequisites
22|
23|- An LLM endpoint (local Ollama, or any OpenAI-compatible API)
24|- Node.js ≥ 18 (for npx) or Rust ≥ 1.75 (for cargo install)
25|- A model available at your endpoint. Examples below use `llama3.2:3b` (pull with `ollama pull llama3.2:3b`). Any model works — just replace `--model`.
26|
27|### npx (recommended)
28|
29|```bash
30|npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
31|```
32|
33|> **Note:** `--direct` is deprecated. Prefer daemon mode (below) for persistence and multi-client support. Direct mode still works but will be removed in a future release.
34|
35|**Expected output:** MCP initialization handshake. Run `tools/list` to verify you see 27 tools.
36|
37|### Cargo install
38|
39|```bash
40|cargo install agent-graph-mcp --locked
41|agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
42|```
43|
44|### Daemon mode (multi-client, persistent state)
45|
46|```bash
47|# Start the daemon (all defaults shown explicitly)
48|agent-graph-mcpd \
49|  --data-dir ~/.local/share/agent-graph \
50|  --socket /tmp/agent-graph/mcp.sock \
51|  --base-url http://127.0.0.1:11434 \
52|  --model llama3.2:3b \
53|  --max-graphs 256 &
54|
55|# Connect proxy
56|agent-graph-mcp --socket /tmp/agent-graph/mcp.sock
57|```
58|
59|**Defaults** (applied when flags are omitted):
60|
61|| Flag | Default | Notes |
62||------|---------|-------|
63|| `--data-dir` | `/tmp/agent-graph` | Ephemeral across reboots. Set explicitly for durable storage |
64|| `--socket` | `/tmp/agent-graph/mcp.sock` | Must match between daemon and proxy |
65|| `--base-url` | `http://127.0.0.1:11434` | Ollama default. Change for any provider |
66|| `--model` | `glm-5.2:cloud` | **Always override this.** The default exists only for backward compatibility |
67|| `--max-graphs` | 64 | Range 1–1024. Raise for large graph libraries |
68|
69|`--max-graphs` is a per-daemon registration capacity. `graph_status` reports both the effective limit and `capacity_state`; an `over_limit_legacy` state preserves existing durable graphs but rejects new registrations until the configured limit is raised or registrations are retired.
70|
71|### Direct vs daemon
72|
73|| Mode | Use when |
74||------|----------|
75|| `--direct` | Single MCP client, no persistence needed, simplest setup |
76|| `--socket` (daemon) | Multiple clients, durable graph storage, long-running workflows, HITL approvals |
77|
78|## Provider and model configuration
79|
80|Every graph run sends LLM calls to an OpenAI-compatible endpoint. You control **where** (`--base-url`) and **which model** (`--model`). The API key flows through the `OPENAI_API_KEY` environment variable.
81|
82|### Codex App Server mode
83|
84|When `--base-url codex-app-server://` is selected, the Rust daemon starts one long-lived local Codex App Server worker over a loopback WebSocket and reuses it across graph nodes. Turns are serialized through a bounded Rust-owned session; a failed or timed-out worker is terminated and recreated cleanly. This avoids the fixed memory multiplier from spawning one heavy App Server process per node.
85|
86|The integration is bounded before launch and during streaming:
87|
88|- one persistent Codex App Server worker when the configured process limit is
89|  one; higher limits use bounded one-shot workers for true provider concurrency;
90|- the Luna service launcher enumerates enabled Codex MCP servers and disables
91|  those servers plus plugin/app injection for prompt-only graph turns;
92|- each completed graph thread is deleted before the worker is reused, preventing
93|  the long-lived connection from retaining auto-subscribed thread state;
94|- prompt input capped at 256 KiB;
95|- model reasoning effort pinned to `low` for bounded graph lanes;
96|- each JSON-RPC line capped at 4 MiB and each WebSocket message capped at 1 MiB;
97|- streamed assistant output capped at 256 KiB;
98|- stderr retained as an 8 KiB tail and redacted before MCP errors;
99|- stdio-only Codex-compatible test executables may use the legacy one-shot compatibility path.
100|
101|`graph max_parallelism` controls graph scheduling, w
```
### Tracked source excerpt: `.hermes/plans/2026-08-01-provenance-tool-runtime.md`

```text
1|# Provenance-Bound Hermes Tool Nodes Implementation Plan
2|
3|> **For Hermes:** Implement task-by-task under RED/GREEN discipline. Do not commit or deploy without independent review.
4|
5|**Goal:** Make Agent Graph `tool` nodes execute a real Hermes worker with the same dynamic tool catalog as a normal Hermes session, while provenance leases, durable receipts, cycle detection, budgets, and approval policy bound recursion and side effects.
6|
7|**Architecture:** Agent Graph remains the deterministic orchestrator. A `tool` node launches `hermes chat -q -Q`—never `hermes --oneshot`—with a daemon-generated lineage lease and an isolated receipt directory. A Hermes plugin intercepts `pre_tool_call` and `post_tool_call`: it verifies the lease, atomically reserves lineage budget, classifies recursive/effectful calls, fails closed when policy or receipt persistence is unavailable, and writes an append-only hash chain. The Rust node accepts output only when a terminal worker receipt verifies against the lease and receipt chain.
8|
9|**Tech Stack:** Rust 2021, Tokio process execution, serde/serde_json, SHA-256/HMAC, SQLite terminal projection, Python 3 Hermes plugin hooks, pytest, cargo test.
10|
11|---
12|
13|## Current evidence
14|
15|- Isolated worktree: `/home/sikmindz/.cache/agent-graph-tool-runtime-20260801`
16|- Branch: `feat/provenance-tool-nodes-20260801`
17|- Baseline HEAD: `4ead448308d7f09dfe9116ac8becf05d0af2334f`
18|- Candidate diff was replicated from `/home/sikmindz/.cache/agent-graph-capacity-20260731`; source and isolated tracked-diff SHA-256 both equal `[LONG_TOKEN_REDACTED]` at isolation time.
19|- `src/spec.rs` already declares `NodeType::Tool` but `GraphSpec::executable_node_type` rejects it.
20|- `src/compiler.rs` rejects `NodeType::Tool` at compilation.
21|- Hermes canonical tool catalog is `model_tools.get_tool_definitions`; canonical dispatch is the full `AIAgent` loop. `todo`, `memory`, `session_search`, and `delegate_task` are agent-loop tools and cannot be reached by plain `registry.dispatch`.
22|- Hermes `--oneshot` says approvals are auto-bypassed; it is forbidden for this runtime. `hermes chat -q -Q` is the worker surface.
23|- Hermes plugin hooks `pre_tool_call` and `post_tool_call` cover registry-dispatched and agent-loop-owned tools.
24|
25|## Hard invariants
26|
27|1. **Full catalog, bounded authority:** Worker agents can see their normal configured tool catalog. Visibility never implies unconditional execution authority.
28|2. **Hermes owns tool semantics:** Agent Graph never reimplements or directly dispatches Hermes tools.
29|3. **No `--oneshot`:** The worker command must reject `--oneshot` and `-z` configurations.
30|4. **Fail closed:** Missing, malformed, expired, unverifiable, or exhausted leases block all tool calls.
31|5. **Receipt before effect:** A pre-call reservation is durably appended before the tool executes. If reservation persistence fails, execution is blocked.
32|6. **Terminal closure:** Rust accepts worker success only with a valid terminal receipt matching graph/run/node/attempt/lease/output digests.
33|7. **Lineage budget:** All descendants share a lineage ID and atomically consume common budgets.
34|8. **Recursive calls are explicit:** `delegate_task`, `cronjob`, `execute_code`, Agent Graph execution/start/resume tools, and tool-search bridge invocation of those tools consume recursive budget and are cycle-checked.
35|9. **No silent widening:** Child processes inherit the same or narrower lease. A model cannot raise tool, effect, depth, call, wall-clock, or child-count limits.
36|10. **Human approval remains distinct:** Effectful/external/authority-changing tools require a valid approval capability or are blocked. A model-produced string is never approval.
37|11. **Replay-safe calls:** Call IDs bind graph version, run, node, attempt, tool, argument digest, parent receipt digest, and lease digest. Replays return the existing terminal receipt or a typed incomplete/indeterminate state; they do not execute twice.
38|12. **Secrets excluded:[REDACTED]
39|13. **Tool output is untrusted:** Worker output cannot mutate policy/lease fields in graph state.
40|14. **Live rollout is separate:** Passing isolated tests does not authorize replacing the production daemon or enabling the plugin.
41|
42|## Recursion policy
43|
44|- `max_graph_depth`: maximum nested Agent Graph lineage depth.
45|- `max_agent_depth`: maximum Hermes worker/delegate lineage depth.
46|- `max_tool_calls`: total calls across the lineage.
47|- `max_recursive_calls`: total calls to orchestration-capable tools across the lineage.
48|- `max_children`: total worker/delegate children across the lineage.
49|- `max_wall_clock_ms`: lineage deadline measured from signed lease issuance.
50|- `active_stack`: ordered digests of graph/worker/tool identities. Re-entry of the same identity without an explicit loop allowance returns `RECURSION_CYCLE_DETECTED`.
51|- Default for unattended graph workers: recursive tools visible but blocked (`max_recursive_calls = 0`). A separately issued operator lease may raise this within hard daemon ceilings.
52|- `cronjob create/update/resume/run`, Agent Graph run/start/resume, `delegate_task`, and `execute_code` are recursive/effectful. Read-only cron/list or graph/list/get operations may be separately classified but still consume tool-call budget.
53|
54|## Phase 1 — Policy and receipt primitives
55|
56|### Task 1.1: RED tests for lease parsing and fail-closed policy
57|
58|**Files:**
59|- Create: `tests/tool_runtime.rs`
60|- Create: `src/tool_runtime.rs`
61|- Modify: `src/lib.rs`
62|
63|**RED cases:** expired lease; missing HMAC; wrong graph/run/node binding; tool not granted; effect class not granted; exhausted tool count; exhausted recursive count; cycle in active stack; attempted scope widening.
64|
65|**GREEN:** typed `ToolLease`, `ToolInvocation`, `ToolEffect`, `ToolDecision`, and deterministic digest/HMAC verification functions. No process execution yet.

```

## Workstream 2: /home/sikmindz/Coding/Libraries

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
? cea-bridge
 M context-governor/src/lib.rs
 M context-governor/src/main.rs
 M context-governor/src/receipt_index.rs
 M context-governor/tests/hmac_key_lifecycle.rs
```
- Diff stat:
```text
context-governor/src/lib.rs                  |  5 +-
 context-governor/src/main.rs                 | 23 ++++----
 context-governor/src/receipt_index.rs        | 83 ++++++++++++++++++++++++----
 context-governor/tests/hmac_key_lifecycle.rs | 40 +++++++++++++-
 4 files changed, 127 insertions(+), 24 deletions(-)
```
- Five latest commits:
```text
2026-08-07T02:43:49-05:00	0b3a0317	feat: receipt HMAC key management — full lifecycle
2026-08-07T01:16:56-05:00	e2ce6386	feat: add eval fixture and finalize plan doc
2026-08-06T22:47:45-05:00	82b236b2	feat: live-model answerability eval with DeepSeek API
2026-08-06T22:42:47-05:00	32c02995	docs: mark E1 full extraction complete
2026-08-06T22:42:32-05:00	7bb54222	refactor: complete classify module extraction (E1 full)
```
### Tracked source excerpt: `AGENTS.md`

```text
1|# AGENTS.md — V29 Agent Coordination
2|
3|## Execution Model
4|
5|This pack is designed for implementation by a single agent (Claude Code or equivalent) working sequentially through phases. No multi-agent coordination is required.
6|
7|## Agent capabilities required
8|
9|- Rust source file editing (str_replace / create_file)
10|- Bash command execution (cargo check, cargo test, script execution)
11|- File system operations (mkdir, mv, cp)
12|- JSON file editing
13|- Markdown file creation and editing
14|
15|## Session strategy
16|
17|### Session 1: Phase 1 (fast — 30min)
18|Fix GATE-001, TRUTH-001, DOC-002. Three commits. Run cargo check after each.
19|
20|### Session 2: Phase 2 core (1–2hr)
21|Fix TRUTH-002, TRUTH-003, GATE-002. Archive cleanup and script fixes.
22|
23|### Session 3: Phase 2 wire format (1hr)
24|Fix WIRE-001. 56 serde annotations, crate by crate. Cargo check after each crate. Cargo test after all.
25|
26|### Session 4: Phase 2 docs (2–3hr)
27|Fix DOC-001. Doc comment pass on supported-lane crates. This is the longest single task. Can be time-boxed to the 5 highest-priority crates if deadline pressure is severe.
28|
29|### Session 5: Phase 3 (1–2hr)
30|Fix all Phase 3 issues. All independent.
31|
32|### Session 6: Final gate (30min)
33|Run make gate, cargo check/test/clippy/doc. Fix any remaining failures. Generate clean archive.
34|
35|## Total estimated time: 6–10 hours with AI assist
36|
37|## Context window management
38|
39|Each session should begin by reading:
40|1. `CLAUDE.md` (always)
41|2. `02_MASTER_ISSUE_MATRIX.md` (for current phase context)
42|3. `04_EXACT_FILE_TOUCH_MAP.md` (for the specific files in the current phase)
43|
44|Do NOT load the full tensor JSON or both audit reports into context — they are reference material, not execution instructions.
45|
```
### Tracked source excerpt: `AiDENs/AGENTS.md`

```text
1|# AGENTS.md — AiDENs P30 hardening doctrine
2|
3|## Mission
4|
5|You are implementing the P30 Codex super-pass for AiDENs. Your job is to harden AiDENs against the 2026-05-08 hostile audit and move the codebase closer to v11A/v11B compliance without creating shadow semantics.
6|
7|## Core law
8|
9|1. Provenance-first design is a hard constraint.
10|2. Correctness outranks speed, momentum, aesthetics, and completion theater.
11|3. No silent approximation, no semantic widening, no invented compatibility semantics.
12|4. AiDENs directs, wires, scopes, exposes, validates, and coordinates. AiDENs must not become the owner of domain truth owned by sibling crates.
13|5. Every material operation must be represented as a typed, receipt-bearing artifact transition where possible.
14|6. Runtime/tool/control layers must not become hidden truth stores.
15|7. Execution is evidence: tool calls, retries, queue hops, provider routes, deadlines, cancellations, fallback paths, degraded paths, replay attempts, and budget exhaustion must be receipt-bearing or explicitly non-durable/degraded.
16|8. Valid time and recorded/transaction time must remain distinct. Never collapse them for convenience.
17|9. Append-plus-supersession only. No silent destructive rewrite of truth-bearing state.
18|10. Repairs, boundary repairs, parse repairs, rollback, schema repair, and compatibility handling must emit explicit repair/degradation provenance.
19|11. Material IDs must be deterministic and replay-safe. Process-local counters, random UUIDs, and branch-order IDs are forbidden for material receipts/artifacts/manifests/operator invocations.
20|12. No user-visible “done,” “verified,” “succeeded,” “ready,” or “v11A compliant” state unless the required receipts/checks exist.
21|
22|## Source-of-truth map
23|
24|| Surface | Canonical owner | AiDENs role | Forbidden AiDENs behavior |
25||---|---|---|---|
26|| Stable IDs, digests, trace primitives | `stack-ids` and contract owner crates | consume/wire only | invent new material identity law |
27|| Semantic memory/projection truth | `semantic-memory`, `semantic-memory-forge`, `forge-memory-bridge` | coordinate import/query paths | create duplicate memory truth layer |
28|| Evidence/export truth | `semantic-memory-forge`, `living-memory`, bridge crates | consume/wire/package | reinterpret evidence meaning |
29|| Tool contracts/receipts | `llm-tool-runtime`, AiDENs receipt/tool kits as adapters | expose safely and record receipts | drop tool evidence or repair silently |
30|| Verification policy/control | `verification-*`, `assurance-runtime` | route/check/report | represent advisory observation as verified success |
31|| Kernel/oracle/conformance | `recursive-kernel-*`, `constraint-compiler`, `kernel-*` | orchestrate and expose | invent local oracle semantics |
32|| Artifact contracts | `aidens-contracts` only where AiDENs-owned; sibling crates where stack-owned | define orchestration contracts | duplicate canonical stack contracts without owner map |
33|| Package/source certification | `z.py`, `zip.py`, certifier sidecars | run/report/consume | claim semantic/build correctness from packaging-only evidence |
34|
35|## Hard fail patterns
36|
37|Codex must stop or quarantine if it encounters any of these:
38|
39|- `unwrap_or_default()` used to erase read/serialization/parse failures in material paths.
40|- `filter_map` used to drop malformed executable tool-call entries without rejected-call receipts.
41|- permissive JSON repair feeding executable tool calls without strict degradation/approval gates.
42|- rollback errors ignored with `let _ = ...`.
43|- wildcard permits, host ambient PATH, or unfrozen toolchain execution for command tools.
44|- material IDs generated from process-local counters, random UUIDs, wall-clock-only data, branch order, or constant strings.
45|- advisory checks reported as `Succeeded` verification attempts.
46|- failure paths returning empty evidence where durable failure receipts are required.
47|- missing gate scripts/schemas referenced by docs without supersession evidence.
48|- root Markdown ambiguity allowed to steer current run instructions.
49|- `serde_json::Value` or dynamic JSON used where a typed boundary contract is required.
50|- `panic!`, `unwrap`, `expect`, `todo!`, `unimplemented!`, broad `allow(...)`, or lint suppression in runtime/control/tool/evidence paths unless explicitly justified and tested.
51|
52|## Required evidence per phase
53|
54|Every phase must emit:
55|
56|- changed file summary;
57|- issue IDs addressed from `matrices/P30_HOSTILE_AUDIT_ABSORPTION_MATRIX.csv`;
58|- tests added/updated;
59|- commands run with outputs captured;
60|- unresolved risks and quarantines;
61|- invariant revalidation checklist;
62|- statement of whether the phase can proceed.
63|
64|## Final claim discipline
65|
66|Do not claim v11A/v11B compliance unless the release/conformance gates prove it. Acceptable claims are narrower, e.g. “P30 repaired the parser fallback P0s and added tests,” or “P30 introduced v11A seed contracts but full v11A release remains pending.”
67|
```
### Tracked source excerpt: `_salvage_from_libraries2/Libraries2/agent-graph/AGENTS.md`

```text
1|# AGENTS.md — Working Agreement for Refactoring This LangGraph Clone
2|
3|## Prime Directive
4|This crate is the **orchestrator/runtime**. It must not become a second “payload library.”
5|Payload logic (LLM calls, parsing, streaming decoding) lives in the LangChain payload crate.
6|
7|## Must-Haves
8|1. **Node boundary is `serde_json::Value`**
9|   - Heterogenous workflows are first-class.
10|   - Typed helpers can exist at edges, but runtime wiring is Value.
11|
12|2. **Core has no Tauri dependency**
13|   - Any tauri-queue integration is feature-gated and/or in a separate crate.
14|
15|3. **Checkpointing and interrupts are first-class**
16|   - Persist node attempts and outcomes.
17|   - Support interrupt/resume with injected input.
18|
19|4. **Explicit concurrency semantics**
20|   - Fan-out/fan-in must be deterministic.
21|   - No implicit concurrent state merges without a join policy.
22|
23|5. **Repo stays green**
24|   - cargo fmt
25|   - cargo test
26|   - cargo clippy -- -D warnings
27|
28|## Allowed Scope
29|✅ Node types, scheduler/executor, checkpoint store, events, interrupt/resume  
30|✅ Feature-gated adapter to tauri-queue  
31|✅ Documentation and examples  
32|❌ No provider clients, no prompt templating, no JSON parsing logic here (belongs in payload crate)  
33|❌ No Tauri dependency in core crate  
34|
35|## Definition of Done Checklist
36|- [ ] PayloadNode exists and runs `Box<dyn Payload>`
37|- [ ] Router and Join semantics implemented and tested
38|- [ ] Looping supported with termination controls
39|- [ ] CheckpointStore trait + in-memory implementation
40|- [ ] Interrupt/resume works end-to-end
41|- [ ] EventSink supports token and lifecycle events
42|- [ ] Optional tauri-queue executor adapter (feature-gated) if implemented
43|- [ ] README + ARCHITECTURE.md updated
44|- [ ] fmt/test/clippy clean
45|
```

## Workstream 3: /home/sikmindz/Coding/Libraries/semantic-memory

- Branch: `main`
- Upstream: `(none)`
- Working tree status:
```text
M Cargo.toml
 M src/config.rs
 M src/lib.rs
 M src/poly_kv_backend.rs
 M src/search.rs
?? examples/poly_kv_whole_path_receipt.rs
?? src/whole_path_receipt.rs
?? tests/whole_path_receipt_schema.rs
```
- Diff stat:
```text
Cargo.toml             |    7 +-
 src/config.rs          |  126 ++++-
 src/lib.rs             |   21 +
 src/poly_kv_backend.rs |  348 +++++++++++--
 src/search.rs          | 1316 +++++++++++++++++++++++++++++++++++++++++++++++-
 5 files changed, 1735 insertions(+), 83 deletions(-)
```
- Five latest commits:
```text
2026-08-02T21:15:15-05:00	bcfe3af	style(semantic-memory): format PolyKV backend
2026-08-01T08:22:40-05:00	94ea314	Merge branch 'feat/full-integration'
2026-08-01T08:22:29-05:00	ceeb3c7	feat: add PolyKV compressed embedding backend
2026-07-30T19:27:14-05:00	aaef5bf	feat: add v39 verified fact replication receiver
2026-07-28T22:19:34-05:00	bd53ac3	feat: enforce verified replication journal v38
```
### Tracked source excerpt: `README.md`

```text
1|# semantic-memory
2|
3|Local-first hybrid retrieval for Rust, with SQLite as authoritative state and receipts for execution evidence.
4|
5|![semantic-memory system and trust boundaries](docs/assets/semantic-memory-boundaries.svg)
6|
7|`semantic-memory` stores facts, documents and chunks, conversations, episodes, embeddings, temporal state, authority ledgers, and search receipts in SQLite. FTS indexes, vector sidecars, sparse representations, and compressed candidate artifacts accelerate retrieval; they do not replace canonical state and can be reconciled from SQLite.
8|
9|> **Status:** research-grade library with a tested default retrieval contract. Feature-gated research and orchestration modules are not implicit guarantees of `MemoryStore::search()` behavior.
10|
11|| Contract fact | Current value |
12|| --- | --- |
13|| Crate version | `0.5.14` |
14|| Minimum Rust version | `1.75` |
15|| Default Cargo feature | `usearch-backend` |
16|| Maximum schema version | `38` |
17|| License | Apache-2.0 |
18|
19|## What the crate owns
20|
21|| Surface | Contract |
22|| --- | --- |
23|| Canonical storage | SQLite content, raw f32 embeddings, temporal fields, lineage, authority records, and durable receipts |
24|| Default retrieval | FTS5/BM25 plus dense-vector candidates, weighted reciprocal-rank fusion, current-state visibility, deduplication, and diversity |
25|| State semantics | Current, historical, and supersession views for facts, plus separately invoked transition and state-resolution APIs |
26|| Recovery | Integrity checks and reconciliation from canonical SQLite state |
27|| Governed mutation | Capability-gated append, supersession, redaction, forgetting, export, and replay through `MemoryStore::authority()` |
28|| Execution evidence | Optional search receipts that disclose backend, exactness, candidates, fallback, degradation, and result identity |
29|
30|## What the crate does not own
31|
32|- **Claim truth.** A search receipt records retrieval execution; it is not a claim-ledger verification decision.
33|- **Agent action permission.** Recall authority does not grant permission to assert a memory or act on it.
34|- **MCP transport policy.** See [`semantic-memory-mcp`](https://github.com/RecursiveIntell/semantic-memory-mcp) for transport, tool profiles, and application-level authority composition.
35|- **Automatic activation of every feature.** Compiling routing, graph, topology, community, decoder, or compression modules does not make the normal search path invoke them.
36|- **Native SPLADE or native ColBERT.** Dense-derived sparse values and the current late-interaction proxy must not be presented as those systems.
37|
38|## Install
39|
40|```toml
41|[dependencies]
42|semantic-memory = "0.5.14"
43|tokio = { version = "1", features = ["macros", "rt"] }
44|```
45|
46|The default build enables `usearch-backend`. For an exact pure-Rust backend without the C++ bridge:
47|
48|```toml
49|[dependencies]
50|semantic-memory = { version = "0.5.14", default-features = false, features = ["brute-force"] }
51|```
52|
53|## Quick start
54|
55|This example uses the deterministic `MockEmbedder`, so it needs no network service or downloaded model.
56|
57|```rust
58|use semantic_memory::{EmbeddingConfig, MemoryConfig, MemoryStore, MockEmbedder};
59|use std::path::PathBuf;
60|
61|#[tokio::main(flavor = "current_thread")]
62|async fn main() -> Result<(), semantic_memory::MemoryError> {
63|    let config = MemoryConfig {
64|        base_dir: PathBuf::from("memory-example"),
65|        embedding: EmbeddingConfig {
66|            dimensions: 768,
67|            ..Default::default()
68|        },
69|        ..Default::default()
70|    };
71|
72|    let store = MemoryStore::open_with_embedder(
73|        config,
74|        Box::new(MockEmbedder::new(768)),
75|    )?;
76|
77|    store
78|        .add_fact("general", "Rust was first released in 2015", None, None)
79|        .await?;
80|
81|    let results = store
82|        .search(
83|            "when was Rust released",
84|            Some(5),
85|            Some(&["general"]),
86|            None,
87|        )
88|        .await?;
89|
90|    for result in results {
91|        println!("{:.4} {}", result.score, result.content);
92|    }
93|
94|    Ok(())
95|}
96|```
97|
98|`MemoryStore::open()` selects `OllamaEmbedder` unless the crate is compiled with `candle-embedder`, in which case it selects `CandleEmbedder`. Use `open_with_embedder` to inject another provider or a deterministic fixture.
99|
100|## Choose the retrieval API deliberately
101|
102|| Need | API | Notes |
103|| --- | --- | --- |
104|| Default hybrid retrieval | `search` | Facts, document chunks, and searchable episodes; current state; receipt disabled by default |
105|| Explicit context and receipt | `search_with_context` | Caller-supplied or wall-clock-initialized evaluation time, receipt/replay mode, exactness profile, request identity, and budgets |
106|| Historical or superseded view | `search_with_view` | Caller chooses `StateView`; never relabels a current search as historical |
107|| Lexical only | `search_fts_only` / `search_fts_only_with_context` | No query embedding required |
108|| Vector only | `search_vector_only` / `search_vector_only_with_context` | Supports `ExactnessProfile::PreferExact` through context |
109|| Per-lane score evidence | `search_explained` / `search_explained_with_context` | Returns BM25, vector, sparse, and recency fields with RRF contributions; the late-interaction proxy is not separately attributed |
110|| Conversation retrieval | `search_conversations` | Separate session/message search surface |
111|| Governed read/search | `MemoryStore::authority()` | Enforces origin and capability contracts independently of relevance score |
112|
113|Messages are not part of the normal hybrid source set unless selected with `SearchSourceType::Messages`. Conversation search is available separately.
114|
115|## Default hybrid retrieval contract
116|
117|![semantic-memory de
```
### Tracked source excerpt: `docs/evaluation/scifact/README.md`

```text
1|# BEIR SciFact retrieval evaluation
2|
3|Receipt-bearing evaluation of `semantic-memory` retrieval APIs on the official BEIR SciFact test corpus.
4|
5|![BEIR SciFact evaluation workflow](../../assets/scifact-evaluation.svg)
6|
7|The harness builds one persisted store, executes a deterministic calibration/held-out split, emits raw per-query rows plus aggregate receipts, and validates those artifacts independently. The checked-in repository contains the harness and validator; it does **not** contain a frozen score artifact.
8|
9|## Claim boundary
10|
11|A validated run can support claims about retrieval quality and local latency for the exact:
12|
13|- executable and source revision;
14|- SciFact corpus and payload hashes;
15|- embedding model and dimensions;
16|- persisted store and document mapping;
17|- retrieval mode and configuration;
18|- split definition and selected query IDs;
19|- raw per-query rows and aggregate receipt.
20|
21|It does not establish:
22|
23|- superiority over another system;
24|- general-domain retrieval quality;
25|- embedding-model quality in isolation;
26|- graph or factor-graph retrieval quality;
27|- native sparse/SPLADE retrieval;
28|- token-level late interaction;
29|- Matryoshka retrieval quality;
30|- production latency on another host.
31|
32|Do not publish a bare metric table. Publish the validated artifact bundle and state what it does not prove.
33|
34|## Frozen evaluation contract
35|
36|| Property | Value enforced by source |
37|| --- | --- |
38|| Dataset | BEIR-hosted SciFact archive; source hashes recorded, but no expected archive digest is pinned in the builder |
39|| Corpus shape | 5,183 documents |
40|| Test queries | 300 |
41|| Default embedding fixture | `all-minilm:latest` through local Ollama; configurable by builder argument or environment |
42|| Maximum semantic text length | 700 Unicode scalar values before the query/document role prefix is added; complete prompts may be longer |
43|| Store namespace | `beir-scifact` |
44|| Retrieval depth | `top_k = 10` |
45|| Calibration split | First 100 query IDs after sorting by `(SHA-256(UTF-8 query_id), query_id)` |
46|| Held-out split | Remaining 200 query IDs |
47|| Modes | `fts_only`, `vector_only`, `hybrid` |
48|| Row schema | `semantic-memory-scifact-query-v1` |
49|| Aggregate schema | `semantic-memory-scifact-aggregate-v1` |
50|
51|The builder rejects any corpus that does not match the expected 5,183-document/300-query shape. The runner refuses unknown query text rather than silently re-embedding through another model.
52|
53|## Retrieval modes
54|
55|| Mode | API | Exactness and lane policy |
56|| --- | --- | --- |
57|| `fts_only` | `search_fts_only_with_context` | Lexical retrieval only |
58|| `vector_only` | `search_vector_only_with_context` | `ExactnessProfile::PreferExact`; receipt must disclose backend evidence |
59|| `hybrid` | `search_explained_with_context` | Baseline BM25+dense RRF with component score evidence |
60|
61|The frozen baseline sets:
62|
63|- `sparse_weight = 0`;
64|- dense-derived sparse retrieval off;
65|- late-interaction weight to zero;
66|- `candidate_dims = None`;
67|- recency off;
68|- derived-vector backend disabled.
69|
70|SciFact ingestion creates no graph edges. The harness therefore cannot support graph, factor-graph, native sparse, token-level late-interaction, or Matryoshka claims.
71|
72|## Artifact flow
73|
74|```text
75|BEIR-hosted SciFact archive
76|  -> shape and path-safety checks
77|  -> normalized document/query embeddings with append-only fsync cache
78|  -> persisted semantic-memory fact store + BEIR-ID mapping
79|  -> deterministic calibration or held-out selection
80|  -> one JSONL + one aggregate JSON per retrieval mode
81|  -> independent metric/hash/split/executable validation
82|```
83|
84|## Prerequisites
85|
86|Run from the Libraries workspace root.
87|
88|- Python 3
89|- Python package `requests`
90|- local Ollama
91|- `all-minilm:latest`
92|- Rust 1.75 or newer
93|- a compatible C++ toolchain for the default usearch build, or the explicit `brute-force` feature command shown below
94|
95|```bash
96|python3 -m pip install requests
97|ollama pull all-minilm:latest
98|```
99|
100|## 1. Build the corpus fixture
101|
102|```bash
103|python3 -u semantic-memory/tools/scifact_eval/build_scifact_semantic_memory.py \
104|  --out semantic-memory/target/scifact-eval/scifact-all-minilm-corpus.json \
105|  --work-dir semantic-memory/target/scifact-eval/build
106|```
107|
108|The builder:
109|
110|1. downloads the archive from the source URL embedded in the output fixture; the builder records source hashes but does not compare against a pinned expected archive digest;
111|2. rejects absolute or parent-traversing ZIP members;
112|3. verifies corpus and query counts;
113|4. constructs the semantic text used for embedding;
114|5. normalizes every vector and rejects empty, zero-norm, non-finite, or dimension-drifting embeddings;
115|6. stores new embeddings in an append-only JSONL cache;
116|7. flushes and `fsync`s every newly cached vector so interrupted builds can resume;
117|8. records source, payload, embedding, and truncation metadata in the fixture.
118|
119|The 700-character ceiling applies to semantic text before `search_document:` or `search_query:` is added. It is not a tokenizer-aware prompt limit. If Ollama rejects a pathological complete prompt, the builder retries deterministic 500/300/120-character prompt lengths and discloses that policy in corpus metadata.
120|
121|`all-minilm:latest` is the documented default, not an immutable harness invariant. If `--model` or `SCIFACT_EMBED_MODEL` changes it, retain that model identity in the fixture and every public claim.
122|
123|## 2. Run calibration
124|
125|Calibration is the only split intended for diagnosis or configuration selection.
126|
127|```bash
128|SCIFACT_EVAL_LAUNCHER='cargo run -p semantic-memory --example scifact_retrieval_eval -- ...' \
129|  cargo run -p semantic-memory --example
```

## Workstream 4: /home/sikmindz/Coding/agent-graph-release

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
(clean)
```
- Diff stat:
```text
(none)
```
- Five latest commits:
```text
2026-08-06T19:57:43-05:00	0f1dda2	feat: deterministic comparator (paired bootstrap, noninferiority, denial gates) + raw MCP client + held-out audit corpus
2026-08-06T19:02:49-05:00	335f96d	docs: correct join mode count to 10 (was 9)
2026-08-06T17:39:57-05:00	2da77fc	feat: operator approval-decision client + held-out audit corpus (6 tasks, profiles, acceptance/denial criteria)
2026-08-06T17:18:28-05:00	1321a15	chore: drop pre-existing tracked pytest bytecode cache
2026-08-06T17:18:11-05:00	b329b87	chore: remove committed Python bytecode cache, ignore __pycache__
```
### Tracked source excerpt: `agent-graph/AGENTS.md`

```text
1|# AGENTS.md — Working Agreement for Refactoring This LangGraph Clone
2|
3|## Prime Directive
4|This crate is the **orchestrator/runtime**. It must not become a second “payload library.”
5|Payload logic (LLM calls, parsing, streaming decoding) lives in the LangChain payload crate.
6|
7|## Must-Haves
8|1. **Node boundary is `serde_json::Value`**
9|   - Heterogenous workflows are first-class.
10|   - Typed helpers can exist at edges, but runtime wiring is Value.
11|
12|2. **Core has no Tauri dependency**
13|   - Any tauri-queue integration is feature-gated and/or in a separate crate.
14|
15|3. **Checkpointing and interrupts are first-class**
16|   - Persist node attempts and outcomes.
17|   - Support interrupt/resume with injected input.
18|
19|4. **Explicit concurrency semantics**
20|   - Fan-out/fan-in must be deterministic.
21|   - No implicit concurrent state merges without a join policy.
22|
23|5. **Repo stays green**
24|   - cargo fmt
25|   - cargo test
26|   - cargo clippy -- -D warnings
27|
28|## Allowed Scope
29|✅ Node types, scheduler/executor, checkpoint store, events, interrupt/resume  
30|✅ Feature-gated adapter to tauri-queue  
31|✅ Documentation and examples  
32|❌ No provider clients, no prompt templating, no JSON parsing logic here (belongs in payload crate)  
33|❌ No Tauri dependency in core crate  
34|
35|## Definition of Done Checklist
36|- [ ] PayloadNode exists and runs `Box<dyn Payload>`
37|- [ ] Router and Join semantics implemented and tested
38|- [ ] Looping supported with termination controls
39|- [ ] CheckpointStore trait + in-memory implementation
40|- [ ] Interrupt/resume works end-to-end
41|- [ ] EventSink supports token and lifecycle events
42|- [ ] Optional tauri-queue executor adapter (feature-gated) if implemented
43|- [ ] README + ARCHITECTURE.md updated
44|- [ ] fmt/test/clippy clean
45|
```
### Tracked source excerpt: `README.md`

```text
1|# ri-agent-graph
2|
3|**Run 9 agents at once.** Graph-based agent orchestration for Rust — a LangGraph-inspired execution engine with parallel fan-out (up to 16 nodes), fan-in joins, checkpointing, interrupt/resume, cryptographic receipts, and an MCP server exposing 25 typed tools.
4|
5|[![Crates.io — engine](https://img.shields.io/crates/v/ri-agent-graph?label=ri-agent-graph)](https://crates.io/crates/ri-agent-graph)
6|[![Crates.io — mcp](https://img.shields.io/crates/v/agent-graph-mcp?label=agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
7|[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
8|
9|---
10|
11|## What's here
12|
13|| Crate | crates.io | Description |
14||-------|-----------|-------------|
15|| **[ri-agent-graph](./agent-graph/)** | [![v0.2.2](https://img.shields.io/crates/v/ri-agent-graph)](https://crates.io/crates/ri-agent-graph) | Core engine — `AgentGraph`, `GraphExecutor`, 8 node types, checkpointing, receipts, 149 tests |
16|| **[agent-graph-mcp](./agent-graph-mcp/)** | [![v0.2.4](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp) | MCP server — 25 typed tools, daemon/proxy, HITL, witnesses, templates |
17|
18|> **The MCP server now has its own dedicated repo at [RecursiveIntell/agent-graph-mcp](https://github.com/RecursiveIntell/agent-graph-mcp)**
19|
20|## Quick start
21|
22|### Core engine
23|
24|```bash
25|cargo add ri-agent-graph
26|```
27|
28|```rust
29|use ri_agent_graph::prelude::*;
30|
31|let graph = AgentGraph::builder()
32|    .add_node("greet", node!(|state| async move {
33|        state.set("msg", "hello").await?; Ok(())
34|    }))
35|    .add_edge(START, "greet")
36|    .add_edge("greet", END)
37|    .build()?;
38|
39|let result = graph.execute(START, AgentState::new()).await?;
40|```
41|
42|### MCP server
43|
44|```bash
45|npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
46|```
47|
48|## Architecture
49|
50|```
51|Hermes ──→ agent-graph-mcp (proxy) ──Unix socket──→ agent-graph-mcpd (daemon) ──→ SQLite
52|              stdin/stdout                framed             Tokio async I/O
53|```
54|
55|## 9 agents at once
56|
57|Fan out to 9 LLM nodes in parallel via `JoinSet`-backed concurrency, then join into one synthesis:
58|
59|```json
60|{
61|  "name": "9-agent-sweep",
62|  "entry": "fanout",
63|  "nodes": [
64|    {"id": "fanout", "type": "passthrough"},
65|    {"id": "agent_0", "type": "llm", "prompt": "Research topic A: {input}"},
66|    {"id": "agent_1", "type": "llm", "prompt": "Research topic B: {input}"},
67|    {"id": "agent_2", "type": "llm", "prompt": "Research topic C: {input}"},
68|    {"id": "agent_3", "type": "llm", "prompt": "Analyze dim 1: {input}"},
69|    {"id": "agent_4", "type": "llm", "prompt": "Analyze dim 2: {input}"},
70|    {"id": "agent_5", "type": "llm", "prompt": "Analyze dim 3: {input}"},
71|    {"id": "agent_6", "type": "llm", "prompt": "Critique angle X: {input}"},
72|    {"id": "agent_7", "type": "llm", "prompt": "Critique angle Y: {input}"},
73|    {"id": "agent_8", "type": "llm", "prompt": "Synthesize: {collected}"},
74|    {"id": "join", "type": "join", "config": {"inputs": ["agent_0","agent_1","agent_2","agent_3","agent_4","agent_5","agent_6","agent_7","agent_8"], "output": "collected", "mode": "collect_array"}},
75|    {"id": "report", "type": "llm", "prompt": "Final report from: {collected}"}
76|  ],
77|  "edges": [
78|    {"from": "fanout", "to": "agent_0"}, {"from": "fanout", "to": "agent_1"},
79|    {"from": "fanout", "to": "agent_2"}, {"from": "fanout", "to": "agent_3"},
80|    {"from": "fanout", "to": "agent_4"}, {"from": "fanout", "to": "agent_5"},
81|    {"from": "fanout", "to": "agent_6"}, {"from": "fanout", "to": "agent_7"},
82|    {"from": "fanout", "to": "agent_8"},
83|    {"from": "agent_0", "to": "join"}, {"from": "agent_1", "to": "join"},
84|    {"from": "agent_2", "to": "join"}, {"from": "agent_3", "to": "join"},
85|    {"from": "agent_4", "to": "join"}, {"from": "agent_5", "to": "join"},
86|    {"from": "agent_6", "to": "join"}, {"from": "agent_7", "to": "join"},
87|    {"from": "agent_8", "to": "join"},
88|    {"from": "join", "to": "report"}, {"from": "report", "to": "END"}
89|  ],
90|  "max_parallelism": 9
91|}
92|```
93|
94|Scale from 1 to 16 parallel branches. Join modes: `collect_array`, `merge_objects`, `first_non_null`, `all_success`, `quorum`.
95|
96|## Node types
97|
98|| Type | Description |
99||------|-------------|
100|| `llm` | LLM call via Ollama with prompt, JSON mode, tool calls |
101|| `router` | Conditional branching via `path`+`op`+`value`→`targets` |
102|| `join` | Fan-in merge — `collect_array`, `merge_objects`, `first_non_null`, `all_success`, `quorum` |
103|| `parallel` | Fan-out dispatch with `JoinSet`-backed concurrency |
104|| `passthrough` | No-op passthrough for fan-out distribution |
105|| `state_transform` | 10 ops: `set`, `copy`, `delete`, `increment`, `append`, `merge`, `merge_object`, `select`, `compare`, `format` |
106|| `subgraph` | Compose another graph as a node |
107|| `human_approval` | HITL gate — emits `InterruptError`, resumes via checkpoint |
108|
109|## MCP tools (25)
110|
111|**Graph lifecycle:** `graph_create`, `graph_list`, `graph_inspect`, `graph_render` · **Execution:** `graph_execute`, `graph_run_start`, `graph_run_wait`, `graph_run_cancel`, `graph_run_get` · **State:** `graph_run_state`, `graph_run_events`, `graph_run_checkpoint`, `graph_run_resume` · **Approval:** `graph_approval_list`, `graph_approval_get`, `graph_approval_request` · **Evidence:** `graph_source_witness_capture`, `graph_source_witness_get` · **Templates:** `graph_template_list`, `graph_template_instantiate`, `graph_template_candidates`, `graph_template_outcomes` · **Policy:** `graph_policy_check`, **Receipt:** `graph_run_receipt`, **Status:** `graph_status`
112|
113|## Built-in templates
114|
115|| Template | Description |
116||----------|-------
```
### Tracked source excerpt: `agent-graph-mcp/README.md`

```text
1|# agent-graph-mcp
2|
3|**MCP server for graph-orchestrated LLM workflows** — 25 typed tools, daemon/proxy architecture, checkpoint/resume, human-in-the-loop approvals, and HMAC-authenticated execution receipts.
4|
5|[![Crates.io](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
6|[![docs.rs](https://img.shields.io/docsrs/agent-graph-mcp)](https://docs.rs/agent-graph-mcp)
7|[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
8|
9|![Architecture](assets/architecture.svg)
10|
11|## What it gives you
12|
13|- **25 typed MCP tools** — graph lifecycle, execution (sync + async), state inspection, checkpoint/resume, HITL approvals, source witnesses, templates, policy validation
14|- **Daemon + proxy architecture** — single-process daemon with file lock ownership, crash recovery, and startup mode enforcement; stateless proxy that bridges stdin/stdout to Unix socket
15|- **Durable persistence** — SQLite-backed with atomic checkpoint transactions, no partial rows after crash
16|- **Deterministic local checkpoint/resume** — HMAC-SHA256 authenticated checkpoints for linear chains of deterministic `passthrough` and `state_transform` nodes
17|- **Built-in templates** — `council_deliberation` (3-analyst parallel), `parallel_council` (debate), `plan_critique_refine`, `analysis_pipeline`, `classifier_router`
18|- **Evidence witnessing** — caller-supplied source capture with HMAC-authenticated receipts; locators never fetched, authority never asserted
19|- **Crash recovery** — interrupted runs report `interrupted` after restart; no fake `running` or `completed` state
20|
21|![Tool Overview](assets/tools-overview.svg)
22|
23|## Architecture
24|
25|```
26|Hermes ──→ agent-graph-mcp (proxy) ──Unix socket──→ agent-graph-mcpd (daemon) ──→ SQLite
27|              stdin/stdout                framed             Tokio async I/O
28|```
29|
30|| Component | Description |
31||-----------|-------------|
32|| **Daemon** (`agent-graph-mcpd`) | Single-process owner with file lock, Tokio async Unix socket listener, SQLite persistence, startup mode enforcement, crash recovery |
33|| **Proxy** (`agent-graph-mcp`) | Stateless stdin/stdout ↔ framed socket bridge; `--direct` flag for legacy in-process mode |
34|| **Socket** | 0600 permissions, 4-byte BE length prefix + JSON-RPC 2.0 framing |
35|
36|## Quick start
37|
38|### 1. Build and install
39|
40|```bash
41|cargo build --release -p agent-graph-mcp
42|cp target/release/agent-graph-mcp ~/.cargo/bin/
43|cp target/release/agent-graph-mcpd ~/.cargo/bin/
44|```
45|
46|### 2. Start the daemon
47|
48|```bash
49|mkdir -p ~/.local/share/agent-graph
50|openssl rand -hex 32 > ~/.local/share/agent-graph/integrity.key
51|agent-graph-mcpd --data-dir ~/.local/share/agent-graph --socket /tmp/agent-graph.sock &
52|```
53|
54|### 3. Configure Hermes
55|
56|```yaml
57|mcp_servers:
58|  agent_graph:
59|    command: ~/.cargo/bin/agent-graph-mcp
60|    args:
61|      - --base-url
62|      - http://127.0.0.1:11434
63|      - --model
64|      - glm-5.2:cloud
65|      - --data-dir
66|      - ~/.agent-graph
67|    enabled: true
68|```
69|
70|### 4. Verify
71|
72|```bash
73|# Smoke test — verify no tracing pollution on stdout
74|printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n' | timeout 5 agent-graph-mcp --base-url http://127.0.0.1:11434 --model glm-5.2:cloud 2>/dev/null | grep -c '"jsonrpc"'
75|# Expected: 2 (initialize + tools/list response)
76|```
77|
78|## Tools reference
79|
80|### Graph lifecycle (4 tools)
81|
82|| Tool | Description |
83||------|-------------|
84|| `graph_create` | Create/validate/delete a graph from JSON spec or template |
85|| `graph_list` | List all registered graphs with metadata |
86|| `graph_inspect` | Full topology: nodes, edges, Mermaid diagram, hash, reducers |
87|| `graph_render` | Render as Mermaid diagram or JSON |
88|
89|### Execution (5 tools)
90|
91|| Tool | Description |
92||------|-------------|
93|| `graph_execute` | Normal execution is synchronous. Sync (blocking) or async execution |
94|| `graph_run_start` | Async start → returns `run_id` immediately |
95|| `graph_run_wait` | Block until terminal state with timeout |
96|| `graph_run_cancel` | Cooperative cancellation (best-effort) |
97|| `graph_run_get` | Current status, budget, pending approvals |
98|
99|### State & checkpointing (4 tools)
100|
101|| Tool | Description |
102||------|-------------|
103|| `graph_run_state` | Live in-memory state projection |
104|| `graph_run_events` | Replay event stream from cursor |
105|| `graph_run_checkpoint` | Durable checkpoint read with integrity verification |
106|| `graph_run_resume` | Atomic one-shot resume from deterministic-local checkpoint |
107|
108|### Approval & evidence (5 tools)
109|
110|| Tool | Description |
111||------|-------------|
112|| `graph_approval_list` | List pending/expired/resolved approvals |
113|| `graph_approval_get` | Read specific approval metadata |
114|| `graph_approval_request` | Create checkpoint-bound HITL approval |
115|| `graph_source_witness_capture` | Persist caller-supplied source content (HMAC-authenticated) |
116|| `graph_source_witness_get` | Read witness with authentication tag verification |
117|
118|### Templates & policy (4 tools)
119|
120|| Tool | Description |
121||------|-------------|
122|| `graph_template_list` | 5 built-in templates |
123|| `graph_template_instantiate` | Template → graph spec |
124|| `graph_template_candidates` | Promotion candidates |
125|| `graph_template_outcomes` | Recorded outcome history |
126|| `graph_policy_check` | Preflight validation against model/tool/data/budget policy |
127|
128|### Status & receipts (3 tools)
129|
130|| Tool | Description |
131||------|-------------|
132|| `g
```

## Workstream 5: /home/sikmindz/Coding/recursive-agent

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
M Cargo.lock
 M Cargo.toml
 M crates/recursive-agent-cli/Cargo.toml
 M crates/recursive-agent-cli/src/main.rs
 M crates/recursive-agent-contracts/Cargo.toml
 M crates/recursive-agent-contracts/src/lib.rs
 M crates/recursive-agent-ledger/Cargo.toml
 M crates/recursive-agent-ledger/src/lib.rs
 M crates/recursive-agent-policy/Cargo.toml
 M crates/recursive-agent-policy/src/lib.rs
 M crates/recursive-agent-provider/Cargo.toml
 M crates/recursive-agent-provider/src/lib.rs
 M crates/recursive-agent-runner/Cargo.toml
 M crates/recursive-agent-runner/src/lib.rs
 M crates/recursive-agent-sandbox/Cargo.toml
 M crates/recursive-agent-sandbox/src/lib.rs
 M crates/recursive-agent-tools/Cargo.toml
 M crates/recursive-agent-tools/src/lib.rs
?? .hermes/
?? crates/recursive-agent-cli/tests/
?? crates/recursive-agent-contracts/src/event.rs
?? crates/recursive-agent-contracts/src/operation.rs
?? crates/recursive-agent-contracts/tests/
?? crates/recursive-agent-daemon/
?? crates/recursive-agent-ledger/tests/
?? crates/recursive-agent-mcp/
?? crates/recursive-agent-mcts/
?? crates/recursive-agent-memory/
?? crates/recursive-agent-policy/tests/
?? crates/recursive-agent-provider/tests/
?? crates/recursive-agent-runner/src/deps.rs
?? crates/recursive-agent-runner/src/error.rs
?? crates/recursive-agent-runner/src/runtime.rs
?? crates/recursive-agent-runner/src/sandbox_engine.rs
?? crates/recursive-agent-runner/src/scheduler.rs
?? crates/recursive-agent-runner/tests/
?? crates/recursive-agent-sandbox/tests/
?? crates/recursive-agent-skills/
?? deny.toml
?? docs/claims.md
?? docs/owner-admission.md
?? docs/receipts/cross-phase-parallelization-advisory.md
?? docs/receipts/cross-phase-parallelization-receipt.json
?? docs/receipts/phase-0/
?? docs/receipts/phase-1/
?? docs/receipts/phase-2/
?? docs/receipts/phase-3/
?? docs/receipts/phase-4/
?? docs/receipts/phase-5/
?? docs/receipts/phase-6/
?? fuzz/
?? integrations/
?? scripts/install-hermes-plugin.sh
?? scripts/uninstall-hermes-plugin.sh
```
- Diff stat:
```text
Cargo.lock                                  |  927 ++++++--------
 Cargo.toml                                  |   12 +-
 crates/recursive-agent-cli/Cargo.toml       |   12 +
 crates/recursive-agent-cli/src/main.rs      |  309 ++++-
 crates/recursive-agent-contracts/Cargo.toml |    1 +
 crates/recursive-agent-contracts/src/lib.rs | 1257 ++++++++++++++++++-
 crates/recursive-agent-ledger/Cargo.toml    |    4 +-
 crates/recursive-agent-ledger/src/lib.rs    | 1744 +++++++++++++++++++++-----
 crates/recursive-agent-policy/Cargo.toml    |    4 +
 crates/recursive-agent-policy/src/lib.rs    | 1616 ++++++++++++++++++++++--
 crates/recursive-agent-provider/Cargo.toml  |    2 +-
 crates/recursive-agent-provider/src/lib.rs  |  496 +++++---
 crates/recursive-agent-runner/Cargo.toml    |   13 +-
 crates/recursive-agent-runner/src/lib.rs    | 1768 ++++++++++++++++++++++++---
 crates/recursive-agent-sandbox/Cargo.toml   |    6 +-
 crates/recursive-agent-sandbox/src/lib.rs   |  445 +++----
 crates/recursive-agent-tools/Cargo.toml     |    5 +-
 crates/recursive-agent-tools/src/lib.rs     |  316 ++++-
 18 files changed, 7207 insertions(+), 1730 deletions(-)
```
- Five latest commits:
```text
2026-08-03T23:35:33-05:00	3805f7a	feat: Phase 3 sandboxed tool execution (Landlock + user-ns)
2026-08-03T23:10:21-05:00	c857899	feat: Phase 2 provider integration + untrack build artifacts
2026-07-13T23:07:52-05:00	cfa8b7e	fix(gate): scope cargo fmt to local crates only (path-deps bleed)
2026-07-13T23:05:59-05:00	9787814	docs: M0 build report
2026-07-13T23:05:35-05:00	8d1b17b	feat: M0 provenance-native agent platform
```
### Tracked source excerpt: `AGENTS.md`

```text
1|# AGENTS.md — Recursive Agent Platform (M0)
2|
3|## Mission
4|
5|Build the smallest runnable vertical slice of a provenance-native agent
6|platform. M0 produces a tamper-evident receipt chain for a single
7|deterministic run, verifies it offline, and replays it from disk without any
8|provider or network call.
9|
10|## Doctrine (carried from RecursiveIntell)
11|
12|1. **Receipts are execution semantics.** Every state transition emits a
13|   typed receipt or an explicit non-durable/degraded outcome. A "completed"
14|   status without an inspectable chain is a false claim.
15|2. **Truth is append-only + supersession.** No silent destructive rewrite.
16|3. **Valid time and recorded time are distinct.**
17|4. **Material IDs come from `stack-ids`.** No process-local counters, no
18|   random UUIDs as material IDs. Family-qualified, parseable, stable.
19|5. **Boundary check at every typed ingress/egress.** Use
20|   `boundary-compiler` RFC 8785 JCS everywhere. Malformed input is a typed
21|   rejection, not a panic.
22|6. **Provider-free in M0.** No Ollama, no OpenAI-compatible call, no
23|   network. The product survives its own restart and verifies offline.
24|   **Phase 2 deliberately lifts this** for the `llm` tool only: provider
25|   calls are receipt-bearing and typed (see `recursive-agent-provider`),
26|   and the receipt chain still verifies offline. All other tools remain
27|   provider-free.
28|7. **Recorded replay only.** Do not promise "deterministic replay" of any
29|   LLM. Recorded replay is the only replay contract M0 offers. A
30|   provider-backed `llm` step records its response as a content-addressed
31|   artifact; replay re-emits that recorded output and never re-calls the
32|   provider.
33|8. **Bounded safety.** No `unsafe`, no `unwrap`/`expect` in lib code
34|   (`cargo clippy -D warnings`). Any panic is a bug.
35|9. **Source hierarchy.** This workspace depends on Libraries by **path**.
36|   No edits under `~/Coding/Libraries/`. AiDENs P32 is still
37|   `feature_expansion_allowed: false`.
38|
39|## Source-of-truth ownership
40|
41|| Concern | Owner | Adapter here |
42||---|---|---|
43|| Canonical JSON / boundary | `boundary-compiler` 0.1.0 | direct dep |
44|| Material IDs / digests | `stack-ids` 0.1.1 | direct dep |
45|| Bitemporal semantics | `bitemporal-runtime` 0.1.0 | direct dep (in-memory view in M0) |
46|| Claims / evidence | `claim-ledger` 0.1.0 | direct dep |
47|| Run orchestration | this workspace | new |
48|| Receipt chain | this workspace (`ledger` crate) | new |
49|| Tool plane | this workspace (`tools` crate) | new |
50|| Provider / LLM | `recursive-agent-provider` (new) | Ollama + OpenAI-compatible adapters |
51|| MCP / channel | none | out of scope M0/Phase 2 |
52|
53|## Receipt contract (M0)
54|
55|- `receipts.ndjson` under `<run-dir>/`.
56|- One receipt per line. Each line is JCS canonical JSON.
57|- Chain digest: `blake3(prev_chain_digest || jcs(receipt))`. Initial
58|  `prev_chain_digest = blake3(b"recursive-agent-m0-genesis")`.
59|- A separate `chain.meta` records genesis and final digest.
60|- A separate `artifacts/` directory holds content-addressed payloads.
61|- `ra verify <run-dir>` rewinds the chain and prints first divergence.
62|- `ra replay <run-dir>` re-emits observed payloads offline; it does not
63|  re-execute tools.
64|
65|## Hard-fail patterns
66|
67|- `unwrap` / `expect` / `panic!` in lib code (enforced by `clippy`).
68|- "ok" with `unwrap_or_default` in material paths.
69|- Provider calls anywhere.
70|- Mocks that hide the real chain digest.
71|- Disabling a check to pass CI.
72|- Random UUIDs in receipt identity (must be family-qualified).
73|- Two distinct digests that should agree.
74|
75|## Finish-line focus (M0)
76|
77|- `ra run`, `ra verify`, `ra replay`, `ra doctor` from a clean tree.
78|- `cargo test --workspace` green.
79|- A negative tampering test that fails verification with a precise error.
80|- All output captured under `docs/receipts/`.
81|
```
### Tracked source excerpt: `README.md`

```text
1|# Recursive Agent Platform (M0)
2|
3|> Local-first, provenance-native agent platform in Rust. This is **M0**: the
4|> smallest vertical slice that produces a tamper-evident receipt chain for
5|> a deterministic run, verifies it offline, and replays it from disk with
6|> no provider call.
7|
8|## What M0 is not
9|
10|- Not a Hermes or OpenClaw clone. It is a new platform that adopts useful
11|  *behaviors* (CLI, receipts, replay, scopes) without copying source,
12|  brand, or upstream contracts.
13|- Not a provider integration. No Ollama, no OpenAI-compatible call, no
14|  network. That is **Phase 2**, gated on M0 acceptance.
15|- Not a UI. CLI only.
16|- Not MCP. That is **Phase 3**.
17|- Not a sandboxed execution plane. That is **Phase 4**.
18|
19|## What M0 *is*
20|
21|A small Rust workspace at `~/Coding/recursive-agent/` that depends on
22|canonical Libraries crates by path:
23|
24|- `boundary-compiler` for RFC 8785 JCS at every typed boundary.
25|- `stack-ids` for family-qualified material IDs.
26|- `bitemporal-runtime` for valid-time / recorded-time semantics.
27|- `claim-ledger` for claim/evidence/provenance primitives.
28|- Local crates:
29|  - `recursive-agent-contracts` — typed protocol.
30|  - `recursive-agent-ledger` — append-only chain + content-addressed
31|    artifact store.
32|  - `recursive-agent-policy` — permits, lineage, allowlist.
33|  - `recursive-agent-tools` — `echo` and `time_now` manifests.
34|  - `recursive-agent-runner` — typed run DAG, deterministic walk.
35|  - `recursive-agent-cli` — `ra run`, `ra verify`, `ra replay`,
36|    `ra doctor`.
37|
38|## Quick start
39|
40|```bash
41|cd ~/Coding/recursive-agent
42|cargo build --release
43|./target/release/ra doctor
44|./target/release/ra run fixtures/hello-run.json
45|./target/release/ra verify <run-dir-printed-above>
46|```
47|
48|The first run prints a `<run-dir>` under
49|`~/.local/share/recursive-agent/runs/`. Capture stdout into
50|`docs/receipts/` so the chain can be reproduced.
51|
52|## Layout
53|
54|```text
55|recursive-agent/
56|├── crates/
57|│   ├── recursive-agent-contracts/
58|│   ├── recursive-agent-ledger/
59|│   ├── recursive-agent-policy/
60|│   ├── recursive-agent-tools/
61|│   ├── recursive-agent-runner/
62|│   └── recursive-agent-cli/
63|├── fixtures/
64|├── scripts/
65|├── docs/
66|│   ├── adr/
67|│   └── receipts/
68|├── AGENTS.md
69|└── Cargo.toml
70|```
71|
72|## Capability matrix
73|
74|| Capability | Source | M0 |
75||---|---|---|
76|| Canonical JSON boundary | `boundary-compiler` | yes |
77|| Family-qualified IDs | `stack-ids` | yes |
78|| Bitemporal | `bitemporal-runtime` | in-memory |
79|| Claim/evidence | `claim-ledger` | envelope only |
80|| Provider | none | out of scope |
81|| MCP | none | out of scope |
82|| Messaging | none | out of scope |
83|| Web UI | none | out of scope |
84|| Sandbox | none | out of scope |
85|
```

## Workstream 6: /home/sikmindz/Coding/agent-memory-kits

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
M shared/scripts/run-server.sh
?? benchmarks/
?? claude/plugins/semantic-memory/.mcp.json.bak-pre-grok-install
?? docs/benchmarks/BENCHMARK_READINESS_2026-08-06.md
?? docs/benchmarks/longmemeval-official/
?? shared/scripts/benchmark-longmemeval-semantic-memory.py
```
- Diff stat:
```text
shared/scripts/run-server.sh | 5 +++++
 1 file changed, 5 insertions(+)
```
- Five latest commits:
```text
2026-08-03T18:02:28-05:00	44c758d	chore: snapshot workspace state — openapi spec
2026-08-02T19:54:28-05:00	a87af1f	fix: enforce governed agent profile and authority token forwarding
2026-07-27T09:38:30-05:00	d828f60	fix: add MCP relay port support, rewrite Codex capture nudge, add audit remediation plans
2026-07-21T22:56:06-05:00	8ca4436	docs: add agent-driven setup note to all install flows
2026-07-21T22:54:51-05:00	f0b03a3	docs: mention agent-driven mnemes setup
```
### Tracked source excerpt: `README.md`

```text
1|# agent-memory-kits
2|
3|> **Persistent local-first memory, receipt-backed compaction, and claim/evidence provenance — for every AI coding agent.**
4|> One repo, three companion MCP servers, nine agent hosts.
5|
6|[![crates.io: semantic-memory-mcp](https://img.shields.io/crates/v/semantic-memory-mcp?label=semantic-memory-mcp&style=for-the-badge)](https://crates.io/crates/semantic-memory-mcp)
7|[![crates.io: semantic-memory](https://img.shields.io/crates/v/semantic-memory?label=semantic-memory&style=for-the-badge)](https://crates.io/crates/semantic-memory)
8|[![crates.io: context-governor](https://img.shields.io/crates/v/context-governor?label=context-governor&style=for-the-badge)](https://crates.io/crates/context-governor)
9|[![crates.io: claim-ledger](https://img.shields.io/crates/v/claim-ledger?label=claim-ledger&style=for-the-badge)](https://crates.io/crates/claim-ledger)
10|[![9 host plugins](https://img.shields.io/badge/hosts-9-blueviolet?style=for-the-badge)](./#capability-matrix)
11|[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)](#license)
12|[![Local-first](https://img.shields.io/badge/data-100%25%20local-green?style=for-the-badge)](#privacy--local-first)
13|
14|## Verified release surface
15|
16|The companion packages are published independently from this kit. The versions below were checked against crates.io on **2026-07-18**; badges remain the live version indicator.
17|
18|| Package | Published version | Role | Source / release boundary |
19||---|---:|---|---|
20|| [`semantic-memory`](https://crates.io/crates/semantic-memory) | `0.5.14` | SQLite/FTS5 + vector memory library | [release source](https://github.com/RecursiveIntell/semantic-memory/tree/feat/full-integration) |
21|| [`semantic-memory-mcp`](https://crates.io/crates/semantic-memory-mcp) | `0.5.6` | MCP transport, tool profiles, and loopback HTTP | [release source](https://github.com/RecursiveIntell/semantic-memory-mcp/tree/main) |
22|| [`mnemes`](https://crates.io/crates/mnemes) | `0.1.1` | Multi-device memory control plane | [release source](https://github.com/RecursiveIntell/mnemes) |
23|| [`context-governor`](https://crates.io/crates/context-governor) | `0.2.0` | Deterministic receipt-backed compaction | [registry package](https://crates.io/crates/context-governor) |
24|| [`claim-ledger`](https://crates.io/crates/claim-ledger) | `0.2.1` | Claim/evidence/provenance ledger | [Libraries source](https://github.com/RecursiveIntell/Libraries/tree/main/claim-ledger) |
25|
26|Release facts are source-reported until reproduced locally. For a current runtime surface, use `tools/list` on the configured MCP binary; profile counts are deliberately not frozen in this README.
27|
28|![Architecture overview](.github/hero.svg)
29|
30|AI coding agents forget everything between sessions. This repo fixes that.
31|
32|## The memory builds over time
33|
34|Day 1 is empty. That is by design, not a bug. The recall hook gates on `SM_RECALL_MINTOP=0.58` cosine — an empty store returns nothing, and the hook fails open (no output, no block) on every prompt until the store has facts worth recalling. The system is not failing; it is waiting.
35|
36|The product is the compounding curve, not the first session.
37|
38|```
39|day 1        day 7         day 30        day 90+
40|  |           |              |              |
41|  o-----------o--------------o--------------o-->
42|  install     ~50 facts     ~500 facts    ~5000+ facts
43|  empty store starting to   recall        recall
44|              fill          useful        indispensable
45|```
46|
47|**What to expect, honestly:**
48|
49|- **Day 1 (install day).** Empty store. The recall hook fires on every prompt and returns nothing every time. The MCP tools work. The doctor passes. Nothing to recall. This is correct.
50|- **Days 2–14 (filling in).** The agent saves facts as it works — with judgment, never auto-dumped. `/memory-ingest <repo>` on each repo you touch populates the codebase namespace fast. Recall starts firing on the prompts where it has a hit, ignoring the rest. The user notices on a few specific questions.
51|- **Days 15–60 (useful).** Recall fires on a meaningful fraction of prompts. The agent knows your stack, your conventions, your open questions. You stop restating context the agent should already have.
52|- **Days 60+ (indispensable).** The agent answers cross-session questions that you would have to look up manually. Failed approaches don't get retried. Decisions don't get re-debated. The store is large enough that the cosine gate fires often and the answers are accurate.
53|
54|**What speeds the curve (do these on day 1):**
55|
56|```bash
57|# 1. Install the three companion MCP servers
58|cargo install semantic-memory-mcp context-governor claim-ledger
59|
60|# 2. Install a host plugin — Claude Code shown; the same shape works for all 9 hosts
61|/plugin marketplace add RecursiveIntell/agent-memory-kits
62|/plugin install semantic-memory@semantic-memory-kit
63|/memory-setup
64|
65|# 3. Ingest the repos you actually work in
66|/memory-ingest .
67|/memory-ingest ../other-repo
68|
69|# 4. Restart the host so hooks load. Then work normally.
70|```
71|
72|The hooked host's recall hook queries the warm HTTP server (BM25 + vector + RRF, fail-open) and injects only hits that clear `SM_RECALL_MINTOP=0.58`. A second-prompt later, the same facts come back without re-indexing. Receipts are written to `~/.local/share/semantic-memory-agent-kits/receipts/`. The day-1 install is the same in every README; the difference between day 1 and day 90 is what you do between.
73|
74|---
75|
76|## Table of contents
77|
78|- [What this repo is](#what-this-repo-is)
79|- [Architecture](#architecture)
80|- [Capability matrix](#capability-matrix)
81|- [Per-host docs](#per-host-docs)
82|- [Install](#install)
83|- [RecursiveIntell Pro](#recursiveintell-pro)
84|- [The three MCP companions](#the-three-mcp-companions)
85|- [The codebase ingester](#the-codebase-ingester
```
### Tracked source excerpt: `claude/README.md`

```text
1|# semantic-memory for Claude Code
2|
3|> **Tier 0 reference implementation.** Lifecycle hooks (SessionStart / UserPromptSubmit / PreCompact / Stop), a memory-keeper subagent, capture/curator/maintenance/sync skills, and manifest-declared commands — over `semantic-memory-mcp` (profile-based tool counts, run `generate-tool-surface-docs.py` for current) + `context-governor` (13 CLI commands) + `claim-ledger` (5 tools).
4|> Plugin marketplace path: `semantic-memory@semantic-memory-kit`.
5|
6|[![Tier 0](https://img.shields.io/badge/tier-0-blueviolet?style=for-the-badge)](#tier--scope)
7|[![Local-first](https://img.shields.io/badge/data-100%25%20local-green?style=for-the-badge)](#)
8|[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)](#)
9|[![semantic-memory-mcp](https://img.shields.io/crates/v/semantic-memory-mcp?label=semantic-memory-mcp&style=for-the-badge)](https://crates.io/crates/semantic-memory-mcp)
10|[![context-governor](https://img.shields.io/crates/v/context-governor?label=context-governor&style=for-the-badge)](https://crates.io/crates/context-governor)
11|[![claim-ledger](https://img.shields.io/crates/v/claim-ledger?label=claim-ledger&style=for-the-badge)](https://crates.io/crates/claim-ledger)
12|
13|See the [top-level README](../README.md) for the full capability matrix, architecture overview, and Tier 0 vs Tier 1 distinction.
14|
15|## Tier / scope
16|
17|Tier 0 host plugin. This kit is the **reference implementation** that Tier 1 hosts (Cursor, Cline, Roo Code, Windsurf, Continue, OpenCode) reuse. The Tier 0 contract: real lifecycle hooks fire on SessionStart, UserPromptSubmit, PreCompact, and Stop, with deterministic fail-open behavior; capture is model-nudged (the model writes with judgment, not auto-dumped); and every claim of completion is backed by a receipt.
18|
19|## Architecture
20|
21|![Tier 0 hooked host architecture](../docs/assets/tier0-hooked-architecture.svg)
22|
23|Hook paths: `claude/plugins/semantic-memory/hooks/`. Script paths: `claude/plugins/semantic-memory/scripts/`. Skill paths: `claude/plugins/semantic-memory/skills/`. All relative to repo root.
24|
25|## Install
26|
27|From the repo root:
28|
29|```text
30|/plugin marketplace add RecursiveIntell/agent-memory-kits
31|/plugin install semantic-memory@semantic-memory-kit
32|/memory-setup
33|```
34|
35|Restart Claude Code once so hooks load. `/memory-setup` installs the binary and allowlists tools.
36|
37|## What you get
38|
39|### Hooks (4)
40|
41|`claude/plugins/semantic-memory/hooks/hooks.json` wires four lifecycle hooks. Every hook **fails open** — missing binary, timeout, or bad JSON exits 0 and never blocks the prompt.
42|
43|| Hook | Event | What it does | Fail-open |
44||---|---|---|---|
45|| `memory-primer.sh` | `SessionStart` (startup, resume, clear) | Injects project-scoped primer facts as `additionalContext` | yes — 12s timeout |
46|| `memory-recall.sh` | `UserPromptSubmit` | Queries warm HTTP `/search` (BM25 + vector + RRF), injects hits that clear `SM_RECALL_MINTOP=0.58` as `additionalContext` | yes — 12s timeout |
47|| `memory-capture-nudge.sh` | `PreCompact` and `Stop` | Reminds the model to save durable facts / decisions before the conversation ends or compacts | yes — 5s timeout |
48|| `_resolve.sh` | helper, not a hook event | Resolves the plugin's `${CLAUDE_PLUGIN_ROOT}` to the absolute path so siblings can find binaries | n/a |
49|
50|### Scripts
51|
52|`claude/plugins/semantic-memory/scripts/` includes MCP wrappers, doctor/benchmark helpers, ingestion, proof/evidence helpers, admin server launchers, and context-governor audit wrappers. Avoid hardcoded script counts here; the script directory is the source of truth.
53|
54|- `context-governor-mcp.py` — MCP server entry for `context-governor` (4 `cg_*` tools)
55|- `claim-ledger-mcp.py` — MCP server entry for `claim-ledger` (5 `cl_*` tools)
56|- `context-governor-compact.py` — deterministic transcript compaction, writes receipt
57|- `doctor-all.py` — runs all kit doctors and writes a JSON receipt bundle
58|- `benchmark-retrieval.py` — quality benchmark over warm HTTP
59|- `benchmark-context-governor.py` — compaction latency / ratio benchmark
60|- `ingest_codebase.py` — language-agnostic repo ingester
61|- `evidence-workbench.py`, `proof-packet.py` — proof/evidence packet helpers
62|- `context-governor-audit.py` — context-governor audit wrapper
63|- `run-server.sh`, `run-server-admin.sh` — daily and admin semantic-memory launchers
64|
65|### Commands (2)
66|
67|- `/memory-setup` — install binary, allowlist tools, write rules (see `claude/plugins/semantic-memory/commands/memory-setup.md`)
68|- `/memory-ingest <path>` — run `ingest_codebase.py` on a repo path (see `claude/plugins/semantic-memory/commands/memory-ingest.md`)
69|
70|### Agent (1)
71|
72|- `memory-keeper.md` — subagent that audits memory health, runs the curator, and re-anchors stale facts
73|
74|### Skills (9)
75|
76|Each skill is `claude/plugins/semantic-memory/skills/<name>/SKILL.md`:
77|
78|| Skill | Purpose |
79||---|---|
80|| `memory-capture` | When and how to save durable facts and decisions |
81|| `memory-curator` | Reconcile duplicates, supersede stale facts, prune contradicted records |
82|| `memory-maintenance` | Vacuum, re-embed stale vectors, run `doctor-all` |
83|| `memory-sync` | Promote facts across namespaces; pair with `ingest_codebase.py` |
84|| `knowledge-graph-explorer` | Use `sm_topology`, `sm_communities`, `sm_factor_graph` for second-order discovery |
85|| `release-gate` | Run `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace` and store receipts |
86|| `context-compaction` | Drive `context-governor-compact.py` before manual or auto compaction |
87|| `claim-provenance` | Back material assertions with `cl_run` / `cl_evidence` / `cl_verify` |
88|| `llm-output-parsing` | Use the `sm_parse_*` tools to handle think blocks, malformed JSON, trailing text |
89|
90|### MCP tools exposed
91|
92|The `seman
```
### Tracked source excerpt: `cline/README.md`

```text
1|# semantic-memory for Cline
2|
3|> **Tier 1 host plugin.** MCP-only integration; rule/context injection for behavioral guidance.
4|
5|[![Tier 1](https://img.shields.io/badge/tier-1-blueviolet?style=for-the-badge)](#capability-boundary)
6|[![Local-first](https://img.shields.io/badge/data-100%25%20local-green?style=for-the-badge)](#)
7|[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)](#)
8|
9|See [top-level README](../README.md) for the full capability matrix and architecture overview.
10|
11|This is the Cline MCP setup kit for semantic-memory-mcp.
12|
13|Capability boundary:
14|- Works: exposes the `sm_*` semantic-memory MCP tools to Cline once the MCP config is registered.
15|- Works: local-first memory storage, hybrid search, graph tools, provenance, supersession, claims, and manual/codebase-ingest workflows.
16|- Works: context-injection via host rule/instruction files. The setup kit can install a semantic-memory rule that tells the agent to retrieve memory through MCP, or through the shared context command when shell execution is available.
17|- Boundary: this is rule/instruction based for this host, not a guaranteed pre-prompt hook unless the host exposes a stable hook API.
18|
19|> **This is a Tier 1 kit.** Tier 1 hosts expose the MCP server to the agent and install host-native rule/instruction files that tell the agent to retrieve memory through MCP and preserve receipts. No transcript/prompt lifecycle hook is claimed.
20|
21|## Install
22|
23|From the repository root:
24|
25|```bash
26|cline/scripts/setup.sh
27|```
28|
29|Copy the printed `mcpServers.semantic-memory` snippet into Cline MCP settings.
30|
31|## Verify
32|
33|```bash
34|cline/scripts/doctor.py
35|```
36|
37|Expected:
38|- `mcp_settings.json.example` parses as JSON.
39|- `semantic-memory-mcp` binary is found.
40|- memory dir exists.
41|- MCP `tools/list` exposes `sm_search`, `sm_add_fact`, `sm_stats`, and `sm_supersede_fact`.
42|
43|## Use inside Cline
44|
45|Ask Cline to call the semantic-memory MCP tools, for example:
46|
47|```text
48|Search semantic memory for facts about this repository before changing code.
49|```
50|
51|or:
52|
53|```text
54|Save this decision to semantic memory with namespace code:<repo-name> and source Cline.
55|```
56|
57|## Notes
58|
59|If the warm HTTP health check warns, MCP stdio can still work. Warm HTTP is mainly for hook-based hosts; MCP tool use does not require it.
60|
61|
62|## Context injection
63|
64|Install a workspace rule into a project:
65|
66|```bash
67|shared/scripts/install-context-rules.py cline --scope workspace --workspace /path/to/project
68|```
69|
70|Install a global rule where the host has a documented global-rule location:
71|
72|```bash
73|shared/scripts/install-context-rules.py cline --scope global
74|```
75|
76|The installed rule points at:
77|
78|```bash
79|shared/scripts/semantic-memory-context.py --prompt "$USER_TASK"
80|```
81|
82|That command queries the warm HTTP server first (`SEMANTIC_MEMORY_HTTP_PORT`, default `1739`) and falls back to stdio MCP. Returned entries are explicitly marked as recall, not ground truth.
83|
84|
85|## Context compaction / receipts
86|
87|This kit also includes Context Governor as a companion MCP server and rule layer.
88|
89|- MCP server: `shared/scripts/context-governor-mcp.py`
90|- Receipt-backed compact command: `shared/scripts/context-governor-compact.py`
91|- Rule text: `shared/rules/context-governor.md`
92|
93|Use it when a Cline session is long, a handoff is needed, or context is about to be compacted. It preserves high-risk context and stores exact fallback receipts that can be searched and expanded later.
94|
95|Boundary: for hosts without a verified pre-compact hook, this is rule/command/MCP assisted. It does not claim automatic transcript capture unless the host exposes transcript messages to an extension/hook API.
96|
97|
98|## Quick install
99|
100|Print config snippets only:
101|
102|```bash
103|cline/scripts/setup.sh
104|```
105|
106|Write project-local rule/config files:
107|
108|```bash
109|cline/scripts/setup.sh --write-project /path/to/project
110|```
111|
112|Write safe user/global rule files where this host supports them:
113|
114|```bash
115|cline/scripts/setup.sh --write-user
116|```
117|
118|Dry run before writing:
119|
120|```bash
121|cline/scripts/setup.sh --dry-run --write-project /path/to/project
122|```
123|
124|Verify:
125|
126|```bash
127|cline/scripts/doctor.py
128|shared/scripts/doctor-all.py --deep
129|```
130|
131|## Architecture
132|
133|![Tier 1 MCP architecture](../docs/assets/tier1-mcp-architecture.svg)
134|
135|## Design principles
136|
137|- **Rule-injection, not hook-injection.** Tier 1 hosts install host-native rule files that tell the agent to retrieve memory through MCP; no pre-prompt hook is claimed.
138|- **MCP stdio is the only lifecycle path.** The host starts `semantic-memory-mcp` when it loads the MCP config; no warm HTTP sidecar is started by this host.
139|
140|These extend the [top-level Design principles](../README.md#design-principles); they don't replace them.
141|
142|## Troubleshooting
143|
144|| Symptom | Fix |
145||---|---|
146|| `mcp_settings.json.example` not parseable | `python3 -m json.tool cline/mcp_settings.json.example` — should print valid JSON. |
147|| MCP not loading in Cline | Restart Cline after writing the MCP config; check Cline's MCP logs. |
148|| Rule not auto-applying | Verify the rule path with `cline/scripts/setup.sh --write-user` produced the expected rule file. |
149|
```

## Workstream 7: /home/sikmindz/Coding/mnemes

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
(clean)
```
- Diff stat:
```text
(none)
```
- Five latest commits:
```text
2026-08-02T20:09:45-05:00	d38659b	docs: preserve Mnemes synchronization evidence
2026-08-02T20:03:22-05:00	98c8492	feat(replication): add governed fact-create transport and durable retries
2026-07-30T18:56:53-05:00	37feeb8	docs: add full-surface memory mesh execution plan
2026-07-28T22:19:36-05:00	1181212	feat: harden feature-preserving sync recovery
2026-07-26T20:02:54-05:00	8ec942c	docs: bump semantic-memory version refs to 0.6.0
```
### Tracked source excerpt: `.hermes/plans/mnemes-memory-mesh-lanes/implementation-packs/README.md`

```text
1|# Cheap-Model Implementation Packs — Controller Entry Point
2|
3|> **Purpose:** Make the six memory-mesh lanes safe enough for lower-cost implementation models without granting architectural, release, deployment, or cross-lane authority.
4|> **Source authority:** `../00-lane-map.md` and `../../2026-07-30-mnemes-full-surface-memory-mesh.md`. This directory is an execution aid, not competing architecture.
5|
6|## Non-negotiable controller model
7|
8|- A worker receives **one lane**, one clean isolated worktree per repository, one pinned commit, one explicit allowlist, and one timebox.
9|- Workers may write only the allowlisted paths and only in their isolated worktrees. They must not commit, push, install, activate services, change global Hermes/MCP configuration, touch real device-primary data, or use a destructive cleanup command.
10|- Workers implement a finite task from the corresponding lane plan. They do not reinterpret the architecture, add transport/protocol features, or make public/reliability claims.
11|- A controller, not the worker, owns cross-lane integration, contract changes, test acceptance, conflict resolution, merge/cherry-pick, canary, and rollback decisions.
12|
13|## What is ready now
14|
15|- `00-worker-contract.md`: copy into every worker prompt.
16|- `worker-preflight.sh`: proves the worker is in the intended worktree at the pinned HEAD and prints scoped initial state.
17|- `worker-final-guard.sh`: rejects a moved HEAD, out-of-scope changes, whitespace errors, or an absent test receipt.
18|- `receipt-template.md`: exact handoff shape.
19|- `10-lane-0-source-anchors.md` through `15-lane-5-source-anchors.md`: source-verified entry points, tests, and stop conditions for every lane.
20|- `02-controller-acceptance.md`, `04-dispatch-order.md`, and `06-worker-prompt-template.md`: controller-only acceptance, model-tier assignment, and ready-to-dispatch task shape.
21|
22|## Required controller sequence
23|
24|1. Pick the **smallest task block** from one lane plan—not an entire lane.
25|2. Create clean worktrees at recorded commits. Do not delegate into either currently dirty canonical checkout.
26|3. Copy the lane plan, this contract, exact source-anchor pack, and an allowlist into the worker prompt.
27|4. Run `bash worker-preflight.sh <worktree> <expected-head> -- <allowed paths...>` before worker edits.
28|5. Require RED → minimal GREEN → targeted regression. A pre-existing green suite does not close a requested behavior.
29|6. On return, run `bash worker-final-guard.sh ...` and independently rerun the named tests from the same worktree.
30|7. Record every acceptance row as verified, failed, skipped, blocked, or not implemented. Only then integrate via a controller-owned worktree.
31|
32|## Branching rule
33|
34|Lane 0 must produce Gate 1 before workers can implement runtime integration. After Gate 1, Lanes 1, 2, and 3 can work in parallel on non-overlapping paths. Lane 4 cannot claim integrated behavior until it consumes published Lane 1/2 interfaces. Lane 5 is test/evidence-only until all functional gates pass.
35|
36|## Stop conditions
37|
38|Stop immediately and return a blocker instead of guessing if the worker discovers: a missing or incompatible public API; a required shared-file edit; a mutation path that bypasses the canonical owner transaction; a different signed-byte contract; a failing test environment; an unexpected HEAD; or a need to use a live server/device database.
39|
```
### Tracked source excerpt: `README.md`

```text
1|<div align="center">
2|
3|# Mnemes
4|
5|### Multi-device memory control plane for local-first AI agents
6|
7|**Device-owned · Bitemporal · Provenance-backed · Idempotent**
8|
9|[![crates.io](https://img.shields.io/crates/v/mnemes.svg?style=flat-square&color=6c5ce7)](https://crates.io/crates/mnemes)
10|[![docs.rs](https://img.shields.io/docsrs/mnemes?style=flat-square&color=74b9ff)](https://docs.rs/mnemes)
11|[![license](https://img.shields.io/badge/license-Apache--2.0-00b894?style=flat-square)](LICENSE)
12|[![Rust](https://img.shields.io/badge/rust-1.75%2B-f76707?style=flat-square)](https://www.rust-lang.org/)
13|[![semantic-memory](https://img.shields.io/badge/powered%20by-semantic--memory-a29bfe?style=flat-square)](https://github.com/RecursiveIntell/semantic-memory)
14|
15|</div>
16|
17|---
18|
19|<p align="center">
20|  <img src="docs/architecture.svg" alt="Mnemes architecture diagram" width="100%">
21|</p>
22|
23|---
24|
25|## Table of Contents
26|
27|1. [Overview](#overview)
28|2. [Quick Start: Set Up a Server](#set-up-a-shared-memory-server)
29|3. [Architecture](#architecture)
30|4. [API Surface](#api-surface)
31|5. [Use as a Library](#use-as-a-library)
32|6. [The Semantic-Memory Engine](#the-semantic-memory-engine)
33|7. [Retrieval Pipeline](#retrieval-pipeline)
34|8. [Knowledge Graph](#knowledge-graph)
35|9. [Trust & Provenance](#trust--provenance)
36|10. [Data Model](#data-model)
37|11. [Memory Lifecycle](#memory-lifecycle)
38|12. [Performance & Scaling](#performance--scaling)
39|13. [Security & Governance](#security--governance)
40|
41|---
42|
43|## Overview
44|
45|**Mnemes** (from Greek μνήμη, "memory") is a Rust crate that adds a multi-device identity, synchronization, and routing layer on top of [`semantic-memory`](https://github.com/RecursiveIntell/semantic-memory). It enables a routing brain where laptops, GPU servers, edge devices, and phones can share authorized search results from separate device-owned stores while preserving full provenance:
46|
47|| Capability | What it means |
48|| --- | --- |
49|| **Device identity** | Every memory item is tagged with which device observed or submitted it |
50|| **Actor identity** | Every operation records which agent, process, or human was responsible |
51|| **Operation provenance** | Durable envelopes with idempotency keys, content digests, and receipt IDs |
52|| **Bitemporal lineage** | When the observation was made (`valid_time`) vs. when the server recorded it (`recorded_at`) |
53|| **Server-owned timestamps** | `recorded_at` is always stamped by the accepting server — never trusted from clients |
54|| **Sparse shard routing** | Query-time ranking of device shards by token overlap + locality, with durable receipts |
55|| **Signed replication** | Ed25519-signed mutation envelopes for device-to-server journal replay (in development) |
56|
57|> **Architecture status:** The current candidate implements server-side per-device shards and sparse routing. The target design keeps each canonical database on its home device and synchronizes a durable server replica. Continuous replication is under development — see [docs/DEVICE_OWNED_REPLICATED_MEMORY.md](docs/DEVICE_OWNED_REPLICATED_MEMORY.md).
58|
59|### The Full Stack
60|
61|Mnemes is the **product surface** of a three-crate stack:
62|
63|| Crate | Version | Role |
64||-------|---------|------|
65|| [`semantic-memory`](https://crates.io/crates/semantic-memory) | v0.5.14 | Core library: SQLite store, HNSW vectors, FTS5 search, knowledge graph, trust ledger |
66|| [`semantic-memory-mcp`](https://crates.io/crates/semantic-memory-mcp) | v0.5.6 | MCP server: runtime-profiled tools for AI agents via stdio JSON-RPC |
67|| **`mnemes`** (this crate) | v0.1.1 | Multi-device control plane: identity, routing, replication, pooled memory |
68|
69|## How it works
70|
71|Mnemes is **additive metadata** on top of semantic-memory. It does not duplicate memory payloads. Two storage layers coexist:
72|
73|```
74|pooled.db  ←  device/actor/operation/provenance/routing control plane
75|    │
76|    ├── devices (identity, status, credentials)
77|    ├── actors (agent kind, tool profile, device binding)
78|    ├── operation_envelopes (idempotent, receipted)
79|    ├── provenance_edges (bitemporal lineage graph)
80|    └── routing + sync receipts
81|    │
82|    ▼
83|memory/shards/<device_uuid>/memory.db  ←  one semantic-memory store per device
84|    │
85|    ├── facts, documents, episodes, conversations
86|    ├── embeddings, FTS5 indexes, vector (HNSW)
87|    └── provenance, authority, search receipts
88|```
89|
90|The control plane and semantic stores are **physically separate**. `pooled.db` owns pooling metadata and receipts. Each `memory.db` is owned by the `semantic-memory` engine. Once replication is implemented, the home-device generation is canonical and the server generation is a replayable replica.
91|
92|### Embedding provider selection
93|
94|Mnemes keeps the embedding provider behind `semantic_memory::Embedder`:
95|
96|- local deployments default to the in-process Candle provider when the default `candle-local` feature is enabled;
97|- shared-pool operators may select Ollama/HTTP with `MNEMES_EMBEDDER=ollama`;
98|- library users may inject any provider implementation with `MnemesStore::open_with_embedder`;
99|- the witnessed search endpoint now routes through `MnemesStore::routed_search()` when active shards with facts are registered, falling back to legacy single-store search only in test/single-device mode;
100|- future peer-first routing will select a compatible connected provider before invoking the UNO Q/local fallback.
101|
102|```bash
103|# Local default: Candle/Nomic, no Ollama service required
104|cargo run --bin mnemes-server
105|
106|# Select an HTTP/Ollama-compatible provider for a shared pool
107|MNEMES_EMBEDDER=ollama \
108|MNEMES_OLLAMA_URL=http://127.0.0.1:11434 \
109|MNEMES_EMBEDDING_MODEL=nomic-embed-text \
110|MNEMES_EMBEDDING_DIMENSIONS=768 \
111|cargo run --bin mnemes-serv
```
### Tracked source excerpt: `.hermes/plans/2026-07-21_213500-mnemes-completion-routing-parity.md`

```text
1|# Mnemes Completion Plan — Shard Routing Wire-Up + MSI Parity + Verification
2|
3|> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.
4|
5|**Goal:** Wire the existing `routed_search` path into the public witnessed-search endpoint, prove it returns data through the live MSI service, and verify the full mnemes ecosystem is in parity and clean.
6|
7|**Architecture:** `MnemesStore::routed_search()` in `src/store.rs` is the canonical routing owner (typed `ShardRoutingReceipt`, bounded expansion, conflict scanning). `src/shard.rs::ShardRouter` is a duplicate implementation that should be removed or consolidated. The HTTP/MCP `run_witnessed_search()` in `src/server.rs` currently bypasses routing and calls `state.store.memory()` (legacy global path). The fix is to make `run_witnessed_search()` call `routed_search()` when shards are registered, falling back to legacy only for single-device/test mode.
8|
9|**Tech Stack:** Rust 2021, Axum 0.7, semantic-memory 0.5, rusqlite, tokio, Candle embedder, nomic-embed-text 768d
10|
11|---
12|
13|## Current State (observed 2026-07-21)
14|
15|- **Local repo:** `/home/sikmindz/Coding/mnemes`, `main` at `9f39b49`, clean except uncommitted `src/lib.rs` (added `pub mod shard`) and `src/shard.rs` (compile fixes: borrow scope, `Projection` arm, selection lifetime)
16|- **MSI repo:** `~/Coding/mnemes` at `9f39b49`, `mnemes.service` active
17|- **MSI shard catalog:** 2 shards — `bb18a9fd...` (active, 1009 facts, 35 namespaces) + `c8501f21...` (active, 0 facts)
18|- **MSI env:** `MNEMES_PORT`, `MNEMES_DATA_DIR` in server.env; `MNEMES_URL=http://127.0.0.1:1738` in client.env
19|- **Stale references:** Zero `pooled-memory`/`pooled_memory` references in active source or config
20|- **Legacy dirs:** `pooled-memory.legacy-20260721` (archived), `mnemes-shard-candidate` (renamed)
21|- **Compile:** `cargo check --all-targets` passes; `cargo fmt --check` has one formatting diff in `mnemes-admin.rs`
22|- **Tests:** Last full run passed with `--test-threads=1`; not rerun after shard.rs edits
23|- **Key defect:** `run_witnessed_search()` calls `state.store.memory().search_with_context()` — the legacy global path. It does NOT call `routed_search()`. This is the "coded but not wired" gap.
24|- **Duplicate:** `src/shard.rs` (`ShardRouter`) duplicates `src/shards.rs` + `store.rs::routed_search()`. Must consolidate.
25|
26|## Constraints
27|
28|- `semantic-memory` is canonical semantic authority; mnemes is control plane only
29|- Candle is local default; Ollama selectable; any embedder injectable via `open_with_embedder`
30|- Matryoshka is opt-in feature; nomic-embed-text 768d → 256d truncation for UNO Q
31|- Peer-first embedding routing is a future phase, not this plan
32|- Tests must run with `--test-threads=1` (port conflicts in parallel)
33|- No live SQLite rsync — use `.backup` API
34|- `src/shard.rs` should be removed after confirming nothing depends on it
35|
36|---
37|
38|## Phase 0: Pre-flight and Cleanup
39|
40|### Task 0.1: Preserve current state as receipt
41|
42|**Objective:** Capture exact dirty state before any changes.
43|
44|**Files:** None (read-only)
45|
46|**Step 1:** Capture state
47|```bash
48|cd /home/sikmindz/Coding/mnemes
49|git status --short --branch > /tmp/mnemes-preflight-status.txt
50|git diff --stat >> /tmp/mnemes-preflight-status.txt
51|git rev-parse HEAD >> /tmp/mnemes-preflight-status.txt
52|```
53|
54|**Step 2:** Verify current compile passes
55|```bash
56|cargo check --all-targets 2>&1 | tail -5
57|```
58|Expected: `Finished` with only the `dead_code` warning on `ShardCache::len`
59|
60|### Task 0.2: Fix formatting
61|
62|**Objective:** Clear the `cargo fmt --check` failure.
63|
64|**Files:**
65|- Modify: `src/bin/mnemes-admin.rs` (formatting)
66|
67|**Step 1:** Run formatter
68|```bash
69|cargo fmt
70|```
71|
72|**Step 2:** Verify
73|```bash
74|cargo fmt --check
75|```
76|Expected: exit 0, no output
77|
78|### Task 0.3: Remove duplicate `src/shard.rs` module
79|
80|**Objective:** Eliminate the duplicate `ShardRouter` implementation. The canonical routing lives in `store.rs::routed_search()` + `shards.rs` types.
81|
82|**Files:**
83|- Modify: `src/lib.rs` — remove `pub mod shard;` line
84|- Delete: `src/shard.rs` — entire file (903 lines of duplicate code)
85|- Modify: `src/store.rs` — remove any imports from `crate::shard` if present
86|
87|**Step 1:** Check for any imports of `crate::shard`
88|```bash
89|grep -rn 'use crate::shard' src/ tests/
90|```
91|Expected: no matches (store.rs uses `crate::shards`, not `crate::shard`)
92|
93|**Step 2:** Remove the module declaration
94|```rust
95|// src/lib.rs — remove this line:
96|pub mod shard;
97|```
98|
99|**Step 3:** Delete the file
100|```bash
101|rm src/shard.rs
102|```
103|
104|**Step 4:** Verify compile
105|```bash
106|cargo check --all-targets
107|```
108|Expected: `Finished`, no errors. The `ShardCache::len` dead_code warning should also be gone.
109|
110|**Step 5:** Commit
111|```bash
112|git add src/lib.rs src/shard.rs
113|git commit -m "refactor: remove duplicate shard.rs module — canonical routing is store::routed_search"
114|```
115|
116|---
117|
118|## Phase 1: Wire Routed Search into Witnessed Search
119|
120|### Task 1.1: Add `has_shards()` helper to MnemesStore
121|
122|**Objective:** Provide a fast check for whether any active shards are registered, so `run_witnessed_search` can decide whether to route or fall back.
123|
124|**Files:**
125|- Modify: `src/store.rs` (add method after `aggregate_shard_stats` ~line 1648)
126|
127|**Step 1:** Write failing test
128|
129|Add to `tests/device_shards.rs`:
130|```rust
131|#[tokio::test]
132|async fn has_shards_returns_false_for_empty_store() {
133|    let tmp = tempfile::tempdir().unwrap();
134|    let store = MnemesStore::open(tmp.path()).await.unwrap();
135|    assert!(!store.has_shards().await.unwrap());
136|}
137|```
138|
139|**Step 2:** Run test to verify failure
140|```bash
141|cargo test has_sha
```

## Workstream 8: /home/sikmindz/Coding/RecursiveOps/recursiveops

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
(clean)
```
- Diff stat:
```text
(none)
```
- Five latest commits:
```text
2026-08-02T19:59:32-05:00	d909cde	feat(recursiveops): improve LLM controls and operational views
2026-01-23T01:06:42-06:00	84d5ab2	Initial RecursiveOps server control center
```
### Tracked source excerpt: `AGENTS.md`

```text
1|# RecursiveOps - Agent Rules
2|
3|## Primary Goal
4|Build a reliable Fedora server control center with optional LLM summaries. The app must work without LLM.
5|
6|## Non-Negotiables
7|1. Deterministic core: inventory, checks, parsing, diffing, applying patches.
8|2. LLM is advisory-only. It never executes actions automatically.
9|3. All LLM outputs must be strict JSON validated by Pydantic schemas.
10|4. Route changes must be patch-based with preview, snapshot, and rollback support.
11|5. Backend binds to 127.0.0.1 by default. Must require auth.
12|
13|## Coding Standards
14|- Python: FastAPI + SQLModel + Alembic, async where appropriate.
15|- All system interactions go through core/runner.py and core/whitelist.py.
16|- Prefer machine-readable command outputs (JSON) when available.
17|- Add tests for all parsers and patch logic.
18|
19|## Deliverables Checklist
20|- Working inventory endpoints
21|- Hostname checks with stored results
22|- Logs endpoints (systemd + podman)
23|- LLM analysis endpoints (Ollama)
24|- Cloudflared ingress parser + routes UI
25|- Add-route wizard (diff preview + apply + verify)
26|- Snapshot + diff view + rollback
27|
28|## Safety
29|- Redact secrets before any cloud LLM call.
30|- Never run arbitrary shell commands from user input.
31|
```
### Tracked source excerpt: `README.md`

```text
1|# RecursiveOps
2|
3|RecursiveOps is a local-first Fedora Server control center with optional LLM summaries. The core app works without any LLM enabled.
4|
5|## Features
6|- System inventory (systemd services, Podman containers, mounts, ports)
7|- Cloudflared ingress parser and route mapping
8|- HTTP health checks with history
9|- Logs (systemd journal + podman logs)
10|- Patch-based config changes with diff preview, snapshots, and rollback
11|- Optional LLM explanations (advisory-only)
12|
13|## Quick start
14|
15|### Backend
16|1. Copy and edit the config:
17|   ```bash
18|   cp deploy/config/recursiveops.example.yml /etc/recursiveops/config.yml
19|   ```
20|2. Ensure a JWT secret file exists:
[REDACTED]
22|   sudo mkdir -p /etc/recursiveops
23|   sudo sh -c 'head -c 48 /dev/urandom | base64 > /etc/recursiveops/jwt.secret'
24|   ```
25|3. Create a virtualenv and install:
26|   ```bash
27|   cd backend
28|   python3.11 -m venv .venv
29|   source .venv/bin/activate
30|   pip install -e .
31|   ```
32|4. Run migrations:
33|   ```bash
34|   alembic upgrade head
35|   ```
36|5. Start the API:
37|   ```bash
38|   uvicorn recursiveops.main:app --host 127.0.0.1 --port 8844
39|   ```
40|
41|### Frontend
42|```bash
43|cd frontend
44|npm install
45|npm run dev -- --host 127.0.0.1 --port 5173
46|```
47|
48|## Notes
49|- The backend binds to 127.0.0.1 by default.
50|- All system interactions are restricted by a command whitelist.
51|- LLM integration is optional and advisory-only.
52|
53|
```
### Tracked source excerpt: `backend/README.md`

```text
1|# RecursiveOps
2|
3|RecursiveOps is a local-first Fedora Server control center with optional LLM summaries. The core app works without any LLM enabled.
4|
5|## Features
6|- System inventory (systemd services, Podman containers, mounts, ports)
7|- Cloudflared ingress parser and route mapping
8|- HTTP health checks with history
9|- Logs (systemd journal + podman logs)
10|- Patch-based config changes with diff preview, snapshots, and rollback
11|- Optional LLM explanations (advisory-only)
12|
13|## Quick start
14|
15|### Backend
16|1. Copy and edit the config:
17|   ```bash
18|   cp deploy/config/recursiveops.example.yml /etc/recursiveops/config.yml
19|   ```
20|2. Ensure a JWT secret file exists:
[REDACTED]
22|   sudo mkdir -p /etc/recursiveops
23|   sudo sh -c 'head -c 48 /dev/urandom | base64 > /etc/recursiveops/jwt.secret'
24|   ```
25|3. Create a virtualenv and install:
26|   ```bash
27|   cd backend
28|   python3.11 -m venv .venv
29|   source .venv/bin/activate
30|   pip install -e .
31|   ```
32|4. Run migrations:
33|   ```bash
34|   alembic upgrade head
35|   ```
36|5. Start the API:
37|   ```bash
38|   uvicorn recursiveops.main:app --host 127.0.0.1 --port 8844
39|   ```
40|
41|### Frontend
42|```bash
43|cd frontend
44|npm install
45|npm run dev -- --host 127.0.0.1 --port 5173
46|```
47|
48|## Notes
49|- The backend binds to 127.0.0.1 by default.
50|- All system interactions are restricted by a command whitelist.
51|- LLM integration is optional and advisory-only.
52|
53|
```

## Workstream 9: /home/sikmindz/Coding/Gloss

- Branch: `main`
- Upstream: `(none)`
- Working tree status:
```text
(clean)
```
- Diff stat:
```text
(none)
```
- Five latest commits:
```text
2026-08-02T19:58:48-05:00	b81f3bc	feat(gloss): add memory inspector panel
2026-07-26T19:51:12-05:00	973f855	fix: startup WAL recovery + wal_autocheckpoint to prevent read-only DB errors
2026-07-26T19:37:10-05:00	3981974	fix: bump SQLite busy_timeout 5s→15s for concurrent ingestion writes
2026-07-26T19:14:18-05:00	79442b3	feat: unify provider stacks, add 6 Studio types, externalize prompts to TOML
2026-07-16T22:20:25-05:00	c4c0e2c	docs: rebuild README around verified product truth
```
### Tracked source excerpt: `AGENTS.md`

```text
1|# AGENTS.md — Gloss Closing Pass Rules
2|
3|Active run: `[LONG_TOKEN_REDACTED]`
4|
5|## Project purpose
6|
7|Gloss is a local-first notebook/RAG/chat application. Chat must work without retrieval when retrieval/source state is degraded. Retrieval, semantic-memory, TurboQuant, and release claims must be proof-bearing.
8|
9|## Source-of-truth ownership
10|
11|- Frontend chat lifecycle: `src/stores/chatStore.ts`, `src/components/chat/ChatPanel.tsx`, `src/App.tsx` event forwarding.
12|- Backend chat lifecycle: `src-tauri/src/commands/chat/mod.rs`.
13|- Provider validation and model registry: `src-tauri/src/providers/*`, settings commands, settings UI.
14|- Source/retrieval selection: `src/stores/sourceStore.ts`, retrieval backend modules.
15|- Package/release proof: `scripts/`, `validation/`, `docs/codex-runs/*`, package sidecars.
16|
17|## Forbidden behavior
18|
19|- Do not hide provider errors behind a spinner.
20|- Do not block no-retrieval chat because source list loading/partial/error.
21|- Do not emit `chat:done` before assistant persistence succeeds unless explicit partial/cancel artifact is emitted.
22|- Do not silently allow LAN or cloud endpoints; provider authority must be explicit.
23|- Do not claim semantic-memory/TurboQuant/dense indexing proof from dependencies alone.
24|- Do not add compatibility shims or shadow truth stores.
25|- Do not update release docs by hand if gate JSON says failed.
26|
27|## Required validation
28|
29|Run targeted tests first, then full gates:
30|
31|```bash
32|npm run build
33|npm test
34|cargo fmt --all -- --check
35|cargo test --manifest-path src-tauri/Cargo.toml --features semantic-memory-turbo-quant commands::chat::tests
36|cargo test --manifest-path src-tauri/Cargo.toml --features semantic-memory-turbo-quant providers::tests
37|python3 validation/validate_source_send_gate.py .
38|python3 validation/validate_frontend_event_routing.py .
39|python3 validation/validate_chat_terminal_contract.py .
40|python3 validation/validate_provider_lan_policy.py .
41|python3 validation/validate_release_receipt_consistency.py .
42|```
43|
44|## Completion rule
45|
46|Receipts or it did not happen. Final answer must include hostile-auditor handoff.
47|
```
### Tracked source excerpt: `src-tauri/vendor/forge-memory-bridge/AGENTS.md`

```text
1|# AGENTS.md — forge-memory-bridge
2|
3|Read the root control plane before changing this crate:
4|
5|1. `../CANONICAL_STACK_SPEC_V6.md`
6|2. `../[LONG_TOKEN_REDACTED].md`
7|3. `../PACK_README.md`
8|4. `../SOURCE_BASIS.md`
9|5. `../STATUS_DASHBOARD.md`
10|6. `../MASTER_ISSUE_CHANGE_MATRIX.md`
11|7. `../CONFORMANCE_GATES.md`
12|8. `../PHASED_EXECUTION_PLAN.md`
13|9. `../IMPLEMENTATION_PLAYBOOK.md`
14|10. `../RISKS_AND_FORBIDDEN_SHORTCUTS.md`
15|
16|## Scope
17|
18|This crate transforms Forge export payloads into typed memory import batches. It does not own
19|source truth, import authority, promotion policy, or authoritative store time.
20|
21|## What this crate must do in the finish-line pass
22|
23|1. preserve export provenance exactly,
24|2. validate the exported shape deterministically,
25|3. never invent missing version lineage,
26|4. carry only the fields memory needs for a correct import,
27|5. support the V2 transform path while keeping legacy helpers visibly fenced.
28|
29|The current snapshot still ships V1 surfaces in code. Do not teach those compatibility paths as the
30|long-term architecture once the V2 seam exists.
31|
32|## Do not do
33|
34|- do not stamp authoritative imported `recorded_at`
35|- do not synthesize `supersedes_claim_version_id`
36|- do not decide promotion, comparability, or truth absent from the export
37|- do not query live memory to invent semantics
38|- do not let legacy upgrade helpers look canonical
39|
40|## Primary files
41|
42|- `src/batch.rs`
43|- `src/transform.rs`
44|- `src/legacy.rs`
45|- `tests/forge_bridge_memory_proof.rs`
46|
```
### Tracked source excerpt: `src-tauri/vendor/stack-ids/AGENTS.md`

```text
1|# AGENTS.md — stack-ids
2|
3|Read the root control plane before changing this crate:
4|
5|1. `../CANONICAL_STACK_SPEC_V6.md`
6|2. `../[LONG_TOKEN_REDACTED].md`
7|3. `../PACK_README.md`
8|4. `../SOURCE_BASIS.md`
9|5. `../STATUS_DASHBOARD.md`
10|6. `../MASTER_ISSUE_CHANGE_MATRIX.md`
11|7. `../CONFORMANCE_GATES.md`
12|8. `../PHASED_EXECUTION_PLAN.md`
13|9. `../IMPLEMENTATION_PLAYBOOK.md`
14|10. `../RISKS_AND_FORBIDDEN_SHORTCUTS.md`
15|
16|## Scope
17|
18|`stack-ids` owns only shared opaque primitives and helpers:
19|
20|- IDs such as `AttemptId`, `TrialId`, `ClaimId`, `ClaimVersionId`, `EnvelopeId`, and
21|  `ImportBatchId`
22|- `Scope` and `ScopeKey`
23|- `TraceCtx` and bounded baggage helpers
24|- digests and format validation
25|
26|## Do
27|
28|- keep types opaque, parseable, and serialization-safe
29|- centralize cross-crate ID and trace primitives here instead of duplicating them elsewhere
30|- add invariant, round-trip, and trace-context tests when extending public surface
31|- keep new additions small and obviously reusable across crates
32|
33|## Do not
34|
35|- add business logic, storage rows, or semantic result types
36|- add retry policy or promotion policy here
37|- duplicate canonical primitives in downstream crates
38|- let convenience wrappers turn this crate into a generic contracts layer
39|
40|## Finish-line focus
41|
42|For the current closure pass, this crate mainly participates in:
43|
44|- `TRACE-101` by preserving canonical ID and `TraceCtx` ownership
45|- `TRACE-101` through stable cross-crate retry/replay identifiers
46|- `TRACE-102` through stable queue-hop lineage identifiers
47|- `CONF-001` through root-visible proof that execution crates share one shipped release gate
48|
```

## Workstream 10: /home/sikmindz/Coding/stack-showcase

- Branch: `main`
- Upstream: `(none)`
- Working tree status:
```text
(clean)
```
- Diff stat:
```text
(none)
```
- Five latest commits:
```text
2026-08-02T20:42:20-05:00	516759d	feat: complete portfolio stack showcase
2026-07-11T18:54:31-05:00	66cf289	Initial commit from Create Next App
```
### Tracked source excerpt: `README.md`

```text
1|# Josh Stevenson · Full Portfolio CMS
2|
3|A production-grade multi-page portfolio with a **real admin backend** so you can add/edit/update content without redeploying static files.
4|
5|## Public site
6|
7|| Route | Purpose |
8||-------|---------|
9|| `/` | Home — hero, featured work, proof, stack, timeline, writing |
10|| `/work` | Filterable project index |
11|| `/work/[slug]` | Full case studies (markdown body) |
12|| `/writing` | Essays |
13|| `/writing/[slug]` | Post detail |
14|| `/lab` | Lab notes + buildlog |
15|| `/gallery` | Visual gallery |
16|| `/about` | Bio + timeline (CMS) |
17|| `/now` | Current focus (CMS) |
18|| `/contact` | Contact form → admin inbox |
19|| `/search` | Full-text search |
20|| `/feed.xml` | RSS |
21|| `/sitemap.xml` | Sitemap |
22|
23|## Admin CMS (`/admin`)
24|
25|- Login (session cookie, bcrypt password)
26|- Dashboard
27|- **CRUD**: Projects, Posts, Proof points, Timeline, Gallery, Stack items
28|- **Settings**: site identity, about, now copy
29|- **Messages**: contact form inbox
30|
31|### Default credentials (change after first login)
32|
33|```
34|username: josh
35|password: [REDACTED]
36|```
37|
38|Set in `.env`:
39|
40|```env
41|DATABASE_URL="file:./dev.db"
42|SESSION_SECRET=[REDACTED]
43|ADMIN_USERNAME="josh"
44|ADMIN_PASSWORD=[REDACTED]
45|```
46|
47|After changing `ADMIN_PASSWORD`, re-seed the admin user:
[REDACTED]
49|```bash
50|npm run db:seed
51|```
52|
53|## Develop
54|
55|```bash
56|cd ~/Coding/stack-showcase
57|cp .env.example .env   # if needed
58|npm install
59|npx prisma db push
60|npm run db:seed
61|npm run dev
62|```
63|
64|- Public: http://localhost:3000<br>
65|- Admin: http://localhost:3000/admin<br>
66|
67|## Stack
68|
69|- Next.js App Router · React 19 · Tailwind · Framer Motion<br>
70|- Prisma + SQLite<br>
71|- iron-session auth · bcrypt · Zod · marked<br>
72|
73|## Notes
74|
75|- Content is **database-backed**, not hard-coded MDX only.
76|- Public pages use `force-dynamic` so CMS edits show up after revalidation.
77|- This is a **new site** (not recursiveintell-web), but matches the ambition of a full portfolio + workbench.
78|
```
### Tracked source excerpt: `PLAN.md`

```text
1|# Plan: Private Admin UI Polish
2|
3|**Created**: 2026-07-11<br>
4|**Status**: Complete<br>
5|**Scope**: `src/components/admin/**` + `src/app/admin/**` (UI only)
6|
7|---
8|
9|## Goal
10|Polish the private admin to Linear/Vercel dashboard quality: dense chrome, secure login, consistent tables/forms/buttons, refined dashboard cards. Keep server actions, field names, and auth intact.
11|
12|---
13|
14|## Status Checklist
15|- [x] Shared design system in Field.tsx (dense tokens)
16|- [x] AdminShell dense chrome (200px sidebar)
17|- [x] Login secure presentation (no default password)
18|- [x] Dashboard cards refined
19|- [x] Tables/forms/buttons consistent across pages
20|- [x] Field names unchanged; actions.ts untouched
21|- [x] Typecheck + eslint clean
22|- [x] **DONE**
23|
```

## Workstream 11: /home/sikmindz/Coding/recursiveintell-web

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
(clean)
```
- Diff stat:
```text
(none)
```
- Five latest commits:
```text
2026-08-03T22:45:04-05:00	f433f5d	feat(website): Hermes Agent — primary install path, homepage hero, stack layer
2026-08-03T22:39:03-05:00	064fc26	feat(installer): all 5 MCP servers — claim-ledger, cea-graph, pilot-bridge
2026-08-03T22:36:02-05:00	3f96253	feat(installer): agent-graph systemd unit — auto-starts on boot
2026-08-03T22:33:42-05:00	14988d1	feat(installer): semantic-memory systemd unit — auto-starts on boot
2026-08-03T21:58:55-05:00	a492d9e	fix(installer): auto-register MCP servers, disable built-in memory
```
### Tracked source excerpt: `README.md`

```text
1|# vinext-starter
2|
3|A clean full-stack starter running on
4|[vinext](https://github.com/cloudflare/vinext), with optional Cloudflare D1 and
5|Drizzle support.
6|
7|## Prerequisites
8|
9|- Node.js `>=22.13.0`
10|- Linux with `flock`, `curl`, and GNU `timeout`
11|
12|## Sites Lifecycle
13|
14|The Sites lifecycle CLI runs the locked dependency install before returning this checkout. Edit the source under `app/`, then checkpoint when a coherent milestone is ready to inspect or share. The remote Sites builder runs `npm run build` against the pushed commit. Do not repeat install or build as a normal pre-checkpoint step.
15|
16|This starter does not use `wrangler.jsonc`.
17|
18|`install:ci` is intentionally a single, non-retrying `npm ci`. It refuses a concurrent install for the same project, consumes a matching image-seeded npm cache with `--prefer-offline` while retaining registry fallback for a missing cache object, otherwise downloads and verifies the complete vinext tarball recorded in `package-lock.json`, limits npm to one socket, and terminates a stalled install. `build` applies a short timeout and then validates the Sites artifact. These helpers target Linux and use GNU `timeout`; they are not native macOS scripts.
19|
20|Scripts that need writable project-scoped home, npm, XDG, and temporary paths use `scripts/sites-env.sh`. The `dev` and `start` scripts honor the caller's runtime environment and keep Wrangler logs inside the checkout. The generated `.sites-runtime/` directory is disposable and ignored by Git.
21|
22|## Included Shape
23|
24|- edit site code under `app/`
25|- `app/chatgpt-auth.ts` provides optional dispatch-owned ChatGPT sign-in helpers
26|- `.openai/hosting.json` declares optional Sites D1 and R2 bindings
27|- `vite.config.ts` simulates declared bindings for local development
28|- `db/index.ts` reads the D1 binding from the Cloudflare Worker environment
29|- `db/schema.ts` starts intentionally empty
30|- `examples/d1/` contains an optional D1 example surface
31|- `drizzle.config.ts` supports local migration generation when needed
32|
33|## Workspace Auth Headers
34|
35|OpenAI workspace sites can read the current user's email from
36|`oai-authenticated-user-email`.
37|
38|SIWC-authenticated workspace sites may also receive
39|`oai-authenticated-user-full-name` when the user's SIWC profile has a non-empty
40|`name` claim. The full-name value is percent-encoded UTF-8 and is accompanied by
41|`oai-authenticated-user-full-name-encoding: percent-encoded-utf-8`.
42|
43|Treat the full name as optional and fall back to email when it is absent:
44|
45|```tsx
46|import { headers } from "next/headers";
47|
48|export default async function Home() {
49|  const requestHeaders = await headers();
50|  const email = requestHeaders.get("oai-authenticated-user-email");
51|  const encodedFullName = requestHeaders.get("oai-authenticated-user-full-name");
52|  const fullName =
53|    encodedFullName &&
54|    requestHeaders.get("oai-authenticated-user-full-name-encoding") ===
55|      "percent-encoded-utf-8"
56|      ? decodeURIComponent(encodedFullName)
57|      : null;
58|
59|  const displayName = fullName ?? email;
60|  // ...
61|}
62|```
63|
64|## Optional Dispatch-Owned ChatGPT Sign-In
65|
66|Import the ready-to-use helpers from `app/chatgpt-auth.ts` when the site needs
67|optional or required ChatGPT sign-in:
68|
69|- Use `getChatGPTUser()` for optional signed-in UI.
70|- Use `requireChatGPTUser(returnTo)` for server-rendered pages that should send
71|  anonymous visitors through Sign in with ChatGPT.
72|- Use `chatGPTSignInPath(returnTo)` and `chatGPTSignOutPath(returnTo)` for
73|  browser links or actions.
74|- Pass a same-origin relative `returnTo` path for the destination after sign-in
75|  or sign-out. The helper validates and safely encodes it.
76|- Mark protected pages with `export const dynamic = "force-dynamic"` because
77|  they depend on per-request identity headers.
78|
79|Dispatch owns `/signin-with-chatgpt`, `/signout-with-chatgpt`, `/callback`, the
80|OAuth cookies, and identity header injection. Do not implement app routes for
81|those reserved paths. Routes that do not import and call the helper remain
82|anonymous-compatible.
83|
84|SIWC establishes identity only; it does not prove workspace membership. Use the
85|Sites hosting platform's access policy controls for workspace-wide restrictions,
86|or enforce explicit server-side membership or allowlist checks.
87|
88|Use SIWC for account pages, user-specific dashboards, saved records, and write
89|actions tied to the current ChatGPT user. Leave public content anonymous.
90|
91|## Diagnostic Commands
92|
93|- `npm run install:ci`: perform the one bounded lockfile install
94|- `npm run dev`: start the Vite/Vinext development server
95|- `npm run build`: build and validate the deployable Sites artifact
96|- `npm run start`: start the built Vinext application
97|- `npm test`: build, validate, and verify the rendered development-preview metadata
98|- `npm run validate:artifact`: recheck an existing artifact's manifest and ESM `default.fetch` export
99|- `npm run db:generate`: generate Drizzle migrations after schema changes
100|
101|Use build and validation commands for targeted diagnosis after a remote failure, not as part of the normal checkpoint path.
102|
103|The timeout defaults can be overridden for a controlled canary with `SITES_INSTALL_TIMEOUT`, `SITES_INSTALL_KILL_AFTER`, `SITES_BUILD_TIMEOUT`, and `SITES_BUILD_KILL_AFTER`. A timeout fails the command; the helpers never retry an unchanged install or build.
104|
105|## Learn More
106|
107|- [vinext Documentation](https://github.com/cloudflare/vinext)
108|- [Drizzle D1 Guide](https://orm.drizzle.team/docs/get-started/d1-new)
109|
```

## Workstream 12: /home/sikmindz/Coding/Projects/StableMaster

- Branch: `main`
- Upstream: `origin/main`
- Working tree status:
```text
?? Archive.tar.gz
?? s/
```
- Diff stat:
```text
(none)
```
- Five latest commits:
```text
2026-08-02T20:00:21-05:00	48d588f	feat(stablemaster): add image inpainting editor workflow
2026-02-13T00:06:54-06:00	5f1b1d5	Initial commit — StableMaster desktop app
```
### Tracked source excerpt: `README.md`

```text
1|# StableMaster
2|
3|A desktop application for AI image generation powered by [ComfyUI](https://github.com/comfyanonymous/ComfyUI), built with Tauri v2, React, and Rust.
4|
5|## Features
6|
7|- **Dual Generation Modes** — Standard mode for quick generation, Expert mode with full control over checkpoints, samplers, schedulers, CFG, and seeds
8|- **Image Gallery** — Browse, search, favorite, and rate generated images with full metadata retention
9|- **Generation Queue** — Queue multiple generation jobs with pause/resume/cancel support
10|- **AI Tagging & Captioning** — Batch or single-image tagging and captioning via Ollama vision models
11|- **Prompt Enhancement** — AI-powered prompt refinement using local LLMs through Ollama
12|- **Persistent Configuration** — Per-user settings for ComfyUI endpoint, Ollama endpoint, storage paths, and generation defaults
13|
14|## Prerequisites
15|
16|- [Node.js](https://nodejs.org/) (v18+)
17|- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
18|- [ComfyUI](https://github.com/comfyanonymous/ComfyUI) running locally or on your network
19|- [Ollama](https://ollama.com/) (optional, for AI tagging/captioning/prompt enhancement)
20|
21|## Getting Started
22|
23|```bash
24|# Install frontend dependencies
25|npm install
26|
27|# Run in development mode
28|npm run tauri dev
29|
30|# Build for production
31|npm run tauri build
32|```
33|
34|## Tech Stack
35|
36|| Layer    | Technology                          |
37|| -------- | ----------------------------------- |
38|| Shell    | Tauri v2                            |
39|| Frontend | React 18, TypeScript, Tailwind CSS  |
40|| Backend  | Rust, SQLite, reqwest               |
41|| AI       | ComfyUI (generation), Ollama (LLM)  |
42|
43|## Project Structure
44|
45|```
46|src/                        # React frontend
47|├── components/
48|│   ├── common/             # Button, Input, Modal, Select, StarRating, TagChip
49|│   ├── gallery/            # GalleryGrid, ImageDetail
50|│   ├── generation/         # StandardMode, ExpertMode
51|│   ├── layout/             # Header, Sidebar, ModeToggle
52|│   ├── queue/              # QueuePanel
53|│   └── settings/           # SettingsPanel
54|├── types/                  # TypeScript interfaces
55|└── utils/                  # Constants, helpers
56|
57|src-tauri/src/              # Rust backend
58|├── ai/                     # Prompt enhancement, batch tagging/captioning
59|├── commands/               # Tauri command handlers
60|├── config/                 # App configuration
61|├── database/               # SQLite schema, models
62|├── gallery/                # Image storage
63|├── generation/             # ComfyUI integration, workflows
64|└── queue/                  # Job queue system
65|```
66|
67|## License
68|
69|MIT
70|
```

## Evidence rules for council agents

1. Current captured files and Git state outrank remembered summaries.
2. A dirty tree proves only uncommitted state, not importance or readiness.
3. Commit subjects are source metadata, not proof that tests passed or deployments succeeded.
4. Missing files, omitted repositories, and absent tests are unknown—not failures unless the contract requires them.
5. Every recommendation must cite workstream number plus exact captured evidence.
6. Separate observed, inferred, proposed, blocked, and degraded states.
7. Optimize for the user’s 30–60 day business/job goal, compounding technical leverage, credibility, and time to proof.
8. Preserve dissent and propose a falsifiable first experiment.
