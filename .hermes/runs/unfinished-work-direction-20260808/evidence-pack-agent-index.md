# Unfinished Work Evidence Pack

Generated from live local repositories for the 12-agent direction council.

## User decision context

- Near-term goal from canonical USER.md: start a business and/or get a good job in the next 30–60 days.
- Working preference: evidence-led, local-first, reproducible, bounded, rollback-aware systems.
- Council task: choose the highest-leverage next direction from unfinished work; do not equate dirty state with value or completion.

## Portfolio inventory

- Live scan: 41 Git repositories under `/home/sikmindz/Coding`; 13 reported dirty at capture time.
- This pack intentionally samples 12 active/relevant workstreams. Omitted repositories are missing evidence, not negative evidence.

## Council index bounding receipt

- Raw source SHA-256: `sha256:d9ff5bcb41c14326c0e2d75bfd2103a1047251a6e1fa3ad2a2548daf4762def9`

- Selection: first 700 UTF-8 characters of every workstream section.

- This index is triage evidence only; controller verification against canonical repositories is mandatory.

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
2026
[INDEX EXCERPT ENDS]

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
2026-08-07T02:43:49-05:00	0b3a0317	feat
[INDEX EXCERPT ENDS]

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
2026-08-02T21:15:15-05:00	b
[INDEX EXCERPT ENDS]

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
2026-08-06T17:18:11-05:
[INDEX EXCERPT ENDS]

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
 M crates/
[INDEX EXCERPT ENDS]

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
2026-08-02T19:54:28-05:00	a87af1f	fix: enforce governed agent profile and authority token f
[INDEX EXCERPT ENDS]

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
### Tracked source excerpt: `.hermes/plans/mnemes-memory-mesh-la
[INDEX EXCERPT ENDS]

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
7|1. Deterministic core: inventory, checks, parsing, diffing, applying pat
[INDEX EXCERPT ENDS]

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
### Tracked source ex
[INDEX EXCERPT ENDS]

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
9|| `/` | Home — hero, featured work, proof, stack,
[INDEX EXCERPT ENDS]

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
2026-08-03T21:58:55-05:00	a492d9e	fix(installer): auto-register MCP
[INDEX EXCERPT ENDS]

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
7|- **Dual Generation Modes** — Standard mode for quick gener
[INDEX EXCERPT ENDS]

## Evidence rules for council agents

1. Current captured files and Git state outrank remembered summaries.
2. A dirty tree proves only uncommitted state, not importance or readiness.
3. Commit subjects are source metadata, not proof that tests passed or deployments succeeded.
4. Missing files, omitted repositories, and absent tests are unknown—not failures unless the contract requires them.
5. Every recommendation must cite workstream number plus exact captured evidence.
6. Separate observed, inferred, proposed, blocked, and degraded states.
7. Optimize for the user’s 30–60 day business/job goal, compounding technical leverage, credibility, and time to proof.
8. Preserve dissent and propose a falsifiable first experiment.
