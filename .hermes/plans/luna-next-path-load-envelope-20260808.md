# Luna Codex App-Server Load Envelope Plan

Date: 2026-08-08
Repository: `/home/sikmindz/Coding/agent-graph-mcp-release`
Provider path: `codex-app-server://`
Model: `gpt-5.6-luna`

## Decision

The best next path is **controlled live concurrency-envelope validation of the Rust Codex app-server boundary**. Do not begin unrelated feature work or broaden into a general rewrite.

The current implementation now reuses one long-lived Codex App Server worker, materially removing the per-node process multiplier. It is not yet certified for the workload that caused the incident. The next gate must establish the highest safe fan-out and measure retained graph state, worker RSS, queue wait, and service memory under that workload.

## Verified basis

- `cargo test --test codex_app_server -- --test-threads=1`: 8 passed.
- `cargo test --test mcp_integration -- --test-threads=1`: 54 passed.
- Release build passed for `agent-graph-mcp` and `agent-graph-mcpd`.
- Installed daemon/proxy hashes matched the release build after promotion.
- Live launcher uses `gpt-5.6-luna` and starts one persistent App Server worker.
- Four-way Luna MCP graph completed durably through the persistent worker: 4/4 calls, 117 seconds, `NRestarts=0`.
- Earlier unrestricted ten-way Luna workload was killed by `systemd-oomd`; it produced no trustworthy functional verdict.
- The earlier three-process admission implementation exposed a queue bug: admission wait consumed the per-turn timeout. The persistent worker now serializes turns behind a single session lock, and provider timeout starts when the worker begins the turn.
- New nine-way persistent-worker run `run-19fe171a1c9-1` was durable but incomplete: 9 provider attempts, 1 completed analyst (`operations`), `security` hit the Codex App Server turn-completion timeout, seven other lanes were interrupted by graph cancellation, and join/synthesis did not execute. Service stayed at `NRestarts=0` with one worker and no OOM event; receipt digest `hmac-sha256:561cffb9c9814bab8e61399160754f9769c843a6cc288b25524ef675615f6133`.

## Implemented Rust controls

Owned source: `src/codex_app_server.rs`.

- One persistent Codex App Server worker per daemon by default.
- A bounded Rust-owned session serializes turns and recreates the worker after a failed or timed-out turn.
- Prompt cap: 256 KiB.
- JSON-RPC response-line cap: 1 MiB.
- Streamed assistant-output cap: 256 KiB.
- Stderr retained as an 8 KiB tail, redacted before MCP-visible errors.
- Streaming text accumulation instead of retaining the complete event history.
- Process-group termination when the persistent worker is replaced after timeout or failure.

## Phase 1 — source and live baseline

1. Verify repository status and exact installed hashes.
2. Verify service state, `NRestarts`, `MemoryCurrent`, `MemoryPeak`, `MemoryHigh`, and `MemoryMax`.
3. Verify no stale Codex child process groups remain.
4. Preserve the rollback stage:
   `/home/sikmindz/.hermes/staging/agent-graph-persistent-appserver-final-20260808T124014Z`.

Stop if the service is not healthy or executable identity differs.

## Phase 2 — controlled load matrix

Run through Agent Graph MCP, not Python orchestration, using compact prompts and durable terminal persistence:

| Level | Graph LLM nodes | Persistent workers | Required result |
|---:|---:|---:|---|
| 1 | 1 | 1 | 1/1 durable success |
| 2 | 2 | 1 | 2/2 durable success |
| 3 | 3 | 1 | 3/3 durable success |
| 4 | 4 | 1 | 4/4 durable success; already observed |
| 9 | 9 | 1 | 9/9 durable success through the serialized worker |
| 10 | 10 | 1 | Run only if level 9 passes twice |

For every level record:

- graph ID, graph version, run ID, and receipt;
- configured model and provider;
- LLM call count and node count;
- wall-clock duration;
- service `NRestarts` before/after;
- `MemoryCurrent`, `MemoryPeak`, `MemoryHigh`, `MemoryMax`;
- systemd-oomd/kernel journal evidence;
- process-group and child-process cleanup;
- success, typed failure, timeout, or nondurable terminal status.

## Acceptance gates

A level is green only when:

1. Every requested LLM call reaches a durable terminal result.
2. The daemon remains active with `NRestarts=0`.
3. No `systemd-oomd` or kernel OOM event occurs.
4. Peak memory remains below `MemoryMax` (4 GiB).
5. A worker turn timeout covers provider execution and does not fail merely because graph scheduling was queued before the worker began.
6. Exactly one healthy persistent Codex App Server worker remains; no duplicate or orphaned worker process group exists.
7. Installed binary hashes remain unchanged during the run.

Crossing `MemoryHigh` (2 GiB) is a warning and requires review; it is not by itself a pass or fail. An OOM event, daemon restart, missing durable result, duplicate/orphaned worker, or unexplained timeout is a red gate.

## Phase 3 — disposition

- If 9-way passes twice: evaluate 10-way once under the same gates.
- If 9-way fails without OOM: inspect queue timing, provider latency, retained fanout/join state, and lifecycle cleanup; do not add workers automatically.
- If any level causes OOM/restart: mark the highest prior level as the safe envelope, keep one persistent worker, and reduce graph-side retained state before considering another worker.
- Do not claim that 85-way capacity transfers from API/DeepSeek to Codex App Server. Provider-process memory is a separate capacity regime.

## Rollback

Stop the stress run. Restore the staged daemon/proxy pair from:

`/home/sikmindz/.hermes/staging/agent-graph-codex-rust-admission-queue-20260808T121711Z`

Restart `agent-graph-mcpd.service` through systemd. Verify:

- installed hashes match the staged artifacts;
- service active and socket present;
- `NRestarts=0` after recovery;
- app-server tests remain green;
- MCP integration remains 54/54;
- the four-way Luna durable smoke still passes.

## Current proof debt

- The bounded nine-analyst decision swarm ran through MCP with Luna, but its final synthesis was not certified: seven analysts completed and one provider timed out before the join. The analyst consensus is advisory only.
- Nine-way and ten-way load-envelope behavior remains unverified after the queue-timeout fix.
- Per-process RSS and admission-wait telemetry are not yet present in durable receipts; current evidence requires external service/process measurements.
- Full workspace `cargo fmt --check` still has unrelated pre-existing formatting drift outside this scoped change.
