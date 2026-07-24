# Agent Graph MCP Final Activation Plan

> **For Hermes:** Execute only the steps below; retain rollback artifacts until live MCP test passes.

**Goal:** Replace the disabled, stale Agent Graph MCP binary with the verified candidate and enable it in the active Hermes profile with an external HMAC key path.

**Observed baseline — 2026-07-22:** `agent_graph` is present but disabled in `/home/sikmindz/.hermes/config.yaml`; it points at the old `~/.cargo/bin/agent-graph-mcp` and incorrectly stores `args` as a JSON string. No agent-graph systemd unit/process exists. Candidate SHA-256 is `f4c7ba0495431fac56a4f9f947cb1ff94a394e723d63fc8d864c8c51f6c200d2`; installed SHA-256 is `bc20380149484aa9f273d0668e0880584c1baa2b1c7623f57a5246b5f8a1f3ca`.

## Task 1 — Snapshot
- Back up `~/.cargo/bin/agent-graph-mcp`, `~/.hermes/config.yaml`, and `~/.hermes/.env` under `~/.hermes/backups/agent-graph-activation-<UTC>/` with restrictive permissions.
- Record old/new binary hashes and config backup path.
- **Rollback:** restore these three artifacts, then restart/reload the affected Hermes MCP process.

## Task 2 — Activate strict runtime contract
- Install candidate atomically to `~/.cargo/bin/agent-graph-mcp` while preserving executable permissions.
- In the existing `mcp_servers.agent_graph` entry:
  - convert `args` to a YAML list;
  - preserve local base URL/model;
  - use data dir `/home/sikmindz/.agent-graph`;
  - set `enabled: true`.
- Add exactly `AGENT_GRAPH_INTEGRITY_KEY_PATH=/home/sikmindz/.config/agent-graph-mcp/integrity.key` to Hermes `.env`, never the HMAC key itself.
- **Rollback trigger:** malformed config, test failure, unexpected tool schema, or any startup failure.

## Task 3 — Live proof and rollback readiness
- Validate config, run `hermes mcp test agent_graph`, verify tool discovery/status, then reload/restart only as required by Hermes.
- Confirm `agent_graph` is enabled and spawned with `--data-dir /home/sikmindz/.agent-graph`.
- **Success claim:** active local MCP candidate with protected durable integrity boundary. Not external-source verification, generic replay, generic HITL, or enforced LLM-call budgets.
