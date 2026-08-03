# agent-graph-mcp

**Run 9 agents at once.** MCP server for graph-orchestrated LLM workflows — dispatch up to 16 LLM nodes in parallel fan-out with typed joins, checkpoint/resume, human-in-the-loop approvals, and HMAC-authenticated execution receipts. 25 typed tools.

[![Crates.io](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
[![docs.rs](https://img.shields.io/docsrs/agent-graph-mcp)](https://docs.rs/agent-graph-mcp)
[![MCP Badge](https://lobehub.com/badge/mcp-full/recursiveintell-agent-graph-mcp?theme=light)](https://lobehub.com/mcp/recursiveintell-agent-graph-mcp)
[![npm](https://img.shields.io/npm/v/@recursiveintell/agent-graph-mcp)](https://www.npmjs.com/package/@recursiveintell/agent-graph-mcp)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

![Architecture diagram showing MCP client connecting via stdin/stdout to the agent-graph-mcp proxy, which communicates over Unix socket to the agent-graph-mcpd daemon backed by SQLite](assets/architecture.svg)

> **Expose the `ri-agent-graph` runtime engine over MCP.** Compile declarative JSON workflow specs, execute synchronously or asynchronously, checkpoint/resume, request human approval, capture source witnesses, and get cryptographic receipts — all through 25 typed MCP tools.

## Who is this for?

**AI agent operators** who need multi-node LLM orchestration (parallel research sweeps, council deliberation, plan→critique→refine pipelines) through their existing MCP client (Hermes Agent, Claude Desktop, Cursor). **Not for** simple single-call LLM usage — use a direct provider integration for that.

## Quick start

### Prerequisites

- An LLM endpoint (local Ollama, or any OpenAI-compatible API)
- Node.js ≥ 18 (for npx) or Rust ≥ 1.75 (for cargo install)
- A model available at your endpoint. Examples below use `llama3.2:3b` (pull with `ollama pull llama3.2:3b`). Any model works — just replace `--model`.

### npx (recommended)

```bash
npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
```

**Expected output:** MCP initialization handshake. Run `tools/list` to verify you see 25 tools.

### Cargo install

```bash
cargo install agent-graph-mcp --locked
agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
```

### Daemon mode (multi-client, persistent state)

```bash
agent-graph-mcpd --data-dir ~/.local/share/agent-graph --socket /tmp/agent-graph.sock --max-graphs 256 &
agent-graph-mcp --socket /tmp/agent-graph.sock
```

`--max-graphs` is a per-daemon registration capacity. It defaults to 64 and accepts values from 1 through 1024. Set it explicitly for a durable store whose registered graph count exceeds the historical default. `graph_status` reports both the effective limit and `capacity_state`; an `over_limit_legacy` state preserves existing durable graphs but rejects new registrations until the configured limit is raised or registrations are retired.

### Direct vs daemon

| Mode | Use when |
|------|----------|
| `--direct` | Single MCP client, no persistence needed, simplest setup |
| `--socket` (daemon) | Multiple clients, durable graph storage, long-running workflows, HITL approvals |

## Client configs

**Hermes Agent:**
```yaml
mcp_servers:
  agent_graph:
    command: agent-graph-mcp
    args: [--socket, /tmp/agent-graph.sock]
```

**Claude Desktop:**
```json
{"mcpServers": {"agent-graph": {"command": "npx", "args": ["-y", "@recursiveintell/agent-graph-mcp", "--direct", "--base-url", "http://127.0.0.1:11434", "--model", "llama3.2:3b"]}}}
```

### Try it out

Once configured, your agent can use any of the 25 tools directly. Try these natural language prompts:

> "Use the `council_deliberation` template to debate the merits of Rust vs. Go for systems programming."

> "Create a graph with 3 parallel research nodes analyzing web framework tradeoffs, join the results, and produce a ranked recommendation."

> "Spin up a plan→critique→refine pipeline for a database migration strategy, and pause for my approval before the final report."

## 9 agents at once

Fan out to 9 LLM nodes in parallel, then join results into one synthesis:

```json
{
  "name": "9-agent-research-sweep",
  "entry": "fanout",
  "nodes": [
    {"id": "fanout", "type": "passthrough"},
    {"id": "agent_0", "type": "llm", "prompt": "Research topic A: {input}"},
    {"id": "agent_1", "type": "llm", "prompt": "Research topic B: {input}"},
    {"id": "agent_2", "type": "llm", "prompt": "Research topic C: {input}"},
    {"id": "agent_3", "type": "llm", "prompt": "Analyze dimension 1: {input}"},
    {"id": "agent_4", "type": "llm", "prompt": "Analyze dimension 2: {input}"},
    {"id": "agent_5", "type": "llm", "prompt": "Analyze dimension 3: {input}"},
    {"id": "agent_6", "type": "llm", "prompt": "Critique from angle X: {input}"},
    {"id": "agent_7", "type": "llm", "prompt": "Critique from angle Y: {input}"},
    {"id": "join", "type": "join", "config": {"inputs": ["agent_0","agent_1","agent_2","agent_3","agent_4","agent_5","agent_6","agent_7"], "output": "collected", "mode": "collect_array"}},
    {"id": "report", "type": "llm", "prompt": "Synthesize findings from all agents and produce final report: {collected}"}
  ],
  "edges": [
    {"from": "fanout", "to": "agent_0"}, {"from": "fanout", "to": "agent_1"},
    {"from": "fanout", "to": "agent_2"}, {"from": "fanout", "to": "agent_3"},
    {"from": "fanout", "to": "agent_4"}, {"from": "fanout", "to": "agent_5"},
    {"from": "fanout", "to": "agent_6"}, {"from": "fanout", "to": "agent_7"},
    {"from": "agent_0", "to": "join"}, {"from": "agent_1", "to": "join"},
    {"from": "agent_2", "to": "join"}, {"from": "agent_3", "to": "join"},
    {"from": "agent_4", "to": "join"}, {"from": "agent_5", "to": "join"},
    {"from": "agent_6", "to": "join"}, {"from": "agent_7", "to": "join"},
    {"from": "join", "to": "report"}, {"from": "report", "to": "END"}
  ],
  "max_parallelism": 9
}
```

All 9 LLM calls execute concurrently via Tokio `JoinSet`. The join node collects results from agents 0-7 into `{collected}`, then the report node synthesizes. Scale up to 16 branches per parallel node.

## Built-in templates

| Template | Description |
|----------|-------------|
| `council_deliberation` | 3-analyst parallel council with synthesis |
| `parallel_council` | 2-person debate with cross-examination |
| `plan_critique_refine` | plan → critique → refine pipeline |
| `analysis_pipeline` | planner → researcher → extractor → synthesizer → validator |
| `classifier_router` | LLM classifier → bug/feature/question handlers |

Templates are instantiated with `graph_template_instantiate` — no JSON authoring required.

## Architecture

```
MCP Client ──→ agent-graph-mcp (proxy) ──Unix socket──→ agent-graph-mcpd (daemon) ──→ SQLite
               stdin/stdout                 framed              Tokio async I/O
```

![HITL approval workflow diagram showing agent execution pausing at approval checkpoints, the human making a decision, and the agent resuming with the approved state](assets/hitl-workflow.svg)

Human-in-the-loop approvals are backed by durable SQLite checkpoints. When a graph reaches an approval node, execution pauses, a checkpoint is persisted, and the approval is surfaced via `graph_approval_list`. The human reviews and decides; the graph resumes from the checkpoint.

## Tools (25)

**Graph lifecycle (4):** `graph_create`, `graph_list`, `graph_inspect`, `graph_render`
**Execution (5):** `graph_execute`, `graph_run_start`, `graph_run_wait`, `graph_run_cancel`, `graph_run_get`
**State & checkpoint (4):** `graph_run_state`, `graph_run_events`, `graph_run_checkpoint`, `graph_run_resume`
**HITL approval (3):** `graph_approval_list`, `graph_approval_get`, `graph_approval_request`
**Evidence (2):** `graph_source_witness_capture`, `graph_source_witness_get`
**Templates (4):** `graph_template_list`, `graph_template_instantiate`, `graph_template_candidates`, `graph_template_outcomes`
**Receipts & status (3):** `graph_policy_check`, `graph_run_receipt`, `graph_status`

## Ecosystem

| Crate | Role | Version |
|-------|------|---------|
| [agent-graph-mcp](https://crates.io/crates/agent-graph-mcp) | MCP server (this repo) | 0.2.6 |
| [ri-agent-graph](https://crates.io/crates/ri-agent-graph) | Core graph engine | 0.2 |
| [llm-pipeline](https://crates.io/crates/llm-pipeline) | LLM node payloads + retry | 0.2 |
| [stack-ids](https://crates.io/crates/stack-ids) | Trace primitives (TraceCtx, AttemptId) | 0.1 |

## Verification

```bash
# Smoke test — verify 25 tools are exposed
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | \
  npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b 2>/dev/null | \
  python3 -c "import sys,json; msg=json.loads(sys.stdin.read()); print(f'{len(msg[\"result\"][\"tools\"])} tools')"
# Expected: 25 tools

# Build and test suite
cargo build --release
cargo test --lib --test daemon_recovery --test mcp_integration
```

**Test status (current `main`):** 57 lib tests pass, 1 known failure in `evidence::tests::witness_dependencies_verify_sqlite_content_and_span` (fixture dependency on semantic-memory-mcp binary path — tracked, does not affect runtime correctness). Integration tests (`daemon_recovery`, `mcp_integration`, `lifecycle`, `operator_authority`, etc.) pass.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `tools/list` returns 0 or errors | MCP relay or daemon not running | Verify daemon: `agent-graph-mcpd --data-dir ... &`. Check `graph_status` |
| LLM nodes hang | Provider unreachable or model name wrong | Test: `curl http://127.0.0.1:11434/api/tags` |
| `graph_run_start` returns immediately | Run is async by default | Use `graph_run_wait` to block on completion, or `graph_execute` for sync |
| "socket not found" | Daemon not started or socket path mismatch | Ensure `--socket` matches between daemon and client |
| Approval stuck | Human hasn't decided | Check `graph_approval_list`, use `graph_approval_request` with decision |
| Execution hangs silently | Logging too quiet | Run daemon with `RUST_LOG=debug agent-graph-mcpd ...` — logs to stderr. For `--direct` mode, add `RUST_LOG=debug` before the command |

## Status and limitations

- **Published:** crates.io + npm. Version 0.2.6.
- **Tested on:** Linux (Nobara/Fedora). macOS works via npx. Windows untested.
- **No CI currently configured.** All verification is local.
- **Durable execution** requires the daemon. Direct mode is ephemeral.
- **Max parallelism:** 16 nodes per parallel fan-out (compiler-enforced).
- **LLM providers:** any OpenAI-compatible endpoint. Tested primarily with Ollama and OpenRouter.
- **No AGENTS.md yet.** An AI-agent guidance file is planned — this will help coding agents navigate the 24-module Rust workspace.

## Support, security, and contributing

- **Issues and discussions:** [GitHub Issues](https://github.com/RecursiveIntell/agent-graph-mcp/issues)
- **Security:** For vulnerability reports, open a private security advisory on the repository. No separate SECURITY.md yet — this is a known gap.
- **Contributing:** Pull requests welcome. No formal CONTRIBUTING.md yet — open an issue to discuss before large changes.
- **Code of Conduct:** Not yet published. Standard open-source norms apply.

## License

MIT © [RecursiveIntell](https://github.com/RecursiveIntell)
