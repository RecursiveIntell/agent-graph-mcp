# Unfinished Work Evidence Pack

Generated from live local repositories for the 12-agent direction council.

## User decision context

- Near-term goal from canonical USER.md: start a business and/or get a good job in the next 30–60 days.
- Working preference: evidence-led, local-first, reproducible, bounded, rollback-aware systems.
- Council task: choose the highest-leverage next direction from unfinished work; do not equate dirty state with value or completion.

## Portfolio inventory

- Live scan: 41 Git repositories under `/home/sikmindz/Coding`; 13 reported dirty at capture time.
- This pack intentionally samples 12 active/relevant workstreams. Omitted repositories are missing evidence, not negative evidence.

## Deterministic bounding receipt



- Source SHA-256: `sha256:d9ff5bcb41c14326c0e2d75bfd2103a1047251a6e1fa3ad2a2548daf4762def9`

- Selection: first 2,400 UTF-8 characters from each of 12 workstream sections; no model summarization.

- Truncation is missing evidence, not negative evidence.



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
18|cargo fmt --check

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
32|### Session 6: Final gate (3

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
24|| Default retrieval | FTS5/BM25 plus dense-vector candi

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
39|- [ ] Check

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
 crates/recursi

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
16|The companion packages are published independently from this kit. The versions below

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
19|- `10-lane-0-source-anchors.md` through `15-lane-5-source-anchors.m

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
18|   cp deploy/config/recursiveops.exa

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
35|cargo test --manifest-path src-tauri/Cargo.toml --features semantic-memory-turbo-quant

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
76|- Public pages use `force-dynamic` so CMS edits show up after reva

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
25|- `app/

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

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
49|│   ├── ga

[WORKSTREAM EXCERPT TRUNCATED — inspect canonical repository before decisive judgment]

## Evidence rules for council agents

1. Current captured files and Git state outrank remembered summaries.
2. A dirty tree proves only uncommitted state, not importance or readiness.
3. Commit subjects are source metadata, not proof that tests passed or deployments succeeded.
4. Missing files, omitted repositories, and absent tests are unknown—not failures unless the contract requires them.
5. Every recommendation must cite workstream number plus exact captured evidence.
6. Separate observed, inferred, proposed, blocked, and degraded states.
7. Optimize for the user’s 30–60 day business/job goal, compounding technical leverage, credibility, and time to proof.
8. Preserve dissent and propose a falsifiable first experiment.
