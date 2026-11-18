# agent-graph-mcp

**Compose multi-step LLM workflows with parallel fan-out subject to graph limits; Codex App Server worker processes are subject to their configured process-admission limit.** This MCP server exposes `ri-agent-graph` for declarative graphs with typed joins. With daemon SQLite storage and an integrity key configured, checkpoint/resume is available only for eligible deterministic-local linear `passthrough`/`state_transform` chains. Durable checkpoint approvals require the same key; approval decisions are not exposed through the model-facing MCP tools.

[![Crates.io](https://img.shields.io/crates/v/agent-graph-mcp)](https://crates.io/crates/agent-graph-mcp)
[![docs.rs](https://img.shields.io/docsrs/agent-graph-mcp)](https://docs.rs/agent-graph-mcp)
[![MCP Badge](https://lobehub.com/badge/mcp-full/recursiveintell-agent-graph-mcp?theme=light)](https://lobehub.com/mcp/recursiveintell-agent-graph-mcp)
[![npm](https://img.shields.io/npm/v/@recursiveintell/agent-graph-mcp)](https://www.npmjs.com/package/@recursiveintell/agent-graph-mcp)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

![Architecture diagram showing MCP client connecting via stdin/stdout to the agent-graph-mcp proxy, which communicates over Unix socket to the agent-graph-mcpd daemon backed by SQLite](assets/architecture.svg)

> **Expose the `ri-agent-graph` runtime engine over MCP.** Compile declarative JSON workflow specs and run graphs synchronously or with the separate asynchronous run-start tools. With daemon SQLite storage and an integrity key configured, eligible deterministic-local linear `passthrough`/`state_transform` chains can be checkpointed and resumed; approval decisions are not exposed through the model-facing MCP tools. Source witnesses can be captured; durable SQLite terminal receipts are HMAC-authenticated when an integrity key is configured.

## Who is this for?

**AI agent operators** who need multi-node LLM orchestration (parallel research sweeps, council deliberation, plan→critique→refine pipelines) through their existing MCP client (Hermes Agent, Claude Desktop, Cursor). **Not for** simple single-call LLM usage — use a direct provider integration for that.

## Quick start

### Prerequisites

- An LLM endpoint (local Ollama, or any OpenAI-compatible API)
- Node.js ≥ 18 (for npx) or Rust ≥ 1.75 (for cargo install)
- A model supported by your configured endpoint. Examples below use `llama3.2:3b` (pull with `ollama pull llama3.2:3b`); replace `--model` with a supported model.

### npx (recommended)

```bash
npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
```

> **Note:** `--direct` is deprecated. Prefer daemon mode (below) for persistence and multi-client support. Direct mode still works but will be removed in a future release.

**Expected output:** MCP initialization handshake. Run `tools/list` to inspect the tool list for the installed version.

### Cargo install

```bash
cargo install agent-graph-mcp --locked
agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b
```

### Daemon mode (multi-client, persistent state)

```bash
# Start the daemon (all defaults shown explicitly)
agent-graph-mcpd \
  --data-dir ~/.local/share/agent-graph \
  --socket /tmp/agent-graph/mcp.sock \
  --base-url http://127.0.0.1:11434 \
  --model llama3.2:3b \
  --max-graphs 256 &

# Connect proxy
agent-graph-mcp --socket /tmp/agent-graph/mcp.sock
```

**Defaults** (applied when flags are omitted):

| Flag | Default | Notes |
|------|---------|-------|
| `--data-dir` | `/tmp/agent-graph` | Ephemeral across reboots. Set explicitly for durable storage |
| `--socket` | `/tmp/agent-graph/mcp.sock` | Must match between daemon and proxy |
| `--base-url` | `http://127.0.0.1:11434` | Ollama default. Change for any provider |
| `--model` | `glm-5.2:cloud` | **Always override this.** The default exists only for backward compatibility |
| `--max-graphs` | 64 | Range 1–1024. Raise for large graph libraries |

`--max-graphs` is a per-daemon registration capacity. `graph_status` reports both the effective limit and `capacity_state`; an `over_limit_legacy` state preserves existing durable graphs but rejects new registrations until the configured limit is raised or registrations are retired.

### Direct vs daemon

| Mode | Use when |
|------|----------|
| `--direct` | Single MCP client, no persistence needed, simplest setup |
| `--socket` (daemon) | Multiple clients, durable graph storage, long-running workflows, checkpoint-bound approval requests and metadata (not decision submission) |

## Provider and model configuration

Graphs containing LLM nodes send calls to an OpenAI-compatible endpoint. You control **where** (`--base-url`) and **which model** (`--model`). The API key flows through the `OPENAI_API_KEY` environment variable.

### Codex App Server mode

When `--base-url codex-app-server://` is selected, the Rust daemon uses Codex App Server workers. `AGENT_GRAPH_CODEX_MAX_PROCESSES` controls process admission: the Rust default is 3, clamped at 32; the launcher script sets 9 unless overridden. At process limit one, it reuses a persistent worker over a loopback WebSocket across graph nodes; higher configured limits admit bounded one-shot workers over stdio.

The integration is bounded before launch and during streaming:

- `scripts/launch-codex-daemon.sh` discovers enabled Codex MCP servers with
  plugins and apps disabled, then passes the list to workers that disable those
  servers for prompt-only graph turns;
- each completed graph thread is deleted before the worker is reused, preventing
  the long-lived connection from retaining auto-subscribed thread state;
- prompt input capped at 256 KiB;
- model reasoning effort is configurable with `AGENT_GRAPH_CODEX_REASONING_EFFORT`
  (default: `high`; accepted values: `minimal`, `low`, `medium`, `high`, `xhigh`);
- each JSON-RPC line and WebSocket message is capped at 4 MiB;
- streamed assistant output capped at 256 KiB;
- stderr retained as an 8 KiB tail and redacted before MCP errors;
- stdio-only Codex-compatible test executables may use the legacy one-shot compatibility path.

The graph's `max_parallelism` controls graph scheduling. For Codex App Server, the configured process-admission limit bounds Codex worker processes: one persistent worker serializes turns at limit one, while higher limits admit bounded one-shot workers. These are separate controls; graph fan-out does not establish provider-side concurrency.

### Setting your API key

All OpenAI-compatible providers use the same variable — `OPENAI_API_KEY`. Set it one of these ways:

```bash
# Inline (visible in process list — use only for testing)
OPENAI_API_KEY=sk-... agent-graph-mcpd --base-url https://api.deepseek.com/v1 --model deepseek-v4-pro

# Export (session-only)
export OPENAI_API_KEY=sk-...
agent-graph-mcpd --base-url https://api.deepseek.com/v1 --model deepseek-v4-pro

# systemd unit (persistent, recommended)
# Add to ~/.config/systemd/user/agent-graph-mcpd.service:
#   [Service]
#   Environment=OPENAI_API_KEY=sk-...
systemctl --user daemon-reload
systemctl --user restart agent-graph-mcpd
```

For convenience, the daemon also accepts `AGENT_GRAPH_API_KEY` — if set, it takes precedence over `OPENAI_API_KEY`. Use this when you want to isolate the agent-graph key from other tools that read `OPENAI_API_KEY`.

**Security note:** Never commit API keys to git, dotfiles, or shell history. Use a secrets manager (1Password CLI, `pass`, systemd `LoadCredential=`) or set the environment at boot.

### Provider-specific keys

| Provider | Key format | Where to get it |
|----------|-----------|-----------------|
| DeepSeek | `sk-...` | [platform.deepseek.com/api_keys](https://platform.deepseek.com/api_keys) |
| OpenAI | `sk-proj-...` or `sk-...` | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) |
| OpenRouter | `sk-or-v1-...` | [openrouter.ai/keys](https://openrouter.ai/keys) |
| Alibaba MaaS | `sk-...` | Alibaba Cloud console → Model Studio → API Keys |
| Ollama | None required | Local, no key needed |

All providers use the same env var name. Just set `OPENAI_API_KEY` to the key for whichever provider you're pointing `--base-url` at.

### Daemon mode (persistent)

```bash
# Ollama (local)
agent-graph-mcpd --base-url http://127.0.0.1:11434 --model llama3.2:3b

# OpenAI
OPENAI_API_KEY=sk-... agent-graph-mcpd --base-url https://api.openai.com/v1 --model gpt-4o

# DeepSeek
OPENAI_API_KEY=sk-... agent-graph-mcpd --base-url https://api.deepseek.com/v1 --model deepseek-v4-pro

# OpenRouter (a model supported by your configured endpoint)
OPENAI_API_KEY=sk-or-... agent-graph-mcpd --base-url https://openrouter.ai/api/v1 --model openai/gpt-4o

# Alibaba MaaS (note: has gateway body-size limits — large councils may need trimming)
OPENAI_API_KEY=sk-... agent-graph-mcpd --base-url https://llm-<id>.ap-southeast-1.maas.aliyuncs.com/compatible-mode --model deepseek-v4-flash
```

### Direct mode (ephemeral, same flags)

```bash
npx -y @recursiveintell/agent-graph-mcp --direct --base-url https://api.deepseek.com/v1 --model deepseek-v4-pro
```

### Provider examples and validation boundary

| Provider | Configuration path | Check before relying on it |
|----------|--------------------|----------------------------|
| Ollama (local) | Local endpoint, no API key required by Ollama | Confirm the model is installed and the endpoint responds |
| DeepSeek, OpenRouter, OpenAI, Alibaba MaaS | OpenAI-compatible endpoint plus the provider's key | Confirm the current URL, model, quota, request limits, and response behavior with that provider |

These are configuration examples, not current live-provider test results. The earlier provider-uptime, latency, model-availability, and gateway-size assertions are not established by a source inspection or this README change.

**Choosing a model for agent-graph:** An LLM node uses the daemon's `--model` unless its top-level `model` field supplies an override.  The compiler forwards that field to `LlmNode`, which selects it before the default model.  This source supports per-node model selection; availability still depends on the configured provider.  Graph fan-out limits and Codex App Server worker bounds remain separate from model selection.

## Complete CLI reference

### Daemon (`agent-graph-mcpd`)

| Flag | Default | Description |
|------|---------|-------------|
| `--data-dir PATH` | `/tmp/agent-graph` | Persistent storage. Set for durable graphs across restarts |
| `--socket PATH` | `/tmp/agent-graph/mcp.sock` | Unix socket for proxy connections |
| `--base-url URL` | `http://127.0.0.1:11434` | OpenAI-compatible LLM endpoint |
| `--model NAME` | `glm-5.2:cloud` | Default model for all LLM nodes. **Override this** |
| `--max-graphs N` | 64 | Graph registration capacity (1–1024) |
| `--help` | | Print usage |
| `--version` | | Print version |

### Proxy (`agent-graph-mcp`)

| Flag | Default | Description |
|------|---------|-------------|
| `--socket PATH` | `$XDG_RUNTIME_DIR/agent-graph/mcp.sock` or `/tmp/agent-graph/mcp.sock` | Daemon socket to connect to |
| `--connect-timeout-ms N` | 2000 | Connection timeout in milliseconds |
| `--version` | | Print version |

### Direct mode (`agent-graph-mcp --direct`) — deprecated

Prefers daemon mode. All daemon flags above plus:

| Flag | Description |
|------|-------------|
| `--integrity-key PATH` | HMAC key file for receipt authentication |
| `--require-integrity-key` | Refuse startup if integrity key is missing/unreadable (requires `--data-dir`) |
| `--ephemeral` | In-memory only, no persistence (mutually exclusive with `--data-dir`) |

### Environment variables

| Variable | Used by | Description |
|----------|---------|-------------|
| `OPENAI_API_KEY` | Daemon, direct | API key for the LLM provider. Works for all OpenAI-compatible endpoints (DeepSeek, OpenAI, OpenRouter, etc.) |
| `AGENT_GRAPH_API_KEY` | Daemon, direct | Optional. Takes precedence over `OPENAI_API_KEY`. Use to isolate the agent-graph key from other tools |
| `AGENT_GRAPH_INTEGRITY_KEY_PATH` | Daemon, direct | Alternative to `--integrity-key` for receipt signing |
| `RUST_LOG` | Both | Log level. Set to `debug` for verbose output |

## Client configs

**Hermes Agent:**
```yaml
mcp_servers:
  agent_graph:
    command: agent-graph-mcp
    args: [--socket, /tmp/agent-graph/mcp.sock]
```

**Claude Desktop:**
```json
{"mcpServers": {"agent-graph": {"command": "npx", "args": ["-y", "@recursiveintell/agent-graph-mcp", "--direct", "--base-url", "http://127.0.0.1:11434", "--model", "llama3.2:3b"]}}}
```

### Try it out

Once configured, your agent can use the available tools directly. Try these natural language prompts:

> "Use the `council_deliberation` template to debate the merits of Rust vs. Go for systems programming."

> "Create a graph with 3 parallel research nodes analyzing web framework tradeoffs, join the results, and produce a ranked recommendation."

> "Inspect the deterministic-local checkpoint and approval boundary before creating a workflow that needs human approval."

## Eight-way research sweep

Fan out to eight LLM nodes in parallel, then join results into a ninth LLM call for synthesis:

```json
{
  "name": "9-agent-research-sweep",
  "entry": "fanout",
  "output_key": "report",
  "nodes": [
    {"id":"fanout","type":"passthrough"},
    {"id":"agent_0","type":"llm","prompt":"Research topic A: {input}","config":{"output_key":"agent_0"}},
    {"id":"agent_1","type":"llm","prompt":"Research topic B: {input}","config":{"output_key":"agent_1"}},
    {"id":"agent_2","type":"llm","prompt":"Research topic C: {input}","config":{"output_key":"agent_2"}},
    {"id":"agent_3","type":"llm","prompt":"Analyze dimension 1: {input}","config":{"output_key":"agent_3"}},
    {"id":"agent_4","type":"llm","prompt":"Analyze dimension 2: {input}","config":{"output_key":"agent_4"}},
    {"id":"agent_5","type":"llm","prompt":"Analyze dimension 3: {input}","config":{"output_key":"agent_5"}},
    {"id":"agent_6","type":"llm","prompt":"Critique from angle X: {input}","config":{"output_key":"agent_6"}},
    {"id":"agent_7","type":"llm","prompt":"Critique from angle Y: {input}","config":{"output_key":"agent_7"}},
    {"id":"join","type":"join","config":{"inputs":["agent_0","agent_1","agent_2","agent_3","agent_4","agent_5","agent_6","agent_7"],"output":"collected","mode":"collect_array"}},
    {"id":"report","type":"llm","prompt":"Synthesize findings from all agents and produce final report: {collected}","config":{"output_key":"report"}}
  ],
  "edges": [
    {"from":"fanout","to":"agent_0"},
    {"from":"fanout","to":"agent_1"},
    {"from":"fanout","to":"agent_2"},
    {"from":"fanout","to":"agent_3"},
    {"from":"fanout","to":"agent_4"},
    {"from":"fanout","to":"agent_5"},
    {"from":"fanout","to":"agent_6"},
    {"from":"fanout","to":"agent_7"},
    {"from":"agent_0","to":"join"},
    {"from":"agent_1","to":"join"},
    {"from":"agent_2","to":"join"},
    {"from":"agent_3","to":"join"},
    {"from":"agent_4","to":"join"},
    {"from":"agent_5","to":"join"},
    {"from":"agent_6","to":"join"},
    {"from":"agent_7","to":"join"},
    {"from":"join","to":"report"},
    {"from":"report","to":"END"}
  ],
  "max_parallelism": 9
}
```

The eight `agent_0` through `agent_7` branches are scheduled in parallel subject to graph limits. In Codex App Server mode, worker-process admission is a separate bound; neither establishes provider-side concurrency. Each writes its own state key, the join collects those eight results into `{collected}`, and the report LLM call runs afterward. The top-level `output_key` selects the final report. This graph has nine LLM calls in total: eight research nodes on parallel graph branches and one report node that runs after the branch results. This describes graph scheduling, not provider-side concurrency.

## Loop and subgraph nodes

- **`loop`** — bounded iteration. Config: `{"entry": "<body node>", "exit": "<node>|END", "max_iterations": 1..=32}`. The loop node re-enters `entry` (body should edge back to the loop node to sustain the cycle) and navigates to `exit` once the iteration budget is exhausted. The iteration counter lives in `__loop__:<loop node id>` and can be read by the body via `input_key`. Graph-level `max_iterations` is the outer safety net.
- **`subgraph`** — execute another registered graph. Config: `{"graph_name": "<registered graph>", "input_key": "__input__", "output_key": "__subgraph_output__"}`. The referenced graph runs in-process with the parent's budget counters and cancellation; its terminal output is written under `output_key`. Nesting depth limit: 4.
- **`human_approval`** — a graph-schema node. Its presence does not establish an end-to-end durable operator-decision/resume path: the model-facing approval tools expose checkpoint-bound request/list/get operations, not decision submission. It does not itself authorize external actions.

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

![Conceptual HITL approval workflow; operator-decision and resume steps are illustrative only and are not exposed as a model-facing decision flow in this snapshot](assets/hitl-workflow.svg)

With daemon SQLite storage and an integrity key configured, checkpoint-bound approval requests and metadata can be stored and inspected. The current model-facing tools expose request/list/get operations but do not submit approval decisions. Checkpoint/resume is limited to eligible deterministic-local linear `passthrough` and `state_transform` chains; LLM, router, join, parallel, loop, subgraph and `human_approval` nodes are outside that resume subset. The presence of a `human_approval` node does not establish a durable operator-decision/resume path or permission to execute an external action.

## Tools

This list describes the source router, including run triage and paginated verified-artifact reads. Published npm/Cargo packages can lag source changes; check the running server's `tools/list` rather than assuming a package with the same version number includes every later commit.

**Graph lifecycle:** `graph_create`, `graph_list`, `graph_inspect`, `graph_render`
**Execution:** `graph_execute`, `graph_run_start`, `graph_run_wait`, `graph_run_cancel`, `graph_run_get`, `graph_run_triage`
**State & checkpoint:** `graph_run_state`, `graph_run_events`, `graph_run_checkpoint`, `graph_run_resume`
**HITL approval:** `graph_approval_list`, `graph_approval_get`, `graph_approval_request`
**Evidence:** `graph_source_witness_capture`, `graph_source_witness_get`, `graph_run_artifact`
**Templates:** `graph_template_list`, `graph_template_instantiate`, `graph_template_candidates`, `graph_template_outcomes`
**Receipts & status:** `graph_policy_check`, `graph_run_receipt`, `graph_status`
**Retention:** `graph_retention_review`, `graph_retention_set`

## Ecosystem

| Crate | Role | Version |
|-------|------|---------|
| [agent-graph-mcp](https://crates.io/crates/agent-graph-mcp) | MCP server (this repo) | 0.3.0 |
| [ri-agent-graph](https://crates.io/crates/ri-agent-graph) | Core graph engine | 0.2 |
| [llm-pipeline](https://crates.io/crates/llm-pipeline) | LLM node payloads + retry | 0.2 |
| [stack-ids](https://crates.io/crates/stack-ids) | Trace primitives (TraceCtx, AttemptId) | 0.1 |

## Verification

```bash
# Smoke test — inspect tools exposed by the installed package
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | \
  npx -y @recursiveintell/agent-graph-mcp --direct --base-url http://127.0.0.1:11434 --model llama3.2:3b 2>/dev/null | \
  python3 -c "import sys,json; msg=json.loads(sys.stdin.read()); print(f'{len(msg[\"result\"][\"tools\"])} tools')"
# Compare the returned tool names with the installed package revision

# Build and test suite
cargo build --release
cargo test --lib --test daemon_recovery --test mcp_integration
```

**Test evidence boundary:** No GitHub Actions workflows are present under `.github/workflows/`; other CI systems and remote checks are not assessed here. The commands above are suggested local validation, not a report that those commands have passed for a particular checkout. Check the exact source revision and current remote checks separately before relying on test evidence.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `tools/list` returns 0 or errors | MCP relay or daemon not running | Verify daemon: `agent-graph-mcpd --data-dir ... &`. Check `graph_status` |
| LLM nodes hang | Provider unreachable or model name wrong | Test endpoint: `curl <your-base-url>/models` (Ollama) or `curl -H "Authorization: Bearer $OPENAI_API_KEY" <your-base-url>/models` (cloud) |
| `graph_run_start` returns immediately | Run is async by default | Use `graph_run_wait` to block on completion, or `graph_execute` for sync |
| "socket not found" | Daemon not started or socket path mismatch | Ensure `--socket` matches between daemon and proxy. Default is `/tmp/agent-graph/mcp.sock` |
| Approval stuck | Deterministic checkpoint approval is pending | Inspect `graph_approval_list`/`graph_approval_get`; approval decisions are not exposed through the model-facing MCP tools |
| Execution hangs silently | Logging too quiet | Run daemon with `RUST_LOG=debug agent-graph-mcpd ...` — logs to stderr. For `--direct` mode, add `RUST_LOG=debug` before the command |
| Unknown model errors | Default model is `glm-5.2:cloud` | Always pass `--model`. The default exists for backward compatibility and won't exist on your endpoint |

## Status and limitations

- **Source version:** The checked-in `Cargo.toml` declares `agent-graph-mcp` 0.3.0. Check crates.io and npm independently for published versions and package contents; a source version does not certify either release.
- **Platforms:** The source and examples target Unix-socket operation. This review did not run Linux, macOS, or Windows installation and runtime checks.
- **CI boundary:** this source snapshot has no GitHub Actions workflows under `.github/workflows/`; other CI systems and remote checks are not assessed here.
- **Durable execution** requires the daemon. Direct mode is ephemeral.
- **Graph-level parallelism:** configurable from 1 through 100 per graph (default 8). This is a graph scheduling bound, not a claim about provider-side concurrency; Codex worker admission limits Codex worker processes.
- **LLM providers:** OpenAI-compatible transport is implemented. Check each endpoint and model in the target environment; see [Provider and model configuration](#provider-and-model-configuration) for setup examples.

## Support, security, and contributing

- **Issues and discussions:** [GitHub Issues](https://github.com/RecursiveIntell/agent-graph-mcp/issues)
- **Security:** [SECURITY.md](SECURITY.md) — report vulnerabilities privately via GitHub Security Advisories
- **Contributing:** [CONTRIBUTING.md](CONTRIBUTING.md) — setup, workflow, code style, and PR expectations
- **Code of Conduct:** [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — Contributor Covenant 2.1
- **AI agents:** [AGENTS.md](AGENTS.md) — project structure, conventions, and constraints for coding agents

## License

MIT © [RecursiveIntell](https://github.com/RecursiveIntell)
