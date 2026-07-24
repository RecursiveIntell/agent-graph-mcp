# Agent Graph MCP Integrity-Key Pilot Closure Plan

> **For Hermes:** Execute in place; do not install or alter Hermes/service configuration.

**Goal:** Provision an external integrity key, clear the strict clippy blocker, and prove the release candidate works against an isolated durable store.

**Current evidence — 2026-07-22:** Candidate release build and 16 unit + 44 integration tests pass. `AGENT_GRAPH_INTEGRITY_KEY_PATH` is unset. Strict clippy is blocked only by two redundant closures in `../llm-pipeline/src/backend/openai.rs` lines 226 and 282. No `agent-graph-mcp.service` was discovered. The Libraries workspace is substantially dirty; do not reset, commit, or modify unrelated paths.

## Task 1 — Protected key material
- **Create:** `/home/sikmindz/.config/agent-graph-mcp/integrity.key` (32 random bytes, mode `0600`); parent mode `0700`.
- **RED:** run candidate with a disposable data directory and no `AGENT_GRAPH_INTEGRITY_KEY_PATH`; expected `INTEGRITY_KEY_REQUIRED` for durable integrity-sensitive surface (covered by integration regression).
- **GREEN:** export the explicit key path only for pilot commands and run the durable integration suite.
- **Rollback:** securely remove only the created key directory after explicit user direction; never print its content.

## Task 2 — Strict clippy closure
- **Modify:** `../llm-pipeline/src/backend/openai.rs:226,282` only.
- **RED:** `cargo clippy -p llm-pipeline --all-targets -- -D warnings` fails with `redundant_closure`.
- **GREEN:** replace `map_err(|error| PipelineError::Request(error))` with `map_err(PipelineError::Request)` at both sites; rerun the targeted clippy command.
- **Rollback:** restore only those two expressions.

## Task 3 — Isolated candidate pilot
- **Inputs:** candidate `target/release/agent-graph-mcp`, explicit external key env, disposable `mktemp -d` data directory.
- **Checks:** release build; full agent-graph-mcp test suite with the external key; strict targeted clippy; candidate binary help/startup smoke without altering installed binary; verify candidate/installed hashes differ and data directory is separate.
- **Gate:** do not install/activate. Pilot evidence licenses only an isolated candidate claim.
- **Rollback:** remove the disposable pilot directory; retain key for future approved activation.
