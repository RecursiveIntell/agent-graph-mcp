# agent-graph-mcp

**MCP server for graph-orchestrated LLM workflows** — 25 typed `rmcp` tools, daemon/proxy architecture, SQLite persistence, deterministic checkpoint/resume, human-in-the-loop approvals, source witnesses, and HMAC-authenticated execution receipts.

[![Crates.io](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
[![docs.rs](https://img.shields.io/docsrs/agent-graph-mcp)](https://docs.rs/agent-graph-mcp)
[![MCP Badge](https://lobehub.com/badge/mcp-full/recursiveintell-agent-graph-mcp?theme=light)](https://lobehub.com/mcp/recursiveintell-agent-graph-mcp)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

> **Expose the `ri-agent-graph` runtime engine over MCP.** Compile declarative JSON workflow specs into graph topologies, execute them synchronously or asynchronously, inspect state, checkpoint/resume, request human approval, capture source witnesses, and get cryptographic receipts — all through typed MCP tools.

![Architecture](assets/architecture.svg)

## What it gives you

- **25 typed MCP tools** — graph lifecycle, execution (sync + async), state inspection, checkpoint/resume, HITL approvals, source witnesses, templates, policy validation
- **Daemon + proxy architecture** — `agent-graph-mcpd` daemon owns SQLite with file lock, crash recovery, and startup mode enforcement; `agent-graph-mcp` proxy bridges MCP stdio to the daemon over a Unix socket
- **5 built-in templates** — `council_deliberation`, `parallel_council`, `plan_critique_refine`, `analysis_pipeline`, `classifier_router`
- **Deterministic local checkpoint/resume** — HMAC-SHA256 authenticated checkpoints, atomic one-shot resume, checkpoint mismatch detection
- **Human-in-the-loop** — `graph_approval_request` creates checkpoint-bound approvals; approve/reject via `graph_run_resume`
- **Source witnessing** — `graph_source_witness_capture` / `graph_source_witness_get` with HMAC-authenticated receipts
- **Crash recovery** — interrupted runs report `interrupted` after restart; no fake `running` or `completed` state

![Tool Overview](assets/tools-overview.svg)

![HITL Workflow](assets/hitl-workflow.svg)

## Prerequisites

- **Rust** 1.75+ for building from source
- **SQLite** — bundled via `rusqlite`, no system library required
- **Ollama** (optional) — for LLM node execution. Default model: `glm-5.2:cloud`

## Installation

### npx (recommended)

```bash
npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
```

### Cargo install

```bash
cargo install agent-graph-mcp --locked
agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
```

### Build from source

```bash
git clone https://github.com/RecursiveIntell/agent-graph-mcp.git
cd agent-graph-mcp
cargo build --release
./target/release/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
```

## Quick start

### Daemon mode (production)

```bash
# Start the daemon
mkdir -p ~/.local/share/agent-graph
openssl rand -hex 32 > ~/.local/share/agent-graph/integrity.key
agent-graph-mcpd --data-dir ~/.local/share/agent-graph --socket /tmp/agent-graph.sock &

# Use the proxy (MCP stdio ↔ socket)
agent-graph-mcp --socket /tmp/agent-graph.sock
```

### Direct mode (development)

```bash
agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
```

## Client configuration

### Hermes Agent

```yaml
mcp_servers:
  agent_graph:
    command: agent-graph-mcp
    args:
      - --socket
      - /tmp/agent-graph.sock
    enabled: true
```

### Claude Desktop

```json
{
  "mcpServers": {
    "agent-graph": {
      "command": "npx",
      "args": ["-y", "@recursiveintell/agent-graph-mcp", "--direct", "--base-url", "http://127.0.0.1:11434", "--model", "glm-5.2:cloud"]
    }
  }
}
```

### Generic MCP client (stdio)

```bash
npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud
```

## Architecture

```
┌─────────┐     stdin/stdout      ┌──────────────┐     Unix socket      ┌──────────────────┐     SQLite
│  MCP    │ ────────────────────→ │    Proxy     │ ────────────────────→ │     Daemon       │ ─────────→
│ Client  │ ←──────────────────── │ agent-graph- │ ←──────────────────── │ agent-graph-mcpd │ ←────────
└─────────┘     JSON-RPC 2.0      │     mcp      │     framed           │                  │    persistence
                                   └──────────────┘                      └──────────────────┘
```

| Component | Role |
|-----------|------|
| **Daemon** (`agent-graph-mcpd`) | Single-process owner with file lock. Tokio async Unix socket listener. SQLite persistence. Startup mode enforcement (keyed/keyless). Crash recovery — interrupted runs flagged on restart. |
| **Proxy** (`agent-graph-mcp`) | Stateless stdin/stdout → framed socket bridge. `--direct` flag runs everything in-process for simple deployments. |
| **Socket** | 0600 permissions. 4-byte BE length prefix + JSON-RPC 2.0 framing. |

### Daemon lifecycle

```bash
# View logs
RUST_LOG=debug agent-graph-mcpd --data-dir ~/.local/share/agent-graph --socket /tmp/agent-graph.sock

# systemd service (included in packaging/systemd/)
systemctl --user enable agent-graph-mcpd
systemctl --user start agent-graph-mcpd

# Integrity key (required for checkpoint/resume, approvals, receipts)
export AGENT_GRAPH_INTEGRITY_KEY_PATH=~/.local/share/agent-graph/integrity.key
```

## Tools reference (25)

### Graph lifecycle

| Tool | Description |
|------|-------------|
| `graph_create` | Create/validate/delete a graph from JSON spec or template |
| `graph_list` | List all registered graphs with metadata |
| `graph_inspect` | Full topology: nodes, edges, Mermaid diagram, hash, reducers |
| `graph_render` | Render as Mermaid diagram or JSON topology |

### Execution

| Tool | Description |
|------|-------------|
| `graph_execute` | Sync (blocking) or async execution |
| `graph_run_start` | Async start → returns `run_id` immediately; optional budgets |
| `graph_run_wait` | Block until terminal state with timeout |
| `graph_run_cancel` | Cooperative cancellation (best-effort) |
| `graph_run_get` | Current status, budget usage, pending approvals |

### State & checkpointing

| Tool | Description |
|------|-------------|
| `graph_run_state` | Live in-memory state projection |
| `graph_run_events` | Replay event stream from cursor |
| `graph_run_checkpoint` | Durable checkpoint read with integrity verification |
| `graph_run_resume` | Atomic one-shot resume from checkpoint |

### Approval (HITL)

| Tool | Description |
|------|-------------|
| `graph_approval_list` | List pending/expired/resolved approvals |
| `graph_approval_get` | Read specific approval metadata |
| `graph_approval_request` | Create checkpoint-bound HITL approval |

### Evidence & witnesses

| Tool | Description |
|------|-------------|
| `graph_source_witness_capture` | Persist caller-supplied source content (HMAC-authenticated) |
| `graph_source_witness_get` | Read witness with authentication tag verification |

### Templates

| Tool | Description |
|------|-------------|
| `graph_template_list` | List 5 built-in templates |
| `graph_template_instantiate` | Template → graph spec |
| `graph_template_candidates` | Promotion candidates |
| `graph_template_outcomes` | Recorded outcome history |

### Policy & receipts

| Tool | Description |
|------|-------------|
| `graph_policy_check` | Preflight validation |
| `graph_run_receipt` | Canonical execution receipt (HMAC-SHA256) |
| `graph_status` | Query server/graph/run/events/receipt/templates |

## Graph spec format (v2)

```json
{
  "spec_version": "2",
  "name": "my-council",
  "entry": "coordinator",
  "max_iterations": 32,
  "max_parallelism": 3,
  "nodes": [
    {"id": "coordinator", "type": "llm", "prompt": "Break into workstreams: {input}", "json_mode": true, "config": {"output_key": "workstreams"}},
    {"id": "fanout", "type": "passthrough"},
    {"id": "analyst_0", "type": "llm", "prompt": "Research angle A: {input}", "config": {"output_key": "r0"}},
    {"id": "analyst_1", "type": "llm", "prompt": "Research angle B: {input}", "config": {"output_key": "r1"}},
    {"id": "join", "type": "join", "config": {"inputs": ["r0","r1"], "output": "findings", "mode": "collect_array"}},
    {"id": "synthesize", "type": "llm", "prompt": "Synthesize: {input}", "config": {"input_key": "findings", "output_key": "report"}}
  ],
  "edges": [
    {"from": "coordinator", "to": "fanout"},
    {"from": "fanout", "to": "analyst_0"}, {"from": "fanout", "to": "analyst_1"},
    {"from": "analyst_0", "to": "join"}, {"from": "analyst_1", "to": "join"},
    {"from": "join", "to": "synthesize"}, {"from": "synthesize", "to": "END"}
  ],
  "reducers": {"findings": "append"}
}
```

## Built-in templates

| Template | Description | Use case |
|----------|-------------|----------|
| `council_deliberation` | 3-analyst parallel: coordinator → fanout → 3 researchers → join → synthesize | Complex decision-making |
| `parallel_council` | 2-person debate: optimist vs skeptic → join → judge | Perspective contrast |
| `plan_critique_refine` | plan → critique → refine | Code generation, writing |
| `analysis_pipeline` | planner → researcher → extractor → synthesizer → validator with correction loop | Document processing |
| `classifier_router` | LLM classifier routes to bug/feature/question handlers | Intent detection, triage |

## JSON-RPC smoke test

```bash
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | \
  npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model glm-5.2:cloud 2>/dev/null | \
  python3 -c "import sys,json; msg=json.loads(sys.stdin.read()); print(f'{len(msg[\"result\"][\"tools\"])} tools')"
# Expected: 25 tools
```

## Capability boundary

- **Runs** are process-local and `volatile` while active. With `--data-dir`, terminal projections and explicit pre-execution checkpoints persist to SQLite; uncheckpointed active rows become `interrupted_non_resumable` after restart
- **Cancellation** drops the local provider future (best-effort); the underlying request may continue
- **Checkpoint/resume** is deterministic local only — `passthrough` + `state_transform` chains with SQLite-bound state. LLM, router, join, parallel, subgraph nodes excluded from resume
- **Witness capture** stores caller-supplied content only; locators are never fetched, authority never asserted
- **Receipts** use HMAC-SHA256; they prove structural execution, not external model calls
- **Integrity key** required for checkpoint/resume, approvals, receipts, and witnesses. Without `AGENT_GRAPH_INTEGRITY_KEY_PATH`, these operations fail closed with `INTEGRITY_KEY_REQUIRED`

## Ecosystem

| Crate | Description | Version |
|-------|-------------|---------|
| [agent-graph-mcp](https://crates.io/crates/agent-graph-mcp) | MCP server (this crate) | v0.2.4 |
| [ri-agent-graph](https://crates.io/crates/ri-agent-graph) | Core graph execution engine | v0.2.2 |
| [llm-pipeline](https://crates.io/crates/llm-pipeline) | Reusable LLM node payloads | v0.2.0 |
| [stack-ids](https://crates.io/crates/stack-ids) | Trace/identity primitives | v0.1.3 |

## Verification

```bash
cargo build --release
cargo test --lib --test daemon_recovery --test mcp_integration   # 116 tests
cargo clippy -- -D warnings
cargo fmt --check
```

## Roadmap

- [ ] Generic replay for non-deterministic node types
- [ ] Subgraph composition with isolated state
- [ ] Dynamic parallel branch count (`map_reduce`)
- [ ] Operator authority subsystem for authenticated HITL
- [ ] External tool integration (shell, filesystem, HTTP)
- [ ] WebAssembly target for the proxy

## Contributing

PRs welcome. See the [agent-graph-mcp repo](https://github.com/RecursiveIntell/agent-graph-mcp) for source and issues.

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).

---

Built by [RecursiveIntell](https://github.com/RecursiveIntell) — an applied R&D studio building local-first AI infrastructure.
