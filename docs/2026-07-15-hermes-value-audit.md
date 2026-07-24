# Agent-graph to Hermes capability audit and implementation specification

**Date:** 2026-07-15  
**Branch inspected:** `feat/full-integration`  
**Scope:** live source in `agent-graph/` and `agent-graph-mcp/`  
**Verdict:** `agent-graph-mcp` links `agent-graph`, but it does not execute an `AgentGraph`. It imports only `AgentState` and manually walks one node at a time. Consequently, most of the core runtime's practical value is absent at the Hermes boundary. The right correction is to make the MCP server a narrow, declarative compiler and run manager over `AgentGraph`, not to add graph semantics to Hermes core.

## 1. Audit method and claim labels

This is a source-first audit. Source references use the checked-out files and line numbers as of the date above. Existing test results supplied with the task are accepted as evidence: all `agent-graph` targets passed, and all 24 `agent-graph-mcp` tests passed with one dead-code warning. No long workspace test was run for this report.

Labels used throughout:

- **Live:** reached by a public execution path and backed by concrete implementation.
- **Partial:** executable, but important semantics, persistence, or evidence are incomplete.
- **Type-only:** a public type/trait exists but the runtime does not consume or populate it.
- **MCP-live:** reachable through the currently advertised MCP tools.
- **Proposed:** implementation work, not a current capability.

Memory recall suggested that core had checkpointing, interrupts, fan-out/fan-in, events, and receipts. Every material assertion below was rechecked against live source; memory is not treated as authority.

## 2. Verified current-state inventory

### 2.1 The current MCP execution path

The MCP binary advertises exactly `graph_create`, `graph_execute`, and `graph_status` (`agent-graph-mcp/src/main.rs:149-183`) and dispatches only those names (`agent-graph-mcp/src/main.rs:199-208`). Its `agent-graph` dependency is real (`agent-graph-mcp/Cargo.toml:11-16`), but the only imported core API is `agent_graph::state::AgentState` (`agent-graph-mcp/src/main.rs:4`). There is no construction or invocation of `AgentGraph`, `AgentGraphBuilder`, `GraphConfig`, `CheckpointStore`, `EventSink`, or receipt APIs.

Current graph definitions and execution summaries are process-local `HashMap`/`VecDeque` fields (`agent-graph-mcp/src/main.rs:118-124`). `graph_create` validates and inserts a spec keyed by `spec.name`, replacing an existing graph of the same name (`agent-graph-mcp/src/main.rs:231-247`). `graph_execute` creates a fresh Tokio runtime and blocks the single stdio request until completion (`agent-graph-mcp/src/main.rs:249-267`). `graph_status` reports names and counters, not addressable run records (`agent-graph-mcp/src/main.rs:282-291`). Process restart loses graphs and execution summaries.

The handwritten executor:

- creates fresh state and stores input under `__input__` (`agent-graph-mcp/src/main.rs:294-307`);
- builds local node/edge maps, then executes one `current_node` per loop (`agent-graph-mcp/src/main.rs:309-321`);
- rejects any revisit as a cycle, so configured loops cannot run (`agent-graph-mcp/src/main.rs:321-346`);
- uses `llm-pipeline::LlmCall` directly for LLM nodes (`agent-graph-mcp/src/main.rs:352-389`);
- routes by substring search over serialized JSON and iterates a `HashMap`, so overlapping route matches are nondeterministic (`agent-graph-mcp/src/main.rs:391-425`);
- follows only `targets[0]`, silently discarding additional outgoing edges (`agent-graph-mcp/src/main.rs:436-447`);
- returns only final `__input__`, step summaries, and an error string (`agent-graph-mcp/src/main.rs:450-458`).

The server does have useful input bounds: 64 graphs, 64 KiB graph specs, 128 nodes, 512 edges, recursion limit 64, 64 KiB inputs, 128 KiB outputs, and 100 retained summaries (`agent-graph-mcp/src/main.rs:10-18`), enforced in `validate_graph_spec` and serialization checks (`agent-graph-mcp/src/main.rs:462-537`). These are MCP-live and should be preserved or made configurable only within hard server caps.

The configured provider base URL and model are exposed in status (`agent-graph-mcp/src/main.rs:282-291`) and accepted from command-line arguments with defaults `http://127.0.0.1:11434` and `glm-5.2:cloud` (`agent-graph-mcp/src/main.rs:544-568`). There is no URL policy, host allowlist, TLS requirement, or secret-redaction layer.

### 2.2 Live core graph construction and scheduling

`AgentGraphBuilder` owns nodes, edges, iteration/cycle settings, retries, interrupt configuration, two checkpoint abstractions, reducers, graph name, event sink, and executor (`agent-graph/src/builder.rs:20-34`). It supports:

- nodes and per-node retry policies (`agent-graph/src/builder.rs:61-78`);
- nested `AgentGraph` values as nodes (`agent-graph/src/builder.rs:80-84`);
- multiple normal edges as fan-out and conditional edges via `RoutingFunction` (`agent-graph/src/builder.rs:86-110`);
- START/END entry and finish edges (`agent-graph/src/builder.rs:112-120`);
- max iterations, cycle toggle, reducers, interrupts before/after, checkpointers, checkpoint policy, event sinks, and custom executors (`agent-graph/src/builder.rs:122-186`).

The actual engine is superstep-based. It resolves START, caps iterations by both graph and run config, checks cancellation, and emits superstep events (`agent-graph/src/engine.rs:605-652`). A multi-node superstep forks state, uses a bounded `JoinSet`, and respects `GraphConfig.max_parallelism` (`agent-graph/src/engine.rs:709-750`). Results are sorted back into the original superstep order before reducers merge branch changes (`agent-graph/src/engine.rs:752-762`), giving deterministic branch merge order for a fixed graph. The next-node list is deduplicated in first-seen order (`agent-graph/src/engine.rs:883-889`).

Conditional routing can return one node, no node, or fan-out (`agent-graph/src/router.rs:8-15`, `agent-graph/src/engine.rs:1048-1064`). `Command` can apply state updates and navigate to one node, many nodes, END, dynamic `Send`, or normal edges (`agent-graph/src/command.rs:14-45`, `agent-graph/src/engine.rs:1033-1044`). However, `Navigation::Send` currently discards each `SendOp.state` and retains only its node name (`agent-graph/src/engine.rs:1042`); dynamic fan-out with per-branch state is therefore **type-only/miswired**, not a live capability.

Parallel state changes are merged key by key through registered reducers (`agent-graph/src/engine.rs:1067-1100`). Built-ins are last-write-wins, append, numeric add, deep object merge, and custom functions (`agent-graph/src/reducer.rs:4-132`). `JoinNode` is a separately explicit fan-in node that reads an ordered list of keys and either collects an array, shallow-merges objects, or runs a custom merge (`agent-graph/src/join.rs:14-80`); it errors when all inputs are null and writes a named output key (`agent-graph/src/join.rs:83-113`).

Loops are live through repeated scheduling when cycle detection is disabled or routing returns earlier nodes; termination is enforced by `min(config.recursion_limit, graph.max_iterations)` (`agent-graph/src/engine.rs:605-635`). The separate cycle check at `step_number > max_iter * 2` (`agent-graph/src/engine.rs:638-643`) cannot normally fire before the max-iteration check, so it should not be marketed as meaningful path-cycle detection.

Subgraphs are executable because `AgentGraph` implements `Node`: it forks parent state, finds an entry, executes the child, then copies all child state back (`agent-graph/src/graph.rs:378-417`). This is live but coarse-grained: checkpoint namespace inheritance, child-specific limits, and state input/output mapping are not explicit.

### 2.3 Runtime configuration, state, retries, cancellation, and events

`GraphConfig` carries thread ID, legacy and canonical trace context, recursion limit, parallelism, tags, metadata, and node-visible configurable values (`agent-graph/src/config.rs:5-38`). Defaults are recursion 100 and parallelism 8; the builder clamps parallelism to 1..32 (`agent-graph/src/config.rs:40-51`, `agent-graph/src/config.rs:90-92`). Canonical `stack_ids::TraceCtx` is generated or derived once per execution (`agent-graph/src/config.rs:95-118`, `agent-graph/src/engine.rs:42-64`).

`AgentState` provides typed and raw reads/writes, reducers, snapshots/history, restore/export, optimistic transactions, and deep forks (`agent-graph/src/state.rs:130-378`). `StateLimits` bounds keys, per-value serialized bytes, retained history, and lock wait; defaults are 10,000 keys, 1 MiB/value, 100 history entries, and five seconds (`agent-graph/src/state.rs:11-31`). Limits are checked on inserts and replacement (`agent-graph/src/state.rs:383-454`). There is no total-state byte cap in core, so MCP must add one.

Retry policies support attempt count, exponential backoff, maximum interval, jitter, and predicates (`agent-graph/src/retry.rs:9-39`). The engine applies retries to sequential and parallel branches and records individual attempts (`agent-graph/src/engine.rs:895-966`, `agent-graph/src/engine.rs:1330-1505`). Jitter derives from current time and thread ID (`agent-graph/src/retry.rs:108-123`), so retry timing is intentionally nondeterministic unless `jitter=false`.

`execute_cancellable` returns a task handle and atomic cancellation flag (`agent-graph/src/engine.rs:233-272`); cancellation is observed between supersteps (`agent-graph/src/engine.rs:625-628`), not while a node/LLM/tool call is blocked. `stream` returns a task and a bounded 256-event receiver (`agent-graph/src/engine.rs:274-317`). `GraphEvent` covers run/superstep/node lifecycle, tokens, state updates, interrupts, and checkpoints (`agent-graph/src/event_sink.rs:32-186`). Sinks must be nonblocking, and the channel adapter deliberately drops events when full (`agent-graph/src/event_sink.rs:197-260`).

`PayloadNode` is live for a core `Payload`, with state selectors and output mapping (`agent-graph/src/payload.rs:54-132`, `agent-graph/src/payload.rs:134-184`). Its current `PayloadContext` always has `on_token=None`, an empty run ID, and only the configured node name (`agent-graph/src/payload.rs:152-158`), despite comments saying the executor populates it. Token propagation through this adapter is therefore **not wired**. The MCP's LLM path does not use this core `PayloadNode` at all.

### 2.4 Checkpointing, interrupts, resume, receipts, and replay

There are two checkpoint rails:

1. `CheckpointSaver` stores superstep `Checkpoint` values by thread. `MemorySaver` is volatile; `SqliteSaver` is durable to a configured SQLite path (`agent-graph/src/checkpointer.rs:8-18`, `agent-graph/src/checkpointer.rs:21-60`, `agent-graph/src/checkpointer.rs:68-117`). `get_state`, history, and state update/time-travel use this rail (`agent-graph/src/graph.rs:224-259`).
2. `CheckpointStore` records runs, attempts, state snapshots, and interrupt fields (`agent-graph/src/checkpoint_store.rs:36-141`). Only `InMemoryCheckpointStore` ships in this crate (`agent-graph/src/checkpoint_store.rs:177-246`), so granular run/attempt history is not durable across process restart without a new implementation.

Checkpoint persistence policy is `required`, `best_effort`, or `disabled` (`agent-graph/src/graph.rs:32-43`). The engine checkpoints interrupts and every completed superstep when configured (`agent-graph/src/engine.rs:654-705`, `agent-graph/src/engine.rs:799-871`). Static interrupt-before/after is live. Nodes can also raise `AgentGraphError::InterruptError`, but `CheckpointStore::record_interrupt` exists without any engine call site (`agent-graph/src/checkpoint_store.rs:112-117`; repository search finds only its implementation/tests), and the separate `NodeOutcome`/`Interrupt` return model is not consumed by `Node::execute`, which returns `NodeOutput` (`agent-graph/src/outcome.rs:6-69`, `agent-graph/src/node.rs:10-27`). Dynamic typed interrupts are therefore partial/type-only.

`execute_with_interrupt` converts an interrupt error into a returned state and `InterruptCheckpoint` with a graph hash (`agent-graph/src/engine.rs:178-230`). `resume` rejects a changed semantic graph digest, while `resume_force` bypasses it (`agent-graph/src/graph.rs:189-222`). Resume starts from `checkpoint.resume_node`; it does not restore iteration, active parallel nodes, or honor `resume_before`, and there is no first-class injected resume value. Callers can mutate state before resume, but this is not equivalent to a durable `interrupt()` continuation. This is **partial resume**, not Temporal/LangGraph-style exact continuation.

Receipts and replay have three distinct truth levels:

- `execute_with_receipt` is live but emits one synthetic whole-run step, not one receipt per executed node (`agent-graph/src/engine.rs:78-175`). Tool calls and memory generations are empty.
- `record_run_bundle` records per-node state transitions, including deterministic branch order (`agent-graph/src/engine.rs:319-395`, `agent-graph/src/engine.rs:764-791`, `agent-graph/src/engine.rs:968-983`). However, model/tool/memory envelopes are always empty, and it captures **all process environment variables** (`agent-graph/src/engine.rs:383-393`), which is an unacceptable secret-leak risk at the MCP edge.
- `verify_replay` is offline integrity verification over graph digest, state-delta chain, envelope digests, and terminal receipt; it does not re-execute nodes or prove external calls happened (`agent-graph/src/engine.rs:397-493`). Envelope types exist (`agent-graph/src/receipt.rs:44-123`) but no live producer populates them. `GraphMemoryRetriever` is a standalone trait with no engine integration (`agent-graph/src/memory.rs:1-34`).

Accordingly, current core replay proves internal consistency of a supplied bundle, not causal provenance, faithful external-call replay, or crash-proof durable execution.

## 3. Exhaustive capability matrix

| Capability | Core status and evidence | MCP exposure | Gap | Hermes value | Implementation mechanism | Principal risk |
|---|---|---|---|---|---|---|
| Sequential graph | Live: `execute_with_config` and supersteps (`engine.rs:28-76`, `605-717`) | MCP-live via separate loop | MCP bypasses core | Reusable multi-stage agent workflows | Compile spec to `AgentGraph`; invoke core | Migration output differences |
| Parallel fan-out | Live: multiple edges/router/command + bounded `JoinSet` (`builder.rs:86-110`, `engine.rs:717-750`) | No; first edge only (`main.rs:436-447`) | No concurrency or fan-out | Parallel research, council, verification | Multiple normal edges or ordered router targets | Provider load, partial branch failure |
| Deterministic fan-in | Live for fixed branch order + reducers (`engine.rs:752-762`, `1067-1100`) | No | No merge contract | Stable synthesis independent of completion timing | Declarative reducers and explicit join nodes | LWW hides collisions; key iteration isn't an ordering contract |
| Explicit join | Live `JoinNode` (`join.rs:14-113`) | No | No join node type | Auditable aggregation and quorum inputs | Compile safe join modes: collect, merge, first-non-null, quorum metadata | Custom closures cannot come from untrusted JSON |
| Conditional router | Live `RoutingFunction` (`router.rs:35-79`) | Partial substring `HashMap` router | MCP ordering nondeterministic; unsafe semantics | Adaptive plans and retrieval | Ordered rule array with typed predicates/default | Prompt-controlled route escalation |
| Loops | Live with iteration limit (`engine.rs:605-635`) | Explicitly rejected as cycles (`main.rs:321-346`) | No refine/retry/adaptive loop | Critique/refine, adaptive retrieval | Ordered conditional back-edge; mandatory max iterations | Cost and state explosion |
| Per-node retries | Live (`retry.rs:9-105`, `engine.rs:1330-1505`) | No | Any LLM error ends request | Resilience to transient model/tool failures | Declarative retry policy, retryable class allowlist | Retrying side effects; jitter harms replay |
| Cancellation | Live between supersteps (`engine.rs:233-272`, `625-628`) | No, request is synchronous | No run ID or background task | Stop runaway work from Hermes | Async run registry + `graph_execute{action:"cancel"}` | Node calls may ignore cancellation |
| Streaming/events | Live, bounded/drop-on-pressure (`event_sink.rs:32-260`, `engine.rs:274-317`) | No | Stdio request returns only terminal result | Progress UI, traces, incremental tokens | Per-run ring buffer; inspect cursor; optional MCP notifications later | Event loss, sensitive content |
| Trace context | Live canonical/legacy IDs (`config.rs:5-38`, `95-118`) | No run/trace ID | Cannot correlate Hermes/tool/model spans | End-to-end diagnosis and receipts | Accept safe trace metadata and return IDs | User spoofing; metadata leakage |
| State reducers | Live built-ins/custom (`reducer.rs:4-132`) | No | Parallel writes unavailable | Deterministic shared-state aggregation | Allowlisted reducer names per state key | Type mismatch, silent LWW |
| State bounds | Live per-key/value/history (`state.rs:11-31`, `383-454`) | MCP has request/output caps | Core lacks total-state cap; MCP doesn't instantiate core limits | Predictable resource use | Server hard caps + per-run `StateLimits` + total serialized state/output checks | Memory/serialization DoS |
| Transactions | Live API (`state.rs:35-110`) | No | Nodes do not automatically transact | Atomic deterministic transforms | Use transaction inside transform node | Long transactions/conflicts |
| Static interrupts | Live before/after (`builder.rs:140-155`, `engine.rs:654-705`, `799-839`) | No | No pausable runs | Approval gates, reviews | Compile interrupt node/static edge, persist state, return status | Mistaking volatile pause for durable HITL |
| Dynamic interrupt/tool request | Partial: error path; typed `NodeOutcome` unused | No | No typed resume input; no Hermes callback | Safely delegate tool/approval to Hermes | Node emits interrupt request; Hermes executes externally; resume with correlation-bound result | Injection, replayed/stale approval |
| Resume | Partial graph-hash validation (`graph.rs:189-222`) | No | Ignores saved iteration/active set; no injected continuation | Continue approval-gated or failed work | Persist run record and explicit resume contract; fail closed on digest mismatch | Stale graph/state; duplicate effects |
| Checkpoint policy | Live required/best-effort/disabled (`graph.rs:32-43`) | No | MCP stores nothing | Choose fail-closed governance vs low-overhead runs | Runtime option mapped to builder | “Best effort” misrepresented as durable |
| Superstep checkpoints | Live memory/SQLite saver (`checkpointer.rs:8-117`) | No | No configured saver or thread ID | State history, recovery, inspection | Server-owned SQLite path and retention; required policy for resumable templates | Secrets at rest, DB corruption |
| Granular run/attempt records | Live interface; only in-memory implementation (`checkpoint_store.rs:36-141`, `221-246`) | No | No durable implementation | Retry/audit visibility | Add SQLite checkpoint-store implementation or clearly label volatile | Split-brain between two checkpoint rails |
| State history/update | Live legacy rail (`graph.rs:224-259`) | No | No inspection API | Debug/time travel and controlled corrections | `graph_status` resource selectors; update only under explicit guarded action | Tampering with evidence/state |
| Graph digest/drift check | Live semantic `GraphSpecV1` digest (`graph.rs:45-109`, `308-375`) | No; ID is mutable name | No immutable version | Safe resume, cache/template identity | Return `graph_id` plus immutable `graph_version` digest | Opaque node specs weaken identity |
| Graph visualization | Live Mermaid (`graph.rs:261-291`) | No | No inspectable topology | Hermes can explain/confirm workflows | `graph_status{resource:"graph", include:["mermaid"]}` | Diagram not execution proof |
| Subgraphs | Live but coarse (`builder.rs:80-84`, `graph.rs:378-417`) | No | No nested spec or mappings | Reusable specialist workflows | Referenced immutable graph version; explicit input/output key maps | Checkpoint namespace and recursive graph DoS |
| Custom executor | Live trait (`executor.rs:19-58`) | No | MCP hardcodes local LLM calls | Future sandbox/queue integration | Server-selected executor, never user-supplied code | Remote execution trust |
| LLM payload | Core `PayloadNode` live; MCP LLM direct | MCP-live only direct LLM | Two incompatible payload paths; no token context | Planner/critic/synthesizer nodes | Adapter implementing core `Payload`; explicit selector/mapper | SSRF, secret leakage, prompt injection |
| Passthrough | Trivial | MCP-live | Does not transform state | Wiring/aliases | Keep for compatibility | False impression of copying/mapping |
| Deterministic transform | Core can use `FnNode`; no serializable safe DSL | No | JSON cannot create closures safely | Parsing-free state shaping, counters, flags | Small allowlisted transform ops | Expression injection, unbounded output |
| Dynamic `SendOp` branch state | Type-only; state discarded (`command.rs:38-45`, `engine.rs:1042`) | No | Advertised semantics are false | Map-style per-item work | Fix core before exposing, with tests | Cross-branch state corruption |
| Structured events/receipts | Partial (`event_sink.rs`, `receipt.rs`) | No | No MCP retrieval; missing external call evidence | Observability and audit | Redacted persisted event/receipt store | Logs treated as proof; sensitive data |
| Run receipt | Partial synthetic step (`engine.rs:78-175`) | No | Not per-node and empty tool calls | Outcome/digest evidence | Prefer corrected bundle receipt; label receipt schema version/capabilities | Overclaiming provenance |
| Run bundle | Partial per-node state deltas (`engine.rs:319-395`) | No | Captures all env; envelopes empty | Offline debugging and integrity checks | Redacted allowlist environment; optional encrypted payload capture | Secret exfiltration and large artifacts |
| Offline replay verification | Live integrity check (`engine.rs:397-493`) | No | Does not re-run or prove external calls | Detect bundle tampering/divergence | `graph_execute{action:"verify_replay"}` on stored/imported bundle | Calling verification “deterministic re-execution” |
| Model/tool/memory envelopes | Type-only producer gap (`receipt.rs:44-123`; bundle initializes empty at `engine.rs:389-391`) | No | External dependencies not captured | Faithful diagnostic replay | Instrument adapters with redaction and opt-in payload retention | Sensitive request/response capture |
| Graph memory retrieval | Type-only trait (`memory.rs:1-34`) | No | No engine consumer | Adaptive retrieval and context continuity | MCP-side interrupt/tool-request to Hermes memory, or a vetted adapter | Memory is recall, not authority; stale facts |
| Templates | None | None | Every graph hand-authored | Safe high-value defaults | Versioned server-owned declarative specs | Templates silently gain new authority |

## 4. Recommended architecture: capability at the MCP/skill edge

Keep Hermes core narrow. Hermes should know only that it can call an MCP tool, receive a run/interrupt result, execute an already-authorized Hermes tool if requested, and resume with a correlation-bound response. It should not embed graph scheduling, checkpoint schemas, reducer logic, provider routing, or template implementation.

Recommended layers:

1. **Hermes skill/policy edge:** chooses an approved template or graph, supplies input and trace metadata, surfaces progress, evaluates approval prompts, executes allowed Hermes tools, and calls resume. Existing Hermes authorization remains authoritative; graph nodes cannot mint tool authority.
2. **MCP protocol layer:** validates backward-compatible schemas, enforces quotas/allowlists, owns graph versions and run IDs, redacts outputs, and exposes three stable tools.
3. **Declarative compiler:** converts `GraphSpecV2` to `AgentGraphBuilder`, safe node adapters, reducers, ordered routers, checkpoint policy, and `GraphConfig`. It rejects unsupported/type-only core semantics.
4. **Run manager:** holds cancellable tasks, event ring buffers, interrupted state, receipts, and retention metadata. A configured persistence adapter can make selected artifacts durable.
5. **Core runtime:** remains provider/tool agnostic and owns scheduling, retries, fan-out/fan-in, loop limits, state reduction, trace IDs, and core receipt mechanics.
6. **Effect adapters:** LLM adapter may call a server-configured provider. Hermes tools are not callable directly by a stdio MCP server; tool nodes must normally interrupt and return a request for Hermes to authorize/execute, then accept the result on resume.

This follows the useful boundary seen in the official ecosystems: LangGraph ties interrupts and inspection to checkpointers and thread identity; Temporal distinguishes durable workflow control from retryable external activities and signals; OpenAI Agents traces agents, generations, tools, guardrails, and handoffs; AutoGen GraphFlow uses explicit sequential/parallel/conditional/looping structure. The local runtime can adopt those boundaries without claiming their stronger durability or hosted tracing guarantees.

## 5. Backward-compatible three-tool MCP schema

Do not add a tool per operation. Preserve all current names and default behavior while extending them with optional discriminators. Existing calls remain valid.

### 5.1 `graph_create`

```json
{
  "action": "create | validate | delete",
  "spec": { "...GraphSpecV2...": "..." },
  "template": { "id": "research_synthesis", "version": "1", "parameters": {} },
  "graph_id": "optional for delete",
  "if_version": "optional immutable digest",
  "dry_run": false
}
```

Compatibility: missing `action` means `create`; the existing V1 `{name,entry,nodes,edges,recursion_limit}` is accepted and normalized. Exactly one of `spec` or `template` is required for create/validate. Response includes mutable `graph_id`, immutable `graph_version`/digest, normalized spec version, warnings, and `status`. Delete must be explicit, version-conditional, and reject graphs referenced by live runs unless `force` is a separately authorized server policy; no user-supplied arbitrary code or URLs enter the graph spec.

### 5.2 `graph_execute`

```json
{
  "action": "start | resume | cancel | verify_replay",
  "graph_id": "name-or-id",
  "graph_version": "required for resume and recommended for start",
  "input": {},
  "run_id": "required except start",
  "resume": {
    "correlation_id": "required",
    "value": {},
    "expected_interrupt_kind": "tool_request | approval | input"
  },
  "runtime": { "...RuntimeOptionsV1...": "..." },
  "bundle": "optional imported bundle for verify_replay",
  "wait": "terminal | interrupted | accepted",
  "timeout_ms": 30000
}
```

Compatibility: missing `action` means synchronous `start` with current terminal response shape (`success`, `final_state`, `steps`, `error`) plus additive `run_id`, `trace`, `status`, and optional `interrupt`. `wait:"accepted"` enables background runs and cancellation. `resume` fails closed if graph digest, run epoch, correlation ID, or interrupt kind differs. `verify_replay` only reports integrity verification and never calls models, tools, memory, or nodes.

### 5.3 `graph_status`

```json
{
  "resource": "server | graph | run | events | receipt | bundle | templates",
  "action": "get | list",
  "graph_id": "optional",
  "graph_version": "optional",
  "run_id": "optional",
  "cursor": "optional",
  "limit": 100,
  "include": ["spec", "mermaid", "state", "history", "attempts"],
  "redaction": "safe"
}
```

Compatibility: an empty object retains current server status. `state`, `history`, and bundles are omitted unless explicitly requested and permitted. Responses declare `storage_class` (`volatile`, `sqlite_checkpoint`, `durable_artifact`), `redactions`, `truncated`, and `capabilities` so clients cannot infer durability from a run ID.

All tools should return MCP `structuredContent` in addition to the current text JSON once the selected protocol version supports it. Errors remain tool results with `isError:true`, but acquire stable machine codes such as `GRAPH_VERSION_MISMATCH`, `RUN_INTERRUPTED`, `LIMIT_EXCEEDED`, and `POLICY_DENIED`.

## 6. Declarative node specification

Use a tagged, versioned node union. Node IDs must match a conservative pattern and be unique. Every node declares explicit `input` selectors and `output` mappings; defaults can preserve V1 `__input__` behavior.

```json
{
  "id": "critic",
  "type": "llm",
  "input": {"from": "state", "path": "/draft"},
  "output": {"set": "/critique"},
  "retry": "transient_llm",
  "config": {}
}
```

Supported V2 node types:

- **`llm`:** server-selected provider profile plus model alias, prompt template, JSON mode, max output tokens, timeout, and optional structured-output schema. No arbitrary base URL, headers, environment references, or secret names in graph JSON. Prompt templates interpolate allowlisted JSON-pointer values, with byte caps. This compiles to an `llm-pipeline` adapter implementing core `Payload`/`Node`.
- **`passthrough`:** copies or aliases selected state; V1 no-op remains supported. It must not imply validation.
- **`state_transform`:** an allowlisted deterministic operation: `set`, `copy`, `delete`, `increment`, `append`, `merge_object`, `select`, `compare`, or `format` with bounded input/output. No shell, JavaScript, regex backtracking, filesystem, network, or arbitrary expression evaluation.
- **`router`:** an ordered `rules` array, never a JSON map. Predicates operate on typed JSON paths (`equals`, `exists`, numeric comparison, bounded string contains, schema-valid) and name one or more targets. A required explicit default is `END` or target list. Route evaluation and target order are receipt-visible.
- **`join`:** explicit `inputs`, `output`, and safe `mode` (`collect_array`, `merge_objects`, `first_non_null`, `all_success`, `quorum`). A join is a state merge operation, not a scheduler barrier by itself; topology must ensure all branches converge into its superstep. Custom merge closures are not declarative.
- **`interrupt`:** emits `await_input`, `await_approval`, or `tool_request`, a bounded JSON payload, correlation ID, expected response schema, and resume output mapping. For tool requests it names a logical Hermes tool and arguments but does not execute it. It is safe only after core/MCP resume semantics are completed.
- **`subgraph`:** feasible by referencing `{graph_id, graph_version}` plus explicit input/output maps, maximum nesting depth, and inherited/stricter runtime caps. Inline recursive graphs and mutable-name-only references are rejected. Until checkpoint namespaces and nested interrupt tests pass, expose subgraphs as experimental and non-durable.

Edges remain a separately ordered array. Normal fan-out is explicit by multiple targets. Conditional control should normally live in a router node so the serialized semantic graph includes auditable rule order; opaque core closures must not be accepted from MCP input.

## 7. Runtime options and hard caps

```json
{
  "max_iterations": 20,
  "max_parallelism": 4,
  "retry_defaults": {
    "max_attempts": 3,
    "initial_delay_ms": 250,
    "backoff_factor": 2.0,
    "max_delay_ms": 5000,
    "jitter": false,
    "retry_on": ["timeout", "rate_limited", "provider_unavailable"]
  },
  "checkpoint_policy": "required | best_effort | disabled",
  "thread_id": "optional",
  "trace": {"trace_id": "optional", "tags": [], "metadata": {}},
  "limits": {
    "max_state_keys": 1000,
    "max_value_bytes": 262144,
    "max_total_state_bytes": 2097152,
    "max_node_output_bytes": 262144,
    "max_final_output_bytes": 524288,
    "max_events": 2000,
    "max_run_seconds": 900
  }
}
```

Server hard caps always dominate user options. Clamp/reject rather than silently increasing. Retry predicates are an allowlist of classified adapter errors; never retry policy denial, invalid input, prompt-injection detection, approval rejection, or a side-effecting tool without an idempotency key. Deterministic templates default to `jitter:false`. `required` checkpointing is mandatory for approval/tool-request templates and must fail before effects if the configured persistent backend is unavailable. Trace metadata must be scalar/short, deny secret-like keys, and never be treated as authorization.

The MCP cap should be materially below core defaults. Total serialized state must be checked after every node/superstep because core currently limits individual values but not aggregate size. Parallelism must also respect provider-specific concurrency and rate limits.

## 8. Persistence and receipt design

### 8.1 Storage classes and truthful durability

- **Volatile:** in-memory graph registry, run registry, `MemorySaver`, and `InMemoryCheckpointStore`. Survives only while the MCP process lives. Never label this durable or resumable-after-restart.
- **SQLite checkpoint:** a configured `SqliteSaver` can persist superstep state/history. It does not by itself persist the graph definition, run registry, granular attempt store, pending interrupt lease, or external effect envelopes. Label it checkpoint-persistent, not crash-proof durable execution.
- **Durable artifact:** only after normalized graph version, run metadata, interrupt record, state checkpoint, receipt/bundle, retention policy, schema version, and atomic commit behavior are persisted together can the server claim restart-resumable storage. Even then, it is local durable storage, not Temporal's distributed exactly-once model.

Persist graph specs by immutable digest and runs by random ID plus graph digest. A resume record includes checkpoint version, run epoch, active node/superstep, iteration, correlation ID, interrupt kind/schema digest, last committed effect sequence, and expiration. Resumes are compare-and-swap: one interrupt response is consumed once. Mutable graph names resolve only at start; resume always uses the pinned digest.

### 8.2 Receipts and redaction

Receipts are append-only evidence artifacts, not truth or authorization. Record:

- graph digest/schema version, run/trace IDs, storage class, runtime option digest, and timestamps;
- ordered supersteps, node/attempt/trial IDs, route decisions, retry classifications, checkpoint outcomes, and cancellation/interrupt transitions;
- canonical input/output **digests by default**, with payload retention opt-in and field-level redaction;
- tool request/response digests, Hermes authorization receipt reference, idempotency key, and result status when Hermes returns them;
- model alias/provider profile digest and request/response digests, not credentials or raw headers;
- memory query/result digests and provenance references, explicitly marked recall rather than authority.

Never call `std::env::vars().collect()` for a bundle. Replace `RunBundleV1.environment` population with an allowlist of non-secret runtime descriptors such as server version, target triple, graph runtime version, and configured feature flags. Environment variable names can themselves be sensitive; default to none. Raw prompts, model responses, state, and tool values require explicit retention policy, size limits, encryption/access control, and deletion semantics.

Replay levels must be named precisely:

1. `integrity_verified`: current `verify_replay`-style digest/state-chain checking only.
2. `dependency_stubbed_replay`: deterministic state replay using complete recorded model/tool/memory envelopes.
3. `live_reexecution`: invokes current dependencies and is inherently nondeterministic; compare results but never call it replay proof.

Current code supports only level 1 for state transitions, with empty dependency envelopes.

## 9. Security and governance threat model

| Threat | Current exposure | Required control |
|---|---|---|
| Prompt injection | LLM output becomes next `__input__` and can influence substring routing | Treat model text as untrusted data; typed structured outputs; deterministic routers; policy gates before tool/approval nodes; never let prompts grant authority |
| Arbitrary Hermes tools | MCP has no tool client today, which is a useful boundary | Keep it that way: interrupt with a request; Hermes enforces its own allowlist, user approval, argument schema, and idempotency; server verifies correlation on resume |
| SSRF/base URL abuse | CLI accepts any `--base-url`; status discloses it | Server-admin allowlist of schemes/hosts/ports, default loopback, DNS/IP revalidation, block link-local/private ranges unless explicitly configured, redact credentials, require TLS for remote hosts |
| Secret leakage | Run bundle captures the entire environment; states/events may contain secrets | Remove environment capture; redaction at ingestion and serialization; digest-by-default receipts; encrypted restricted artifacts; never echo provider auth or secret-like metadata |
| Unbounded graph/state/cost | MCP has graph/request caps; core lacks total state/run time/cost caps | Preserve hard node/edge/spec caps; add total state, event, time, token, nesting, iterations, parallelism, and retained-artifact quotas; cancellation and TTL cleanup |
| Stale graph resume | Core checks graph hash but `resume_force` exists and resume is incomplete | MCP never exposes force by default; pin immutable graph version; bind run epoch/correlation/schema; expire interrupts; CAS consume responses; show diff on mismatch |
| Nondeterministic routing | MCP iterates `HashMap` substring rules | Ordered rule arrays, explicit default, reject ambiguous rules or define first-match semantics, receipt the selected rule and ordered candidates |
| Retry duplicate effects | Core retries any error by default when a retry policy exists | Classify errors; effects require idempotency keys and commit receipts; no retry after uncertain effect without reconciliation |
| Parallel write conflict | Reducer defaults can make LWW silently win | Require reducer declaration for keys written by multiple branches; compiler performs write-set validation when possible; explicit join for critical aggregation |
| Checkpoint/evidence tampering | SQLite/local files have no access or integrity policy | Restrictive permissions, atomic writes, schema migrations, keyed signature/MAC where threat model requires, receipt digest chain, retention and audit access controls |
| Event/backpressure loss | Channel sink drops when full | Declare events best-effort unless persisted; sequence numbers and gap indicators; do not use event stream as sole execution evidence |
| Graph/template substitution | Current graph ID is mutable name and create overwrites | Immutable digests, conditional updates, signed/server-owned templates, pin versions in runs and receipts |
| Unsafe transforms/subgraphs | JSON-to-closure temptation; recursive nesting | Allowlisted DSL only, no eval; nesting/depth/size caps; referenced pinned graphs; validation before registration |
| Approval laundering | Model-generated text may look like approval | Approval is a typed Hermes-origin response bound to run, interrupt, graph version, action digest, user/session authority, and expiration |

## 10. High-value Hermes workflow templates

Templates are immutable, versioned graph specs plus parameter schemas and policy declarations. They orchestrate only the node types above. They cannot browse, edit files, execute shell, call arbitrary MCP tools, send messages, approve effects, or access semantic memory unless Hermes performs the requested operation and resumes the run.

| Template | Shape and value | MCP can execute | Requires Hermes/external action |
|---|---|---|---|
| `research_synthesis` | Plan -> parallel source-analysis LLM branches -> explicit evidence join -> synthesis -> citation/coverage check | LLM nodes, fan-out, join, retry, receipts | Actual web/app search and source fetching must be tool requests; Hermes returns bounded source records |
| `plan_critique_refine` | Planner -> critic -> router -> bounded refinement loop -> finalizer | LLM, deterministic routing, loop cap | Optional user acceptance only through interrupt |
| `parallel_council` | N independent roles -> ordered join -> judge -> dissent retention | Parallel LLM calls, deterministic ordering, aggregation | Does not create independent evidence; roles share provider/context unless configured otherwise |
| `implementation_verification` | Spec analysis -> implementation request interrupt -> parallel test/review analysis -> adjudication | Planning/review LLMs, joins, run evidence | Hermes/Codex edits files and runs commands; MCP only receives structured diffs/test results and cannot claim tests ran without receipts |
| `adaptive_retrieval` | Query classify -> retrieval request -> sufficiency judge -> bounded refine/retrieve loop -> answer | Classifier/judge/loop | Hermes or vetted memory/search connector performs retrieval; memory results are recall, not authority |
| `approval_gated_action` | Prepare action -> policy summary -> approval interrupt -> action request -> verify result | Preparation, pause/resume, digest binding | Hermes obtains real approval and executes authorized action; MCP never self-approves |
| `failure_recovery` | Classify failure -> retry/reconcile/router -> request operator input if ambiguous -> verify recovered state | Deterministic classification, safe retries, checkpointed pause | Side-effect reconciliation and operator choice come from Hermes/external system |
| `context_compaction_adjudication` | Extract candidate facts in parallel -> contradiction/privacy critics -> deterministic join -> adjudicator -> approval gate -> compact artifact | LLM analysis, join, receipt/digest | Source context and memory reads supplied by Hermes; persistence/deletion requires separate authority; output is advisory until approved |

Template parameters may select approved model aliases, bounded branch counts, iteration caps, and prompts within guarded fields. They may not loosen server limits, select raw URLs, add tools, or change checkpoint policy below the template's minimum.

## 11. TDD implementation plan and binary acceptance gates

Implement in small phases. Tests precede or accompany each behavior. Exact proposed files assume the current one-file MCP binary is split without changing the core payload boundary.

### Phase 0: characterize and lock compatibility

Files:

- `agent-graph-mcp/tests/mcp_legacy_contract.rs` — golden JSON-RPC tests for current initialize/list/create/execute/status shapes and error behavior.
- `agent-graph-mcp/tests/fixtures/v1/*.json` — current graph calls and responses with nondeterministic fields normalized.
- `agent-graph-mcp/src/main.rs` — only extract transport after goldens exist.

Gate:

```bash
cargo test --manifest-path agent-graph-mcp/Cargo.toml --test mcp_legacy_contract
```

Acceptance: existing calls require no new fields; all three old tool names remain; V1 passthrough/router/LLM validation behavior is explicitly captured before migration.

### Phase 1: compile declarative specs to the real runtime

Files:

- `agent-graph-mcp/src/spec.rs` — `GraphSpecV1` compatibility parser, `GraphSpecV2`, ordered edges/rules, runtime schema.
- `agent-graph-mcp/src/compiler.rs` — validation and `AgentGraphBuilder` compilation.
- `agent-graph-mcp/src/nodes/{mod.rs,llm.rs,transform.rs,router.rs,join.rs,passthrough.rs}` — safe adapters.
- `agent-graph-mcp/src/server.rs` — graph registry with immutable digest/version.
- `agent-graph-mcp/tests/compiler.rs` — every node/edge/reducer/retry mapping and rejection.
- `agent-graph-mcp/tests/runtime_graph.rs` — sequential, parallel, deterministic join, conditional, bounded loop.

Gate:

```bash
cargo test --manifest-path agent-graph-mcp/Cargo.toml --test compiler --test runtime_graph
```

Acceptance: a two-edge fan-out actually runs both nodes; reversed completion timing produces identical merged output; ordered router overlaps always choose the documented first rule; loops stop at the configured bound; no handwritten executor remains.

Before exposing dynamic per-branch `send`, fix and test core:

- `agent-graph/src/engine.rs` — preserve and apply `SendOp.state` to each fork.
- `agent-graph/tests/send_state_tests.rs` — different per-branch inputs and deterministic merge.

Gate:

```bash
cargo test -p agent-graph --test send_state_tests
```

### Phase 2: async runs, events, cancellation, and trace identity

Files:

- `agent-graph-mcp/src/run_manager.rs` — task registry, cancellation handles, TTL, event sequence/ring buffer.
- `agent-graph-mcp/src/protocol.rs` — additive three-tool schemas and stable errors.
- `agent-graph-mcp/src/event_store.rs` — redacted bounded event sink.
- `agent-graph-mcp/tests/run_lifecycle.rs` — accepted/running/completed/failed/cancelled and cursor gaps.

Gate:

```bash
cargo test --manifest-path agent-graph-mcp/Cargo.toml --test run_lifecycle
```

Acceptance: start returns run/trace IDs; status is addressable; cancellation stops before the next node; event order has monotonic sequence IDs; overflow reports a gap; terminal output respects byte caps.

### Phase 3: checkpoints, interrupts, and exact resume contract

Core files:

- `agent-graph/src/interrupt.rs`, `agent-graph/src/outcome.rs`, `agent-graph/src/node.rs` — converge on one typed interrupt model.
- `agent-graph/src/engine.rs` — record interrupts, persist/restore iteration and active superstep, inject correlation-bound resume values, remove misleading unused fields or wire them.
- `agent-graph/src/checkpoint_store.rs` — resume metadata contract.
- `agent-graph/tests/durable_interrupt_tests.rs` — before/after/dynamic interrupts, parallel interrupt, restart fixture, stale graph, duplicate response.

MCP files:

- `agent-graph-mcp/src/nodes/interrupt.rs` — input/approval/tool-request node.
- `agent-graph-mcp/src/persistence.rs` — configured SQLite artifacts and migrations.
- `agent-graph-mcp/tests/resume.rs` — restart, graph mismatch, expiry, correlation mismatch, exactly-once response consumption.

Gates:

```bash
cargo test -p agent-graph --test durable_interrupt_tests
cargo test --manifest-path agent-graph-mcp/Cargo.toml --test resume
```

Acceptance: killing/restarting the MCP process can resume only when storage reports a durable class; a volatile run truthfully cannot; stale graph/resume tokens fail closed; approval/tool requests never execute inside MCP.

### Phase 4: receipts, redaction, and replay levels

Core files:

- `agent-graph/src/engine.rs`, `agent-graph/src/receipt.rs`, `agent-graph/src/payload.rs` — per-node receipts, populated payload context, adapter hooks for dependency envelopes, remove blanket environment capture.
- `agent-graph/tests/receipt_truth_tests.rs` — multi-node receipt, external envelope completeness flags, secret regression, tamper localization.

MCP files:

- `agent-graph-mcp/src/receipts.rs`, `agent-graph-mcp/src/redaction.rs` — artifact storage and digest-by-default policy.
- `agent-graph-mcp/tests/receipts_replay.rs` — inspect/export/verify and claim labels.

Gates:

```bash
cargo test -p agent-graph --test receipt_truth_tests
cargo test --manifest-path agent-graph-mcp/Cargo.toml --test receipts_replay
```

Acceptance: a two-node run has two correctly timed/digested step receipts; environment secrets are absent; missing dependency envelopes produce `integrity_verified` only, never “complete replay”; one-byte mutation localizes failure.

### Phase 5: templates, governance, and transport smoke test

Files:

- `agent-graph-mcp/src/templates.rs` and `agent-graph-mcp/templates/*.json` — eight pinned templates and parameter schemas.
- `agent-graph-mcp/src/policy.rs` — provider profiles, URL/tool boundaries, quotas.
- `agent-graph-mcp/tests/templates.rs`, `security.rs`, `stdio_e2e.rs` — golden compilation, malicious specs, JSON-RPC subprocess test.
- `agent-graph-mcp/README.md` and this docs directory — operational claims after code exists.

Gates:

```bash
cargo test --manifest-path agent-graph-mcp/Cargo.toml --test templates --test security --test stdio_e2e
cargo test --manifest-path agent-graph-mcp/Cargo.toml --all-targets
cargo clippy --manifest-path agent-graph-mcp/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path agent-graph-mcp/Cargo.toml --check
```

Final focused core gates (not a full Libraries workspace run):

```bash
cargo test -p agent-graph --all-targets
cargo clippy -p agent-graph --all-targets -- -D warnings
cargo fmt --all --check
```

Binary acceptance smoke sequence: spawn `agent-graph-mcp` over stdio; initialize; confirm exactly the three tools; validate/instantiate a template; start a parallel run; poll events; receive a tool-request interrupt; restart the server with persistent storage; resume with a bound result; inspect a redacted receipt; verify the bundle offline; attempt stale/double resume and confirm stable policy errors.

## 12. Priorities

1. Replace the handwritten MCP executor with a compiler to `AgentGraph`; this unlocks the largest value and removes semantic duplication.
2. Fix truth/safety defects before exposure: ordered routes, `SendOp.state`, environment capture, payload context, and receipt labels.
3. Add addressable asynchronous runs, trace IDs, events, and cancellation.
4. Complete durable interrupt/resume semantics and persistence before advertising approval-gated long-running workflows.
5. Complete per-node receipts and dependency envelope instrumentation before advertising replay beyond integrity checking.
6. Ship pinned templates only after their node types and governance tests are stable.

## 13. Explicit non-goals and claim boundary

Non-goals:

- moving graph scheduling or persistence into Hermes core;
- allowing graph JSON to execute shell, Python, JavaScript, arbitrary Rust closures, filesystem access, arbitrary HTTP, or arbitrary MCP tools;
- making the MCP server an authorization authority, secret manager, browser, code editor, or general tool broker;
- replacing Hermes's normal single-agent loop for simple tasks where graph structure adds no value;
- claiming exactly-once external effects, distributed durability, transactional provider calls, or Temporal equivalence;
- claiming OpenAI Agents-compatible guardrails/handoffs/tracing merely because similar events can be represented;
- claiming LangGraph-compatible interrupt continuation/subgraph persistence until the local resume/checkpoint gaps are fixed;
- claiming deterministic LLM output, deterministic live re-execution, or proof that external calls occurred from digests alone;
- treating memory retrieval, model judgment, events, logs, receipts, or templates as authority or truth.

Current truthful claim: **`agent-graph` is a tested in-process Rust graph scheduler with live supersteps, bounded parallelism, deterministic merge ordering for fixed topology, routers, bounded loops, reducers, retries, static interrupts, cancellation, events, trace context, volatile/granular checkpoints, optional SQLite superstep checkpoints, partial resume, subgraphs, state limits, partial receipts, and offline bundle-integrity verification.**

Current truthful MCP claim: **`agent-graph-mcp` is a process-local three-tool LLM workflow server with V1 create/execute/status, sequential execution, substring routing, and request/output bounds. It does not presently expose or execute the core `AgentGraph` runtime.**

Target claim after all gates: **Hermes can invoke versioned, bounded, policy-constrained graph workflows through three backward-compatible MCP tools, with real core scheduling, externally authorized tool-request interrupts, truthful storage classes, redacted evidence, and precisely labeled integrity/replay guarantees.**

## 14. External pattern references

These references guide boundaries; they are not evidence that the local code already implements the same guarantees.

- LangGraph: [persistence](https://docs.langchain.com/oss/python/langgraph/persistence), [interrupts](https://docs.langchain.com/oss/python/langgraph/interrupts), and [subgraphs](https://docs.langchain.com/oss/python/langgraph/use-subgraphs).
- Temporal: [durable execution overview](https://docs.temporal.io/) and [AI/HITL workflow boundary](https://go.temporal.io/platform-hub/ai-engineering).
- OpenAI Agents SDK: [tracing](https://openai.github.io/openai-agents-python/tracing/) and [handoffs](https://openai.github.io/openai-agents-python/handoffs/).
- AutoGen: [GraphFlow sequential, parallel, conditional, and looping workflows](https://microsoft.github.io/autogen/stable/user-guide/agentchat-user-guide/graph-flow.html).

## 15. Implementation status appendix (2026-07-15)

The MCP crate now compiles registered V1/V2 specs into `agent_graph::AgentGraph`; the handwritten sequential walker has been removed. The shipped slice includes core-runtime sequential execution, bounded multi-edge fan-out, deterministic core reducer merge order, explicit joins, ordered first-match routers, bounded loops, safe transforms, immutable semantic graph digests, volatile addressable runs, accepted background starts, cancellation requests checked at node boundaries, bounded cursor events, redacted integrity bundles, offline tamper verification, and two executable built-in templates.

The server still advertises exactly `graph_create`, `graph_execute`, and `graph_status`. Old calls remain valid and receive additive structured content, version, run, trace, receipt, and storage/capability fields. Legacy route maps normalize in lexicographic pattern order; V2 rule arrays preserve declared order.

Truth boundary: all graph/run storage remains `volatile`; cancellation does not interrupt a blocked provider call; dependency envelopes are incomplete; replay is labeled only `integrity_verified`. Resume, restart durability, approval/tool-request interrupts, arbitrary Hermes tool calls, exactly-once effects, and complete external-call replay return or remain `UNSUPPORTED`. Provider URLs remain server-start configuration only and never enter graph JSON.
