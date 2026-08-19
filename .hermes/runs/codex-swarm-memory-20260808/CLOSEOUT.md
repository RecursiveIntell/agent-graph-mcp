# Nine-way concurrent Luna validation closeout — 2026-08-08

## Verdict

**PASS for one bounded nine-at-once run, with tight soft-memory headroom.**

Successful run: `run-19fe214d3bf-1`

- Graph: `luna-next-path-large-bounded-20260808`
- Graph version: `sha256:8a42bd9ad08761ab0c6dc3aad6b60986fc918e81c122112a8fb903d3c028cc1c`
- Provider/model: `codex-app-server://` / `gpt-5.6-luna`
- Reasoning: explicitly pinned to `low` in both App Server launch paths
- Runtime: 76,843 ms
- Receipt counters: 12 graph nodes, 10 LLM calls
- Nine fan-out LLM lanes: all succeeded
- Synthesis: succeeded
- Failed attempts: 0
- Budget exhaustion: none
- Persistence status: durable terminal

## Concurrency evidence

Sampler: `nine-concurrent-low.csv`

- Maximum Codex-related process count: 18
- Architecture: nine Node launchers plus nine native App Server processes
- Samples with at least nine Codex-related processes: 47
- Measured overlap at 250 ms sampling: 11.75 seconds
- Maximum cgroup process count: 41

This is true provider concurrency, not nine graph branches serialized through one persistent worker.

## Memory and service evidence

- Fresh run cgroup peak: 2,151,055,360 bytes
- Configured `MemoryHigh`: 2,147,483,648 bytes
- Configured `MemoryMax`: 4,294,967,296 bytes
- Maximum aggregate process RSS: 1,847,688 KiB
- Maximum Codex-related RSS: 1,724,544 KiB
- Per-field sampled maxima: anon 382,922,752 bytes; file 1,891,217,408 bytes; kernel 207,265,792 bytes. These occurred at different samples and are not additive.
- `memory.events`: max 0; oom 0; oom_kill 0; oom_group_kill 0
- `MemoryHigh` pressure events: 13,402
- Daemon restarts: 0
- Post-run cgroup process tree: daemon only; all one-shot workers exited

The run exceeded the 2 GiB soft boundary by about 3.6 MB and was throttled, but stayed roughly 2.14 GiB below the 4 GiB hard limit. Nine-way operation is proven once, but it does not have comfortable soft-limit headroom.

## Changes deployed

- `AGENT_GRAPH_CODEX_MAX_PROCESSES=9` selects bounded concurrent one-shot workers.
- Process limit 1 still selects the reusable persistent worker.
- Both paths explicitly pass `model_reasoning_effort="low"`.
- Inherited Codex MCP servers and plugin/app injection remain disabled.
- Stdio JSON-RPC lines are bounded at 4 MiB + 1-byte detection; assistant output remains bounded at 256 KiB.
- Completed persistent-worker threads are deleted before reuse.

Installed and release daemon SHA-256:

`c6492fa555b173d1cf3481aac7308515317e771f338aff7a399ad0b7fb4cbfd9`

## Validation gates

- `cargo check --lib`: pass
- `tests/codex_app_server.rs`: 9/9 pass
- Release build: pass
- Deployment receipt: pass
- `git diff --check`: pass
- Live nine-way run: pass

Build receipt:

`.hermes/receipts/receipts/2026-08-08-9dff015a302b4141bde3d310b9b0be64.json`

Deployment receipt:

`.hermes/receipts/receipts/2026-08-08-de63b2ebe2b84b4d88f4308d0222adfb.json`

## Diagnostic runs retained

- `run-19fe20b12fd-1`: all nine attempted concurrently; failed because one JSON-RPC line exceeded the old 1 MiB bound.
- `run-19fe20e2ced-1`: 8/9 completed; one lane timed out at 120 seconds because workers inherited `xhigh` reasoning.
- `run-19fe214d3bf-1`: all-low final acceptance run; successful.

## Evidence limitations

- The canonical graph receipt reports `evidence_authority=structural_unverified`, `receipt_digest=null`, and `storage_class=volatile_live`, although the execution row is durable terminal and records `status=completed`, `failed_attempts=0`.
- The sampler's historical `mcp_process_count` field used substring matching and falsely counted suppression arguments such as `mcp_servers.<name>.enabled=false`. It was corrected after this run. Do not use that column from existing CSVs as proof of MCP child processes.
- Post-run cgroup memory remained high despite the process tree returning to daemon-only; sampled evidence attributes most peak pressure to file-backed charging. This is not worker residency, but cooldown/reclaim behavior should be monitored in repeated runs.

## Rollback

Rollback material is staged under:

`/home/sikmindz/.hermes/staging/agent-graph-mcp-isolation-20260808T153517Z`

Restore the staged binaries and launcher, then restart `agent-graph-mcpd.service`. Verify hashes, `ActiveState=active`, `NRestarts`, process cap, and a one-node admission probe before reopening traffic.

## Next proof gate

Before treating nine as the durable production envelope, run at least three identical nine-way repetitions and require:

- 9/9 lanes plus synthesis every time;
- zero failed attempts, timeout, OOM, and restart events;
- all workers cleaned up;
- no monotonic post-run cgroup growth;
- a deliberate decision on whether to raise `MemoryHigh` slightly or retain nine with soft-limit throttling.
