# agent-graph-mcp

**MCP server for graph-orchestrated LLM workflows** — 25 typed tools, daemon/proxy architecture, checkpoint/resume, human-in-the-loop approvals, and HMAC-authenticated execution receipts.

[![Crates.io](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
[![docs.rs](https://img.shields.io/docsrs/agent-graph-mcp)](https://docs.rs/agent-graph-mcp)
[![MCP Badge](https://lobehub.com/badge/mcp-full/recursiveintell-agent-graph-mcp?theme=light)](https://lobehub.com/mcp/recursiveintell-agent-graph-mcp)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

![Architecture](assets/architecture.svg)

## What it gives you

- **25 typed MCP tools** — graph lifecycle, execution (sync + async), state inspection, checkpoint/resume, HITL approvals, source witnesses, templates, policy validation
- **Daemon + proxy architecture** — single-process daemon with file lock ownership, crash recovery, and startup mode enforcement; stateless proxy that bridges stdin/stdout to Unix socket
- **Durable persistence** — SQLite-backed with atomic checkpoint transactions, no partial rows after crash
- **Deterministic local checkpoint/resume** — HMAC-SHA256 authenticated checkpoints for linear chains of deterministic `passthrough` and `state_transform` nodes
- **Built-in templates** — `council_deliberation` (3-analyst parallel), `parallel_council` (debate), `plan_critique_refine`, `analysis_pipeline`, `classifier_router`
- **Evidence witnessing** — caller-supplied source capture with HMAC-authenticated receipts

![Tool Overview](assets/tools-overview.svg)

## Quick start

### npx (recommended)

```bash
npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
```

### Cargo install

```bash
cargo install agent-graph-mcp --locked
agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
```

### Hermes config

```yaml
mcp_servers:
  agent_graph:
    command: npx
    args:
      - -y
      - "@recursiveintell/agent-graph-mcp"
      - --direct
      - --base-url
      - http://127.0.0.1:11434
      - --model
      - glm-5.2:cloud
```

## Tools (25)

**Graph lifecycle:** `graph_create`, `graph_list`, `graph_inspect`, `graph_render`
**Execution:** `graph_execute`, `graph_run_start`, `graph_run_wait`, `graph_run_cancel`, `graph_run_get`
**State:** `graph_run_state`, `graph_run_events`, `graph_run_checkpoint`, `graph_run_resume`
**Approval:** `graph_approval_list`, `graph_approval_get`, `graph_approval_request`
**Evidence:** `graph_source_witness_capture`, `graph_source_witness_get`
**Templates:** `graph_template_list`, `graph_template_instantiate`, `graph_template_candidates`, `graph_template_outcomes`
**Policy:** `graph_policy_check`
**Receipt:** `graph_run_receipt`
**Status:** `graph_status`

## Built-in templates

| Template | Description |
|----------|-------------|
| `council_deliberation` | 3-analyst parallel council: coordinator → fanout → researchers → join → synthesize |
| `parallel_council` | 2-person debate: optimist vs skeptic → join → judge |
| `plan_critique_refine` | Sequential plan → critique → refine |
| `analysis_pipeline` | planner → researcher → extractor → synthesizer → validator with correction loop |
| `classifier_router` | LLM classifier routes input to bug/feature/question handlers |

## Verification

```bash
# npx smoke test
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | \
  npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud 2>/dev/null | \
  grep -c '"name"'
# Expected: 25

# Build + test
cargo build --release
cargo test --lib --test daemon_recovery --test mcp_integration
cargo clippy -- -D warnings
```

## Ecosystem

| Crate | Description |
|-------|-------------|
| [agent-graph-mcp](https://crates.io/crates/agent-graph-mcp) | MCP server (this crate) |
| [ri-agent-graph](https://crates.io/crates/ri-agent-graph) | Core graph execution engine |
| [llm-pipeline](https://crates.io/crates/llm-pipeline) | Reusable LLM node payloads |
| [stack-ids](https://crates.io/crates/stack-ids) | Trace/identity primitives |

## License

MIT © [RecursiveIntell](https://github.com/RecursiveIntell)
