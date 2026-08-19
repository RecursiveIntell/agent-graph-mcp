## System context

Compact the supplied unfinished-work evidence without inventing facts. Preserve workstream paths, Git status, recent commit metadata, decisive source excerpts, omissions, evidence-state labels, and the user goal.

## User context

1|# Unfinished Work Evidence Pack
2|
3|Generated from live local repositories for the 12-agent direction council.
4|
5|## User decision context
6|
7|- Near-term goal from canonical USER.md: start a business and/or get a good job in the next 30–60 days.
8|- Working preference: evidence-led, local-first, reproducible, bounded, rollback-aware systems.
9|- Council task: choose the highest-leverage next direction from unfinished work; do not equate dirty state with value or completion.
10|
11|## Portfolio inventory
12|
13|- Live scan: 41 Git repositories under `/home/sikmindz/Coding`; 13 reported dirty at capture time.
14|- This pack intentionally samples 12 active/relevant workstreams. Omitted repositories are missing evidence, not negative evidence.
15|
16|## Workstream 1: /home/sikmindz/Coding/agent-graph-mcp-release
17|
18|- Branch: `main`
19|- Upstream: `origin/main`
20|- Working tree status:
21|```text
22|M Cargo.toml
23| M src/main.rs
24| M src/proxy.rs
25| M src/spec.rs
26| M tests/mcp_integration.rs
27|?? .hermes/plans/agent-collaboration-protocol-20260808.md
28|?? .hermes/plans/luna-next-path-load-envelope-20260808.md
29|?? .hermes/receipts/
30|?? .hermes/runs/
31|?? vendor/
32|```
33|- Diff stat:
34|```text
35|Cargo.toml               |  1 +
36| src/main.rs              | 18 +++++++++++-------
37| src/proxy.rs             |  3 +++
38| src/spec.rs              |  4 ++--
39| tests/mcp_integration.rs |  9 ++++++---
40| 5 files changed, 23 insertions(+), 12 deletions(-)
41|```
42|- Five latest commits:
43|```text
44|2026-08-08T14:41:32-05:00	ba9fba1	fix: bound and isolate concurrent Codex workers
45|2026-08-06T21:03:44-05:00	aaaa52f	feat: token accounting — record provider prompt/completion/total tokens in LLM invocation receipts (local llm-pipeline via [patch.crates-io])
46|2026-08-06T20:41:44-05:00	9761331	feat: forward-compatible token-usage hook in LLM invocation records (TODO gate for llm-pipeline upgrade; spec: [REDACTED]
47|2026-08-06T17:39:55-05:00	13258e3	feat: operator DecideApproval action, operator.sock served by daemon, approval-aware resume (APPROVAL_REJECTED gate, approved-consumed resume)
48|2026-08-06T17:15:55-05:00	ca2821f	feat: loop + subgraph nodes, swarm strategy joins, effect gate, docs; include pending workspace changes (bridge_config, lib, server, tool_exec, provekv_executor)
49|```
50|### Tracked source excerpt: `AGENTS.md`
51|
52|```text
53|1|# AGENTS.md — agent-graph-mcp
54|2|
55|3|Instructions for AI coding agents (Claude Code, Codex, Cursor, Copilot, etc.) working on this repository.
56|4|
57|5|## Project identity
58|6|
59|7|`agent-graph-mcp` is an MCP server that exposes the `ri-agent-graph` runtime engine as 25 typed tools. It compiles declarative JSON workflow specs, executes LLM graphs with parallel fan-out, checkpoint/resume, human-in-the-loop approvals, source witnessing, and HMAC-authenticated receipts.
60|8|
61|9|**Stack:** Rust (edition 2021, MSRV 1.75), Tokio async, rmcp, SQLite.
62|10|
63|11|## Build, test, lint
64|12|
65|13|```bash
66|14|cargo build                    # debug build
67|15|cargo build --release          # release binary
68|16|cargo test --lib               # 58 lib tests (1 known fixture-path failure)
69|17|cargo test --test daemon_recovery --test mcp_integration  # integration tests
70|18|cargo fmt --check              # formatting
71|19|cargo clippy --all-targets -- -D warnings  # lint (must pass clean)
72|20|cargo deny check               # dependency auditing
73|21|```
74|22|
75|23|The binary installs to `~/.cargo/bin/agent-graph-mcp`. There is also a daemon binary (`agent-graph-mcpd`) for persistent multi-client mode.
76|24|
77|25|## Project structure
78|26|
79|27|```
80|28|src/
81|29|├── main.rs              # CLI entry point (direct mode, daemon client)
82|30|├── cli.rs               # Argument parsing
83|31|├── server.rs            # MCP tool router (25 tools)
84|32|├── tools.rs             # Tool parameter types + JSON schemas
85|33|├── daemon.rs             # Daemon process (agent-graph-mcpd)
86|34|├── run_manager.rs       # Graph execution lifecycle
87|35|├── store.rs             # SQLite persistence
88|36|├── migrations.rs        # Schema migrations
89|37|├── compiler.rs          # JSON spec → executable graph compilation
90|38|├── spec.rs              # Graph spec types
91|39|├── nodes.rs             # Node type definitions (LLM, join, passthrough, etc.)
92|40|├── lifecycle.rs         # Create, validate, delete graph operations
93|41|├── templates.rs         # Built-in templates (council_deliberation, etc.)
94|42|├── evidence.rs          # Source witnessing, HMAC receipts
95|43|├── policy.rs            # Graph execution policy checks
96|44|├── promotion.rs         # Template promotion to built-in status
97|45|├── transport.rs         # Daemon transport layer (Unix socket)
98|46|├── proxy.rs             # MCP proxy between client and daemon
99|47|├── owner_lock.rs        # Single-owner daemon lock
100|48|├── operator.rs          # Operator IPC
101|49|├── operator_auth.rs     # Operator authentication
102|50|├── operator_ipc.rs      # Operator IPC protocol
103|51|├── auth.rs              # Client authentication
104|52|├── fs_security.rs       # Filesystem security controls
105|53|├── codex_app_server.rs  # Codex app server integration
106|54|└── lib.rs               # Module declarations + re-exports
107|55|tests/
108|56|├── daemon_recovery.rs   # Daemon crash recovery tests
109|57|├── mcp_integration.rs   # MCP protocol integration tests
110|58|├── lifecycle.rs         # Graph lifecycle tests
111|59|├── operator_authority.rs # Operator permission tests
112|60|├── migrations.rs        # Schema migration tests
113|61|├── template_promotion.rs # Template promotion tests
114|62|└── ...                  # Additional integration tests
115|63|```
116|64|
117|65|## Coding conventions
118|66|
119|67|- **No `unwrap()` or `expect()` in library code.** Use `anyhow::Result` or `thiserror` for error handling.
120|68|- **All public items need `///` doc comments.**
121|69|- **Tool handlers return `Result<Json<Output>, ErrorData>`** — the `Output` struct carries the JSON schema required by MCP spec.
122|70|- **Tests go in `#[cfg(test)] mod tests` at file bottom** (unit) or in `tests/` (integration).
123|71|- **Do not add new dependencies without a clear reason.** Prefer extending the existing stack.
124|72|- **Schema migrations** go in `src/migrations.rs` with versioned migration functions.
125|73|- **Graph spec validation** happens at `graph_create` time — invalid specs are rejected before execution.
126|74|
127|75|## What NOT to do
128|76|
129|77|- **Do not fabricate tool capabilities.** Tools must match actual rmcp `#[tool]` handlers.
130|78|- **Do not add speculative features.** New tools, node types, or templates need a concrete consumer.
131|79|- **Do not break the daemon protocol.** The Unix socket framed transport between proxy and daemon is a stability boundary.
132|80|- **Do not modify SQLite schema without a versioned migration.** Schema changes must be backward-compatible or gated behind a migration.
133|81|- **Do not expose internal errors to MCP clients.** Tool errors should be descriptive but must not leak stack traces or internal state.
134|82|- **Do not merge PRs with failing tests.** The known `evidence::tests::[LONG_TOKEN_REDACTED]` failure is tracked but should not be joined by new failures.
135|83|
136|84|## Security boundaries
137|85|
138|86|- **HMAC receipts** in `src/evidence.rs` use SHA-256 HMAC for content authentication. Do not weaken or bypass.
139|87|- **Daemon authentication** via Unix socket peer credentials (`src/auth.rs`). Do not add unauthenticated TCP listeners.
140|88|- **Operator IPC** requires explicit authorization (`src/operator_auth.rs`). Never skip operator permission checks.
141|89|- **Source witnessing** captures caller-supplied content with HMAC verification. Never weaken the authentication tag check.
142|90|- **Dependencies** are audited via `cargo deny`. New dependencies must pass advisory, ban, license, and source checks.
143|91|
144|92|## Publication
145|93|
146|94|- **crates.io:** `cargo publish -p agent-graph-mcp`
147|95|- **npm:** `npm publish` (package includes prebuilt binaries)
148|96|- Version bumps follow the existing `Cargo.toml` version. Update both crates.io and npm on release.
149|97|
150|98|## License
151|99|
152|100|MIT. All contributions are under the same license.
153|101|
154|```
155|### Tracked source excerpt: `README.md`
156|
157|```text
158|1|# agent-graph-mcp
159|2|
160|3|**Run 9 agents at once.** MCP server for graph-orchestrated LLM workflows — dispatch up to 16 LLM nodes in parallel fan-out with typed joins, checkpoint/resume, human-in-the-loop approvals, and HMAC-authenticated execution receipts. 27 typed tools.
161|4|
162|5|[![Crates.io](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
163|6|[![docs.rs](https://img.shields.io/docsrs/agent-graph-mcp)](https://docs.rs/agent-graph-mcp)
164|7|[![MCP Badge](https://lobehub.com/badge/mcp-full/recursiveintell-agent-graph-mcp?theme=light)](https://lobehub.com/mcp/recursiveintell-agent-graph-mcp)
165|8|[![npm](https://img.shields.io/npm/v/@recursiveintell/agent-graph-mcp)](https://www.npmjs.com/package/@recursiveintell/agent-graph-mcp)
166|9|[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
167|10|
168|11|![Architecture diagram showing MCP client connecting via stdin/stdout to the agent-graph-mcp proxy, which communicates over Unix socket to the agent-graph-mcpd daemon backed by SQLite](assets/architecture.svg)
169|12|
170|13|> **Expose the `ri-agent-graph` runtime engine over MCP.** Compile declarative JSON workflow specs, execute synchronously or asynchronously, checkpoint/resume, request human approval, capture source witnesses, and get cryptographic receipts — all through 27 typed MCP tools. Normal execution is synchronous. Durable approval is supported only as a SQLite-backed decision.
171|14|
172|15|## Who is this for?
173|16|
174|17|**AI agent operators** who need multi-node LLM orchestration (parallel research sweeps, council deliberation, plan→critique→refine pipelines) through their existing MCP client (Hermes Agent, Claude Desktop, Cursor). **Not for** simple single-call LLM usage — use a direct provider integration for that.
175|18|
176|19|## Quick start
177|20|
178|21|### Prerequisites
179|22|
180|23|- An LLM endpoint (local Ollama, or any OpenAI-compatible API)
181|24|- Node.js ≥ 18 (for npx) or Rust ≥ 1.75 (for cargo install)
182|25|- A model available at your endpoint. Examples below use `llama3.2:3b` (pull with `ollama pull llama3.2:3b`). Any model works — just replace `--model`.
183|26|
184|27|### npx (recommended)
185|28|
186|29|```bash
187|30|npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
188|31|```
189|32|
190|33|> **Note:** `--direct` is deprecated. Prefer daemon mode (below) for persistence and multi-client support. Direct mode still works but will be removed in a future release.
191|34|
192|35|**Expected output:** MCP initialization handshake. Run `tools/list` to verify you see 27 tools.
193|36|
194|37|### Cargo install
195|38|
196|39|```bash
197|40|cargo install agent-graph-mcp --locked
198|41|agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
199|42|```
200|43|
201|44|### Daemon mode (multi-client, persistent state)
202|45|
203|46|```bash
204|47|# Start the daemon (all defaults shown explicitly)
205|48|agent-graph-mcpd \
206|49|  --data-dir ~/.local/share/agent-graph \
207|50|  --socket /tmp/agent-graph/mcp.sock \
208|51|  --base-url http://127.0.0.1:11434 \
209|52|  --model llama3.2:3b \
210|53|  --max-graphs 256 &
211|54|
212|55|# Connect proxy
213|56|agent-graph-mcp --socket /tmp/agent-graph/mcp.sock
214|57|```
215|58|
216|59|**Defaults** (applied when flags are omitted):
217|60|
218|61|| Flag | Default | Notes |
219|62||------|---------|-------|
220|63|| `--data-dir` | `/tmp/agent-graph` | Ephemeral across reboots. Set explicitly for durable storage |
221|64|| `--socket` | `/tmp/agent-graph/mcp.sock` | Must match between daemon and proxy |
222|65|| `--base-url` | `http://127.0.0.1:11434` | Ollama default. Change for any provider |
223|66|| `--model` | `glm-5.2:cloud` | **Always override this.** The default exists only for backward compatibility |
224|67|| `--max-graphs` | 64 | Range 1–1024. Raise for large graph libraries |
225|68|
226|69|`--max-graphs` is a per-daemon registration capacity. `graph_status` reports both the effective limit and `capacity_state`; an `over_limit_legacy` state preserves existing durable graphs but rejects new registrations until the configured limit is raised or registrations are retired.
227|70|
228|71|### Direct vs daemon
229|72|
230|73|| Mode | Use when |
231|74||------|----------|
232|75|| `--direct` | Single MCP client, no persistence needed, simplest setup |
233|76|| `--socket` (daemon) | Multiple clients, durable graph storage, long-running workflows, HITL approvals |
234|77|
235|78|## Provider and model configuration
236|79|
237|80|Every graph run sends LLM calls to an OpenAI-compatible endpoint. You control **where** (`--base-url`) and **which model** (`--model`). The API key flows through the `OPENAI_API_KEY` environment variable.
238|81|
239|82|### Codex App Server mode
240|83|
241|84|When `--base-url codex-app-server://` is selected, the Rust daemon starts one long-lived local Codex App Server worker over a loopback WebSocket and reuses it across graph nodes. Turns are serialized through a bounded Rust-owned session; a failed or timed-out worker is terminated and recreated cleanly. This avoids the fixed memory multiplier from spawning one heavy App Server process per node.
242|85|
243|86|The integration is bounded before launch and during streaming:
244|87|
245|88|- one persistent Codex App Server worker when the configured process limit is
246|89|  one; higher limits use bounded one-shot workers for true provider concurrency;
247|90|- the Luna service launcher enumerates enabled Codex MCP servers and disables
248|91|  those servers plus plugin/app injection for prompt-only graph turns;
249|92|- each completed graph thread is deleted before the worker is reused, preventing
250|93|  the long-lived connection from retaining auto-subscribed thread state;
251|94|- prompt input capped at 256 KiB;
252|95|- model reasoning effort pinned to `low` for bounded graph lanes;
253|96|- each JSON-RPC line capped at 4 MiB and each WebSocket message capped at 1 MiB;
254|97|- streamed assistant output capped at 256 KiB;
255|98|- stderr retained as an 8 KiB tail and redacted before MCP errors;
256|99|- stdio-only Codex-compatible test executables may use the legacy one-shot compatibility path.
257|100|
258|101|`graph max_parallelism` controls graph scheduling, w
259|```
260|### Tracked source excerpt: `.hermes/plans/2026-08-01-provenance-tool-runtime.md`
261|
262|```text
263|1|# Provenance-Bound Hermes Tool Nodes Implementation Plan
264|2|
265|3|> **For Hermes:** Implement task-by-task under RED/GREEN discipline. Do not commit or deploy without independent review.
266|4|
267|5|**Goal:** Make Agent Graph `tool` nodes execute a real Hermes worker with the same dynamic tool catalog as a normal Hermes session, while provenance leases, durable receipts, cycle detection, budgets, and approval policy bound recursion and side effects.
268|6|
269|7|**Architecture:** Agent Graph remains the deterministic orchestrator. A `tool` node launches `hermes chat -q -Q`—never `hermes --oneshot`—with a daemon-generated lineage lease and an isolated receipt directory. A Hermes plugin intercepts `pre_tool_call` and `post_tool_call`: it verifies the lease, atomically reserves lineage budget, classifies recursive/effectful calls, fails closed when policy or receipt persistence is unavailable, and writes an append-only hash chain. The Rust node accepts output only when a terminal worker receipt verifies against the lease and receipt chain.
270|8|
271|9|**Tech Stack:** Rust 2021, Tokio process execution, serde/serde_json, SHA-256/HMAC, SQLite terminal projection, Python 3 Hermes plugin hooks, pytest, cargo test.
272|10|
273|11|---
274|12|
275|13|## Current evidence
276|14|
277|15|- Isolated worktree: `/home/sikmindz/.cache/agent-graph-tool-runtime-20260801`
278|16|- Branch: `feat/provenance-tool-nodes-20260801`
279|17|- Baseline HEAD: `4ead448308d7f09dfe9116ac8becf05d0af2334f`
280|18|- Candidate diff was replicated from `/home/sikmindz/.cache/agent-graph-capacity-20260731`; source and isolated tracked-diff SHA-256 both equal `[LONG_TOKEN_REDACTED]` at isolation time.
281|19|- `src/spec.rs` already declares `NodeType::Tool` but `GraphSpec::executable_node_type` rejects it.
282|20|- `src/compiler.rs` rejects `NodeType::Tool` at compilation.
283|21|- Hermes canonical tool catalog is `model_tools.get_tool_definitions`; canonical dispatch is the full `AIAgent` loop. `todo`, `memory`, `session_search`, and `delegate_task` are agent-loop tools and cannot be reached by plain `registry.dispatch`.
284|22|- Hermes `--oneshot` says approvals are auto-bypassed; it is forbidden for this runtime. `hermes chat -q -Q` is the worker surface.
285|23|- Hermes plugin hooks `pre_tool_call` and `post_tool_call` cover registry-dispatched and agent-loop-owned tools.
286|24|
287|25|## Hard invariants
288|26|
289|27|1. **Full catalog, bounded authority:** Worker agents can see their normal configured tool catalog. Visibility never implies unconditional execution authority.
290|28|2. **Hermes owns tool semantics:** Agent Graph never reimplements or directly dispatches Hermes tools.
291|29|3. **No `--oneshot`:** The worker command must reject `--oneshot` and `-z` configurations.
292|30|4. **Fail closed:** Missing, malformed, expired, unverifiable, or exhausted leases block all tool calls.
293|31|5. **Receipt before effect:** A pre-call reservation is durably appended before the tool executes. If reservation persistence fails, execution is blocked.
294|32|6. **Terminal closure:** Rust accepts worker success only with a valid terminal receipt matching graph/run/node/attempt/lease/output digests.
295|33|7. **Lineage budget:** All descendants share a lineage ID and atomically consume common budgets.
296|34|8. **Recursive calls are explicit:** `delegate_task`, `cronjob`, `execute_code`, Agent Graph execution/start/resume tools, and tool-search bridge invocation of those tools consume recursive budget and are cycle-checked.
297|35|9. **No silent widening:** Child processes inherit the same or narrower lease. A model cannot raise tool, effect, depth, call, wall-clock, or child-count limits.
298|36|10. **Human app

... [OUTPUT TRUNCATED - 51,224 chars omitted out of 101,150 total] ...

iler` RFC 8785 JCS everywhere. Malformed input is a typed
1256|21|   rejection, not a panic.
1257|22|6. **Provider-free in M0.** No Ollama, no OpenAI-compatible call, no
1258|23|   network. The product survives its own restart and verifies offline.
1259|24|   **Phase 2 deliberately lifts this** for the `llm` tool only: provider
1260|25|   calls are receipt-bearing and typed (see `recursive-agent-provider`),
1261|26|   and the receipt chain still verifies offline. All other tools remain
1262|27|   provider-free.
1263|28|7. **Recorded replay only.** Do not promise "deterministic replay" of any
1264|29|   LLM. Recorded replay is the only replay contract M0 offers. A
1265|30|   provider-backed `llm` step records its response as a content-addressed
1266|31|   artifact; replay re-emits that recorded output and never re-calls the
1267|32|   provider.
1268|33|8. **Bounded safety.** No `unsafe`, no `unwrap`/`expect` in lib code
1269|34|   (`cargo clippy -D warnings`). Any panic is a bug.
1270|35|9. **Source hierarchy.** This workspace depends on Libraries by **path**.
1271|36|   No edits under `~/Coding/Libraries/`. AiDENs P32 is still
1272|37|   `feature_expansion_allowed: false`.
1273|38|
1274|39|## Source-of-truth ownership
1275|40|
1276|41|| Concern | Owner | Adapter here |
1277|42||---|---|---|
1278|43|| Canonical JSON / boundary | `boundary-compiler` 0.1.0 | direct dep |
1279|44|| Material IDs / digests | `stack-ids` 0.1.1 | direct dep |
1280|45|| Bitemporal semantics | `bitemporal-runtime` 0.1.0 | direct dep (in-memory view in M0) |
1281|46|| Claims / evidence | `claim-ledger` 0.1.0 | direct dep |
1282|47|| Run orchestration | this workspace | new |
1283|48|| Receipt chain | this workspace (`ledger` crate) | new |
1284|49|| Tool plane | this workspace (`tools` crate) | new |
1285|50|| Provider / LLM | `recursive-agent-provider` (new) | Ollama + OpenAI-compatible adapters |
1286|51|| MCP / channel | none | out of scope M0/Phase 2 |
1287|52|
1288|53|## Receipt contract (M0)
1289|54|
1290|55|- `receipts.ndjson` under `<run-dir>/`.
1291|56|- One receipt per line. Each line is JCS canonical JSON.
1292|57|- Chain digest: `blake3(prev_chain_digest || jcs(receipt))`. Initial
1293|58|  `prev_chain_digest = blake3(b"recursive-agent-m0-genesis")`.
1294|59|- A separate `chain.meta` records genesis and final digest.
1295|60|- A separate `artifacts/` directory holds content-addressed payloads.
1296|61|- `ra verify <run-dir>` rewinds the chain and prints first divergence.
1297|62|- `ra replay <run-dir>` re-emits observed payloads offline; it does not
1298|63|  re-execute tools.
1299|64|
1300|65|## Hard-fail patterns
1301|66|
1302|67|- `unwrap` / `expect` / `panic!` in lib code (enforced by `clippy`).
1303|68|- "ok" with `unwrap_or_default` in material paths.
1304|69|- Provider calls anywhere.
1305|70|- Mocks that hide the real chain digest.
1306|71|- Disabling a check to pass CI.
1307|72|- Random UUIDs in receipt identity (must be family-qualified).
1308|73|- Two distinct digests that should agree.
1309|74|
1310|75|## Finish-line focus (M0)
1311|76|
1312|77|- `ra run`, `ra verify`, `ra replay`, `ra doctor` from a clean tree.
1313|78|- `cargo test --workspace` green.
1314|79|- A negative tampering test that fails verification with a precise error.
1315|80|- All output captured under `docs/receipts/`.
1316|81|
1317|```
1318|### Tracked source excerpt: `README.md`
1319|
1320|```text
1321|1|# Recursive Agent Platform (M0)
1322|2|
1323|3|> Local-first, provenance-native agent platform in Rust. This is **M0**: the
1324|4|> smallest vertical slice that produces a tamper-evident receipt chain for
1325|5|> a deterministic run, verifies it offline, and replays it from disk with
1326|6|> no provider call.
1327|7|
1328|8|## What M0 is not
1329|9|
1330|10|- Not a Hermes or OpenClaw clone. It is a new platform that adopts useful
1331|11|  *behaviors* (CLI, receipts, replay, scopes) without copying source,
1332|12|  brand, or upstream contracts.
1333|13|- Not a provider integration. No Ollama, no OpenAI-compatible call, no
1334|14|  network. That is **Phase 2**, gated on M0 acceptance.
1335|15|- Not a UI. CLI only.
1336|16|- Not MCP. That is **Phase 3**.
1337|17|- Not a sandboxed execution plane. That is **Phase 4**.
1338|18|
1339|19|## What M0 *is*
1340|20|
1341|21|A small Rust workspace at `~/Coding/recursive-agent/` that depends on
1342|22|canonical Libraries crates by path:
1343|23|
1344|24|- `boundary-compiler` for RFC 8785 JCS at every typed boundary.
1345|25|- `stack-ids` for family-qualified material IDs.
1346|26|- `bitemporal-runtime` for valid-time / recorded-time semantics.
1347|27|- `claim-ledger` for claim/evidence/provenance primitives.
1348|28|- Local crates:
1349|29|  - `recursive-agent-contracts` — typed protocol.
1350|30|  - `recursive-agent-ledger` — append-only chain + content-addressed
1351|31|    artifact store.
1352|32|  - `recursive-agent-policy` — permits, lineage, allowlist.
1353|33|  - `recursive-agent-tools` — `echo` and `time_now` manifests.
1354|34|  - `recursive-agent-runner` — typed run DAG, deterministic walk.
1355|35|  - `recursive-agent-cli` — `ra run`, `ra verify`, `ra replay`,
1356|36|    `ra doctor`.
1357|37|
1358|38|## Quick start
1359|39|
1360|40|```bash
1361|41|cd ~/Coding/recursive-agent
1362|42|cargo build --release
1363|43|./target/release/ra doctor
1364|44|./target/release/ra run fixtures/hello-run.json
1365|45|./target/release/ra verify <run-dir-printed-above>
1366|46|```
1367|47|
1368|48|The first run prints a `<run-dir>` under
1369|49|`~/.local/share/recursive-agent/runs/`. Capture stdout into
1370|50|`docs/receipts/` so the chain can be reproduced.
1371|51|
1372|52|## Layout
1373|53|
1374|54|```text
1375|55|recursive-agent/
1376|56|├── crates/
1377|57|│   ├── recursive-agent-contracts/
1378|58|│   ├── recursive-agent-ledger/
1379|59|│   ├── recursive-agent-policy/
1380|60|│   ├── recursive-agent-tools/
1381|61|│   ├── recursive-agent-runner/
1382|62|│   └── recursive-agent-cli/
1383|63|├── fixtures/
1384|64|├── scripts/
1385|65|├── docs/
1386|66|│   ├── adr/
1387|67|│   └── receipts/
1388|68|├── AGENTS.md
1389|69|└── Cargo.toml
1390|70|```
1391|71|
1392|72|## Capability matrix
1393|73|
1394|74|| Capability | Source | M0 |
1395|75||---|---|---|
1396|76|| Canonical JSON boundary | `boundary-compiler` | yes |
1397|77|| Family-qualified IDs | `stack-ids` | yes |
1398|78|| Bitemporal | `bitemporal-runtime` | in-memory |
1399|79|| Claim/evidence | `claim-ledger` | envelope only |
1400|80|| Provider | none | out of scope |
1401|81|| MCP | none | out of scope |
1402|82|| Messaging | none | out of scope |
1403|83|| Web UI | none | out of scope |
1404|84|| Sandbox | none | out of scope |
1405|85|
1406|```
1407|
1408|## Workstream 6: /home/sikmindz/Coding/agent-memory-kits
1409|
1410|- Branch: `main`
1411|- Upstream: `origin/main`
1412|- Working tree status:
1413|```text
1414|M shared/scripts/run-server.sh
1415|?? benchmarks/
1416|?? claude/plugins/semantic-memory/.mcp.json.bak-pre-grok-install
1417|?? docs/benchmarks/BENCHMARK_READINESS_2026-08-06.md
1418|?? docs/benchmarks/longmemeval-official/
1419|?? shared/scripts/benchmark-longmemeval-semantic-memory.py
1420|```
1421|- Diff stat:
1422|```text
1423|shared/scripts/run-server.sh | 5 +++++
1424| 1 file changed, 5 insertions(+)
1425|```
1426|- Five latest commits:
1427|```text
1428|2026-08-03T18:02:28-05:00	44c758d	chore: snapshot workspace state — openapi spec
1429|2026-08-02T19:54:28-05:00	a87af1f	fix: enforce governed agent profile and authority token forwarding
1430|2026-07-27T09:38:30-05:00	d828f60	fix: add MCP relay port support, rewrite Codex capture nudge, add audit remediation plans
1431|2026-07-21T22:56:06-05:00	8ca4436	docs: add agent-driven setup note to all install flows
1432|2026-07-21T22:54:51-05:00	f0b03a3	docs: mention agent-driven mnemes setup
1433|```
1434|### Tracked source excerpt: `README.md`
1435|
1436|```text
1437|1|# agent-memory-kits
1438|2|
1439|3|> **Persistent local-first memory, receipt-backed compaction, and claim/evidence provenance — for every AI coding agent.**
1440|4|> One repo, three companion MCP servers, nine agent hosts.
1441|5|
1442|6|[![crates.io: semantic-memory-mcp](https://img.shields.io/crates/v/semantic-memory-mcp?label=semantic-memory-mcp&style=for-the-badge)](https://crates.io/crates/semantic-memory-mcp)
1443|7|[![crates.io: semantic-memory](https://img.shields.io/crates/v/semantic-memory?label=semantic-memory&style=for-the-badge)](https://crates.io/crates/semantic-memory)
1444|8|[![crates.io: context-governor](https://img.shields.io/crates/v/context-governor?label=context-governor&style=for-the-badge)](https://crates.io/crates/context-governor)
1445|9|[![crates.io: claim-ledger](https://img.shields.io/crates/v/claim-ledger?label=claim-ledger&style=for-the-badge)](https://crates.io/crates/claim-ledger)
1446|10|[![9 host plugins](https://img.shields.io/badge/hosts-9-blueviolet?style=for-the-badge)](./#capability-matrix)
1447|11|[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)](#license)
1448|12|[![Local-first](https://img.shields.io/badge/data-100%25%20local-green?style=for-the-badge)](#privacy--local-first)
1449|13|
1450|14|## Verified release surface
1451|15|
1452|16|The companion packages are published independently from this kit. The versions below were checked against crates.io on **2026-07-18**; badges remain the live version indicator.
1453|17|
1454|18|| Package | Published version | Role | Source / release boundary |
1455|19||---|---:|---|---|
1456|20|| [`semantic-memory`](https://crates.io/crates/semantic-memory) | `0.5.14` | SQLite/FTS5 + vector memory library | [release source](https://github.com/RecursiveIntell/semantic-memory/tree/feat/full-integration) |
1457|21|| [`semantic-memory-mcp`](https://crates.io/crates/semantic-memory-mcp) | `0.5.6` | MCP transport, tool profiles, and loopback HTTP | [release source](https://github.com/RecursiveIntell/semantic-memory-mcp/tree/main) |
1458|22|| [`mnemes`](https://crates.io/crates/mnemes) | `0.1.1` | Multi-device memory control plane | [release source](https://github.com/RecursiveIntell/mnemes) |
1459|23|| [`context-governor`](https://crates.io/crates/context-governor) | `0.2.0` | Deterministic receipt-backed compaction | [registry package](https://crates.io/crates/context-governor) |
1460|24|| [`claim-ledger`](https://crates.io/crates/claim-ledger) | `0.2.1` | Claim/evidence/provenance ledger | [Libraries source](https://github.com/RecursiveIntell/Libraries/tree/main/claim-ledger) |
1461|25|
1462|26|Release facts are source-reported until reproduced locally. For a current runtime surface, use `tools/list` on the configured MCP binary; profile counts are deliberately not frozen in this README.
1463|27|
1464|28|![Architecture overview](.github/hero.svg)
1465|29|
1466|30|AI coding agents forget everything between sessions. This repo fixes that.
1467|31|
1468|32|## The memory builds over time
1469|33|
1470|34|Day 1 is empty. That is by design, not a bug. The recall hook gates on `SM_RECALL_MINTOP=0.58` cosine — an empty store returns nothing, and the hook fails open (no output, no block) on every prompt until the store has facts worth recalling. The system is not failing; it is waiting.
1471|35|
1472|36|The product is the compounding curve, not the first session.
1473|37|
1474|38|```
1475|39|day 1        day 7         day 30        day 90+
1476|40|  |           |              |              |
1477|41|  o-----------o--------------o--------------o-->
1478|42|  install     ~50 facts     ~500 facts    ~5000+ facts
1479|43|  empty store starting to   recall        recall
1480|44|              fill          useful        indispensable
1481|45|```
1482|46|
1483|47|**What to expect, honestly:**
1484|48|
1485|49|- **Day 1 (install day).** Empty store. The recall hook fires on every prompt and returns nothing every time. The MCP tools work. The doctor passes. Nothing to recall. This is correct.
1486|50|- **Days 2–14 (filling in).** The agent saves facts as it works — with judgment, never auto-dumped. `/memory-ingest <repo>` on each repo you touch populates the codebase namespace fast. Recall starts firing on the prompts where it has a hit, ignoring the rest. The user notices on a few specific questions.
1487|51|- **Days 15–60 (useful).** Recall fires on a meaningful fraction of prompts. The agent knows your stack, your conventions, your open questions. You stop restating context the agent should already have.
1488|52|- **Days 60+ (indispensable).** The agent answers cross-session questions that you would have to look up manually. Failed approaches don't get retried. Decisions don't get re-debated. The store is large enough that the cosine gate fires often and the answers are accurate.
1489|53|
1490|54|**What speeds the curve (do these on day 1):**
1491|55|
1492|56|```bash
1493|57|# 1. Install the three companion MCP servers
1494|58|cargo install semantic-memory-mcp context-governor claim-ledger
1495|59|
1496|60|# 2. Install a host plugin — Claude Code shown; the same shape works for all 9 hosts
1497|61|/plugin marketplace add RecursiveIntell/agent-memory-kits
1498|62|/plugin install semantic-memory@semantic-memory-kit
1499|63|/memory-setup
1500|64|
1501|65|# 3. Ingest the repos you actually work in
1502|66|/memory-ingest .
1503|67|/memory-ingest ../other-repo
1504|68|
1505|69|# 4. Restart the host so hooks load. Then work normally.
1506|70|```
1507|71|
1508|72|The hooked host's recall hook queries the warm HTTP server (BM25 + vector + RRF, fail-open) and injects only hits that clear `SM_RECALL_MINTOP=0.58`. A second-prompt later, the same facts come back without re-indexing. Receipts are written to `~/.local/share/semantic-memory-agent-kits/receipts/`. The day-1 install is the same in every README; the difference between day 1 and day 90 is what you do between.
1509|73|
1510|74|---
1511|75|
1512|76|## Table of contents
1513|77|
1514|78|- [What this repo is](#what-this-repo-is)
1515|79|- [Architecture](#architecture)
1516|80|- [Capability matrix](#capability-matrix)
1517|81|- [Per-host docs](#per-host-docs)
1518|82|- [Install](#install)
1519|83|- [RecursiveIntell Pro](#recursiveintell-pro)
1520|84|- [The three MCP companions](#the-three-mcp-companions)
1521|85|- [The codebase ingester](#the-codebase-ingester
1522|```
1523|### Tracked source excerpt: `claude/README.md`
1524|
1525|```text
1526|1|# semantic-memory for Claude Code
1527|2|
1528|3|> **Tier 0 reference implementation.** Lifecycle hooks (SessionStart / UserPromptSubmit / PreCompact / Stop), a memory-keeper subagent, capture/curator/maintenance/sync skills, and manifest-declared commands — over `semantic-memory-mcp` (profile-based tool counts, run `generate-tool-surface-docs.py` for current) + `context-governor` (13 CLI commands) + `claim-ledger` (5 tools).
1529|4|> Plugin marketplace path: `semantic-memory@semantic-memory-kit`.
1530|5|
1531|6|[![Tier 0](https://img.shields.io/badge/tier-0-blueviolet?style=for-the-badge)](#tier--scope)
1532|7|[![Local-first](https://img.shields.io/badge/data-100%25%20local-green?style=for-the-badge)](#)
1533|8|[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)](#)
1534|9|[![semantic-memory-mcp](https://img.shields.io/crates/v/semantic-memory-mcp?label=semantic-memory-mcp&style=for-the-badge)](https://crates.io/crates/semantic-memory-mcp)
1535|10|[![context-governor](https://img.shields.io/crates/v/context-governor?label=context-governor&style=for-the-badge)](https://crates.io/crates/context-governor)
1536|11|[![claim-ledger](https://img.shields.io/crates/v/claim-ledger?label=claim-ledger&style=for-the-badge)](https://crates.io/crates/claim-ledger)
1537|12|
1538|13|See the [top-level README](../README.md) for the full capability matrix, architecture overview, and Tier 0 vs Tier 1 distinction.
1539|14|
1540|15|## Tier / scope
1541|16|
1542|17|Tier 0 host plugin. This kit is the **reference implementation** that Tier 1 hosts (Cursor, Cline, Roo Code, Windsurf, Continue, OpenCode) reuse. The Tier 0 contract: real lifecycle hooks fire on SessionStart, UserPromptSubmit, PreCompact, and Stop, with deterministic fail-open behavior; capture is model-nudged (the model writes with judgment, not auto-dumped); and every claim of completion is backed by a receipt.
1543|18|
1544|19|## Architecture
1545|20|
1546|21|![Tier 0 hooked host architecture](../docs/assets/tier0-hooked-architecture.svg)
1547|22|
1548|23|Hook paths: `claude/plugins/semantic-memory/hooks/`. Script paths: `claude/plugins/semantic-memory/scripts/`. Skill paths: `claude/plugins/semantic-memory/skills/`. All relative to repo root.
1549|24|
1550|25|## Install
1551|26|
1552|27|From the repo root:
1553|28|
1554|29|```text
1555|30|/plugin marketplace add RecursiveIntell/agent-memory-kits
1556|31|/plugin install semantic-memory@semantic-memory-kit
1557|32|/memory-setup
1558|33|```
1559|34|
1560|35|Restart Claude Code once so hooks load. `/memory-setup` installs the binary and allowlists tools.
1561|36|
1562|37|## What you get
1563|38|
1564|39|### Hooks (4)
1565|40|
1566|41|`claude/plugins/semantic-memory/hooks/hooks.json` wires four lifecycle hooks. Every hook **fails open** — missing binary, timeout, or bad JSON exits 0 and never blocks the prompt.
1567|42|
1568|43|| Hook | Event | What it does | Fail-open |
1569|44||---|---|---|---|
1570|45|| `memory-primer.sh` | `SessionStart` (startup, resume, clear) | Injects project-scoped primer facts as `additionalContext` | yes — 12s timeout |
1571|46|| `memory-recall.sh` | `UserPromptSubmit` | Queries warm HTTP `/search` (BM25 + vector + RRF), injects hits that clear `SM_RECALL_MINTOP=0.58` as `additionalContext` | yes — 12s timeout |
1572|47|| `memory-capture-nudge.sh` | `PreCompact` and `Stop` | Reminds the model to save durable facts / decisions before the conversation ends or compacts | yes — 5s timeout |
1573|48|| `_resolve.sh` | helper, not a hook event | Resolves the plugin's `${CLAUDE_PLUGIN_ROOT}` to the absolute path so siblings can find binaries | n/a |
1574|49|
1575|50|### Scripts
1576|51|
1577|52|`claude/plugins/semantic-memory/scripts/` includes MCP wrappers, doctor/benchmark helpers, ingestion, proof/evidence helpers, admin server launchers, and context-governor audit wrappers. Avoid hardcoded script counts here; the script directory is the source of truth.
1578|53|
1579|54|- `context-governor-mcp.py` — MCP server entry for `context-governor` (4 `cg_*` tools)
1580|55|- `claim-ledger-mcp.py` — MCP server entry for `claim-ledger` (5 `cl_*` tools)
1581|56|- `context-governor-compact.py` — deterministic transcript compaction, writes receipt
1582|57|- `doctor-all.py` — runs all kit doctors and writes a JSON receipt bundle
1583|58|- `benchmark-retrieval.py` — quality benchmark over warm HTTP
1584|59|- `benchmark-context-governor.py` — compaction latency / ratio benchmark
1585|60|- `ingest_codebase.py` — language-agnostic repo ingester
1586|61|- `evidence-workbench.py`, `proof-packet.py` — proof/evidence packet helpers
1587|62|- `context-governor-audit.py` — context-governor audit wrapper
1588|63|- `run-server.sh`, `run-server-admin.sh` — daily and admin semantic-memory launchers
1589|64|
1590|65|### Commands (2)
1591|66|
1592|67|- `/memory-setup` — install binary, allowlist tools, write rules (see `claude/plugins/semantic-memory/commands/memory-setup.md`)
1593|68|- `/memory-ingest <path>` — run `ingest_codebase.py` on a repo path (see `claude/plugins/semantic-memory/commands/memory-ingest.md`)
1594|69|
1595|70|### Agent (1)
1596|71|
1597|72|- `memory-keeper.md` — subagent that audits memory health, runs the curator, and re-anchors stale facts
1598|73|
1599|74|### Skills (9)
1600|75|
1601|76|Each skill is `claude/plugins/semantic-memory/skills/<name>/SKILL.md`:
1602|77|
1603|78|| Skill | Purpose |
1604|79||---|---|
1605|80|| `memory-capture` | When and how to save durable facts and decisions |
1606|81|| `memory-curator` | Reconcile duplicates, supersede stale facts, prune contradicted records |
1607|82|| `memory-maintenance` | Vacuum, re-embed stale vectors, run `doctor-all` |
1608|83|| `memory-sync` | Promote facts across namespaces; pair with `ingest_codebase.py` |
1609|84|| `knowledge-graph-explorer` | Use `sm_topology`, `sm_communities`, `sm_factor_graph` for second-order discovery |
1610|85|| `release-gate` | Run `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace` and store receipts |
1611|86|| `context-compaction` | Drive `context-governor-compact.py` before manual or auto compaction |
1612|87|| `claim-provenance` | Back material assertions with `cl_run` / `cl_evidence` / `cl_verify` |
1613|88|| `llm-output-parsing` | Use the `sm_parse_*` tools to handle think blocks, malformed JSON, trailing text |
1614|89|
1615|90|### MCP tools exposed
1616|91|
1617|92|The `seman
1618|```
1619|### Tracked source excerpt: `cline/README.md`
1620|
1621|```text
1622|1|# semantic-memory for Cline
1623|2|
1624|3|> **Tier 1 host plugin.** MCP-only integration; rule/context injection for behavioral guidance.
1625|4|
1626|5|[![Tier 1](https://img.shields.io/badge/tier-1-blueviolet?style=for-the-badge)](#capability-boundary)
1627|6|[![Local-first](https://img.shields.io/badge/data-100%25%20local-green?style=for-the-badge)](#)
1628|7|[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)](#)
1629|8|
1630|9|See [top-level README](../README.md) for the full capability matrix and architecture overview.
1631|10|
1632|11|This is the Cline MCP setup kit for semantic-memory-mcp.
1633|12|
1634|13|Capability boundary:
1635|14|- Works: exposes the `sm_*` semantic-memory MCP tools to Cline once the MCP config is registered.
1636|15|- Works: local-first memory storage, hybrid search, graph tools, provenance, supersession, claims, and manual/codebase-ingest workflows.
1637|16|- Works: context-injection via host rule/instruction files. The setup kit can install a semantic-memory rule that tells the agent to retrieve memory through MCP, or through the shared context command when shell execution is available.
1638|17|- Boundary: this is rule/instruction based for this host, not a guaranteed pre-prompt hook unless the host exposes a stable hook API.
1639|18|
1640|19|> **This is a Tier 1 kit.** Tier 1 hosts expose the MCP server to the agent and install host-native rule/instruction files that tell the agent to retrieve memory through MCP and preserve receipts. No transcript/prompt lifecycle hook is claimed.
1641|20|
1642|21|## Install
1643|22|
1644|23|From the repository root:
1645|24|
1646|25|```bash
1647|26|cline/scripts/setup.sh
1648|27|```
1649|28|
1650|29|Copy the printed `mcpServers.semantic-memory` snippet into Cline MCP settings.
1651|30|
1652|31|## Verify
1653|32|
1654|33|```bash
1655|34|cline/scripts/doctor.py
1656|35|```
1657|36|
1658|37|Expected:
1659|38|- `mcp_settings.json.example` parses as JSON.
1660|39|- `semantic-memory-mcp` binary is found.
1661|40|- memory dir exists.
1662|41|- MCP `tools/list` exposes `sm_search`, `sm_add_fact`, `sm_stats`, and `sm_supersede_fact`.
1663|42|
1664|43|## Use inside Cline
1665|44|
1666|45|Ask Cline to call the semantic-memory MCP tools, for example:
1667|46|
1668|47|```text
1669|48|Search semantic memory for facts about this repository before changing code.
1670|49|```
1671|50|
1672|51|or:
1673|52|
1674|53|```text
1675|54|Save this decision to semantic memory with namespace code:<repo-name> and source Cline.
1676|55|```
1677|56|
1678|57|## Notes
1679|58|
1680|59|If the warm HTTP health check warns, MCP stdio can still work. Warm HTTP is mainly for hook-based hosts; MCP tool use does not require it.
1681|60|
1682|61|
1683|62|## Context injection
1684|63|
1685|64|Install a workspace rule into a project:
1686|65|
1687|66|```bash
1688|67|shared/scripts/install-context-rules.py cline --scope workspace --workspace /path/to/project
1689|68|```
1690|69|
1691|70|Install a global rule where the host has a documented global-rule location:
1692|71|
1693|72|```bash
1694|73|shared/scripts/install-context-rules.py cline --scope global
1695|74|```
1696|75|
1697|76|The installed rule points at:
1698|77|
1699|78|```bash
1700|79|shared/scripts/semantic-memory-context.py --prompt "$USER_TASK"
1701|80|```
1702|81|
1703|82|That command queries the warm HTTP server first (`SEMANTIC_MEMORY_HTTP_PORT`, default `1739`) and falls back to stdio MCP. Returned entries are explicitly marked as recall, not ground truth.
1704|83|
1705|84|
1706|85|## Context compaction / receipts
1707|86|
1708|87|This kit also includes Context Governor as a companion MCP server and rule layer.
1709|88|
1710|89|- MCP server: `shared/scripts/context-governor-mcp.py`
1711|90|- Receipt-backed compact command: `shared/scripts/context-governor-compact.py`
1712|91|- Rule text: `shared/rules/context-governor.md`
1713|92|
1714|93|Use it when a Cline session is long, a handoff is needed, or context is about to be compacted. It preserves high-risk context and stores exact fallback receipts that can be searched and expanded later.
1715|94|
1716|95|Boundary: for hosts without a verified pre-compact hook, this is rule/command/MCP assisted. It does not claim automatic transcript capture unless the host exposes transcript messages to an extension/hook API.
1717|96|
1718|97|
1719|98|## Quick install
1720|99|
1721|100|Print config snippets only:
1722|101|
1723|102|```bash
1724|103|cline/scripts/setup.sh
1725|104|```
1726|105|
1727|106|Write project-local rule/config files:
1728|107|
1729|108|```bash
1730|109|cline/scripts/setup.sh --write-project /path/to/project
1731|110|```
1732|111|
1733|112|Write safe user/global rule files where this host supports them:
1734|113|
1735|114|```bash
1736|115|cline/scripts/setup.sh --write-user
1737|116|```
1738|117|
1739|118|Dry run before writing:
1740|119|
1741|120|```bash
1742|121|cline/scripts/setup.sh --dry-run --write-project /path/to/project
1743|122|```
1744|123|
1745|124|Verify:
1746|125|
1747|126|```bash
1748|127|cline/scripts/doctor.py
1749|128|shared/scripts/doctor-all.py --deep
1750|129|```
1751|130|
1752|131|## Architecture
1753|132|
1754|133|![Tier 1 MCP architecture](../docs/assets/tier1-mcp-architecture.svg)
1755|134|
1756|135|## Design principles
1757|136|
1758|137|- **Rule-injection, not hook-injection.** Tier 1 hosts install host-native rule files that tell the agent to retrieve memory through MCP; no pre-prompt hook is claimed.
1759|138|- **MCP stdio is the only lifecycle path.** The host starts `semantic-memory-mcp` when it loads the MCP config; no warm HTTP sidecar is started by this host.
1760|139|
1761|140|These extend the [top-level Design principles](../README.md#design-principles); they don't replace them.
1762|141|
1763|142|## Troubleshooting
1764|143|
1765|144|| Symptom | Fix |
1766|145||---|---|
1767|146|| `mcp_settings.json.example` not parseable | `python3 -m json.tool cline/mcp_settings.json.example` — should print valid JSON. |
1768|147|| MCP not loading in Cline | Restart Cline after writing the MCP config; check Cline's MCP logs. |
1769|148|| Rule not auto-applying | Verify the rule path with `cline/scripts/setup.sh --write-user` produced the expected rule file. |
1770|149|
1771|```
1772|
1773|## Workstream 7: /home/sikmindz/Coding/mnemes
1774|
1775|- Branch: `main`
1776|- Upstream: `origin/main`
1777|- Working tree status:
1778|```text
1779|(clean)
1780|```
1781|- Diff stat:
1782|```text
1783|(none)
1784|```
1785|- Five latest commits:
1786|```text
1787|2026-08-02T20:09:45-05:00	d38659b	docs: preserve Mnemes synchronization evidence
1788|2026-08-02T20:03:22-05:00	98c8492	feat(replication): add governed fact-create transport and durable retries
1789|2026-07-30T18:56:53-05:00	37feeb8	docs: add full-surface memory mesh execution plan
1790|2026-07-28T22:19:36-05:00	1181212	feat: harden feature-preserving sync recovery
1791|2026-07-26T20:02:54-05:00	8ec942c	docs: bump semantic-memory version refs to 0.6.0
1792|```
1793|### Tracked source excerpt: `.hermes/plans/mnemes-memory-mesh-lanes/implementation-packs/README.md`
1794|
1795|```text
1796|1|# Cheap-Model Implementation Packs — Controller Entry Point
1797|2|
1798|3|> **Purpose:** Make the six memory-mesh lanes safe enough for lower-cost implementation models without granting architectural, release, deployment, or cross-lane authority.
1799|4|> **Source authority:** `../00-lane-map.md` and `../../2026-07-30-mnemes-full-surface-memory-mesh.md`. This directory is an execution aid, not competing architecture.
1800|5|
1801|

## User context

Return a bounded council evidence context. Keep enough exact locators for twelve independent agents to cite and challenge. Missing or omitted evidence must remain explicit.