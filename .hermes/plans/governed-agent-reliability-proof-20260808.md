# Governed-Agent Reliability Proof — Complete Execution Plan

**Status:** implementation-ready plan; no implementation or external publication has occurred  
**Date:** 2026-08-08  
**Canonical runtime:** `/home/sikmindz/Coding/agent-graph-mcp-release`  
**Admitted starting revision:** `ba9fba16c6093e5410f2f54613fb254dc5819248` (`agent-graph-mcp` 0.3.0)  
**Planning council:** `governed-agent-proof-plan-12-20260808-v2`  
**Council graph version:** `sha256:5f9c264cda6f6111b2fd8f229316d78739cd6b524baa1df3d3e80d16d1018a09`  
**Council run:** `run-19fe3be6015-1`  
**Execution evidence:** 12 successful LLM invocations, 14 graph nodes, 153,816 ms wall-clock  
**Council authority:** advisory (`structural_unverified`); this plan is controller-reconciled against current files

## 1. Decision and outcome

Build one reproducible, fail-closed evaluation slice that compares:

- **Baseline:** one LLM node, one task, one final response.
- **Candidate:** three independent JSON analysts, deterministic `contradiction_matrix` and `minority_report` joins over the same analyst artifacts, then one synthesis LLM node.

Run both arms against the same six immutable held-out tasks, same model/provider generation, task wording, context, final-output ceiling, timeout policy, and attempt policy. Retain raw output, per-criterion adjudication, graph/run identities, receipts, hashes, latency and call counts. Recompute the comparison offline from retained evidence.

The proof is useful only if an outsider can clone the admitted source, verify the vendored inputs, validate the evidence bundle without provider access, and understand exactly what was and was not proved.

## 2. Explicit non-goals

This plan does **not** authorize or require:

- a new agent protocol, mailbox, recursive delegation layer, UI, semantic-memory rewrite, or general platform expansion;
- modifications to the live dirty checkout;
- 12-way simultaneous provider execution;
- production deployment or service-configuration changes;
- alteration of the legacy corpus or comparator semantics;
- publication, pushing, messaging prospects, or other external effects without a separate approval;
- claims of production readiness, security, customers, revenue, market demand, benchmark superiority, or general reliability.

## 3. Current source truth

### 3.1 Canonical runtime

Observed on 2026-08-08:

- root: `/home/sikmindz/Coding/agent-graph-mcp-release`;
- branch: `main` tracking `origin/main`;
- HEAD: `ba9fba16c6093e5410f2f54613fb254dc5819248`;
- package: `agent-graph-mcp` 0.3.0, Rust 2021, MSRV 1.75;
- one Cargo workspace member;
- `Cargo.lock` is not tracked;
- working tree is dirty and must be preserved;
- status projection digest: `sha256:9ee00e335de061a2e9d374e91ff74ed4abe83ac296b3e4e4c92203edf28b4e84`.

Pre-existing modified paths include:

- `Cargo.toml`;
- `src/main.rs`;
- `src/proxy.rs`;
- `src/spec.rs`;
- `tests/mcp_integration.rs`.

Pre-existing untracked projections include `.hermes/` and `vendor/`.

The runtime source implements `contradiction_matrix`, `minority_report`, and `collect_object` joins in `src/compiler.rs`. These are source facts, not council proposals.

### 3.2 Governing commands

`AGENTS.md` is authoritative for repository practice. Its stated gates are:

```bash
cargo build
cargo build --release
cargo test --lib
cargo test --test daemon_recovery --test mcp_integration
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo deny check
```

The plan adds focused evaluator tests and the remaining existing integration tests; it does not replace the repository gates.

### 3.3 Legacy proof inputs

The clean legacy repository `/home/sikmindz/Coding/agent-graph-release` is at:

```text
0f1dda227b89272de4e74422556937997836b1d9
```

Inputs to vendor byte-for-byte into the canonical proof package:

| Source | SHA-256 |
|---|---|
| `tools/corpus/held_out_audit_v1.json` | `f1dc92efdbfe8197f9bb1ee894af4045f630df02912b3bb77789267e78e32e3e` |
| `tools/deterministic_comparator.py` | `e1c69a32e8b84dc62c46df50c0ff7ab667667c1f709d51495dfebeaad06ccb67` |

The corpus contains exactly six tasks and explicitly names the intended comparison as “dual-join council (candidate) vs single-agent collect (baseline).”

The comparator accepts `--baseline`, `--candidate`, `--noninferiority-margin`, `--bootstrap`, and `--label`. It emits `recursiveintell.comparison-receipt.v1`.

**Known comparator contract caveat:** its CLI exits nonzero for `SHADOW`, but returns zero for `REJECT`. The evaluator wrapper must parse the emitted verdict and fail closed for both `SHADOW` and `REJECT`; the vendored comparator remains byte-identical.

### 3.4 Evidence already available but not sufficient

- The prior first council produced 12 retained direction reports.
- The second planning council produced 12 successful LLM attempts and a candidate plan.
- Historical focused runtime tests passed on an earlier observed generation.
- The legacy repository’s Python/unit tests passed, but its strict Clippy gate currently fails.

None of those observations proves that the new evaluator exists, the clean canonical revision passes every gate, or the reliability claim is reproducible.

## 4. Controller reconciliation of the council plan

| Council proposal | Controller disposition | Reason |
|---|---|---|
| Use the current MCP repository as canonical runtime | Accept | Matches live source and the first judgment |
| Generic `evaluation/` subsystem | Accept with exact layout below | Repository has Python release scripts/tests; isolate proof tooling without changing runtime semantics |
| Treat legacy corpus/comparator as read-only inputs | Accept; vendor byte-for-byte | Fresh-checkout reproduction cannot depend on a sibling checkout |
| Use unspecified “dual-join” candidate | Replace with fixed topology below | Corpus names the shape, but no complete candidate spec is stored beside it |
| Use unspecified CI/site paths | Reject | No tracked `.github` workflow or live `recursiveintell-web`/`stack-showcase` checkout was found |
| `cargo test --workspace` plus all-features Clippy as sole gate | Replace | Use exact `AGENTS.md` gates, focused evaluator tests, and all current integration tests |
| Five repetitions by default | Replace with 3, escalate to 5 by preregistered rule | Bounds cost while still observing run variance |
| Low/likely/high labor 73/160/279 h | Replace with recalculated 74/144/240 h | Council low and likely totals did not match its task table |
| Outreach after proof | Accept with approval and private tracking | External effects and personal/contact data require an explicit gate |

## 5. Target artifact layout

All implementation occurs in an isolated worktree of the canonical runtime.

```text
evaluation/
├── README.md
├── SOURCE_MANIFEST.json
├── held_out_audit_v1.json             # byte-identical vendored input
├── deterministic_comparator.py        # byte-identical vendored input
├── contract.py                        # versioned records + fail-closed validators
├── runner.py                          # MCP JSON-RPC execution and raw capture
├── adjudicate.py                      # criterion decisions + blinded review bundle
├── replay.py                          # offline integrity/comparison replay
├── specs/
│   ├── baseline-single-v1.json
│   └── candidate-dual-join-v1.json
└── fixtures/
    ├── valid-run-v1.json
    ├── invalid-missing-criterion-v1.json
    ├── invalid-tampered-output-v1.json
    └── invalid-incomplete-receipt-v1.json

tests/
└── test_evaluation_harness.py

docs/evaluation/
├── governed-agent-reliability-v1.md
└── reproducibility-v1.md
```

Generated evidence goes under an ignored run directory, not source control by default:

```text
evaluation/artifacts/<experiment-id>/
├── experiment-manifest.json
├── source-manifest.json
├── baseline/
├── candidate/
├── adjudication/
├── comparison-receipt.json
├── offline-replay-receipt.json
├── sha256sums.txt
└── closeout.md
```

Only an explicitly reviewed, redacted evidence bundle may be promoted into `docs/evaluation/results/<experiment-id>/` or attached to a release.

## 6. Frozen experiment contract

### 6.1 Baseline graph

`evaluation/specs/baseline-single-v1.json`:

- one LLM node;
- no tool authority;
- input is the unmodified task text plus a neutral response contract;
- one final response;
- configured model, provider, reasoning effort, timeout, retry policy and final-output token limit recorded in the experiment manifest.

### 6.2 Candidate graph

`evaluation/specs/candidate-dual-join-v1.json`:

1. passthrough fan-out;
2. three LLM analysts with the same task input:
   - source/criteria verifier;
   - adversarial denial hunter;
   - practical decision and falsifying-test analyst;
3. each analyst emits strict JSON fields required by both joins: `scope`, `time`, `claim`, `dissent`;
4. deterministic `contradiction_matrix` join;
5. deterministic `minority_report` join over the same three artifacts;
6. deterministic state composition of both envelopes;
7. one synthesis LLM producing the final response.

The candidate has four LLM calls per task. The final synthesis node uses the same model and final-output ceiling as the baseline. Analyst overhead, wall time and invocation count are measured rather than hidden.

### 6.3 Repetitions and admission

1. **Canary:** task `t1`, one repetition per arm.
2. **Primary experiment:** all six tasks, three repetitions per arm.
3. **Escalation to five repetitions:** only when preregistered before inspecting arm labels and when any task’s acceptance score range exceeds 0.34 across the first three repetitions, or the comparison CI crosses the noninferiority margin.
4. No failed, empty, skipped or cancelled attempt is silently replaced. A retry is a new attempt with a retained failure record.
5. At most two candidate task runs execute concurrently. With three analysts per candidate, this caps planned provider overlap at six—below the one measured nine-provider run that crossed `MemoryHigh`.

Primary run volume:

- baseline: 6 tasks × 3 repetitions × 1 LLM = 18 LLM calls;
- candidate: 6 tasks × 3 repetitions × 4 LLM = 72 LLM calls;
- total: 90 LLM calls, plus two canary-arm executions totaling 5 LLM calls;
- five-repetition escalation adds 60 LLM calls.

### 6.4 Scoring and human authority

For every response and every corpus criterion, store:

- task ID, arm-blinded output ID and repetition;
- criterion type (`acceptance` or `denial`);
- exact criterion text and criterion digest;
- decision (`met`, `not_met`, `inconclusive`);
- exact output character spans or explicit “no supporting span”;
- short rationale;
- reviewer identity class (`automated_advisory` or `human_authority`);
- adjudication timestamp and adjudication-record digest.

Task acceptance score is:

```text
met acceptance criteria / total acceptance criteria
```

`inconclusive` counts as not met. Any matched denial criterion is listed in `denial_failures`.

An automated rubric judge may prepare a blinded draft, but it cannot authorize promotion. A named human must confirm every denial decision and all disputed/inconclusive acceptance decisions before publication. Unresolved disagreement remains `inconclusive` and blocks promotion.

### 6.5 Comparison and promotion

Feed one aggregate score per task and all denial failures into the byte-identical comparator. Aggregate task score is the mean across admitted repetitions.

The wrapper rejects promotion unless all are true:

- both arms contain exactly six task IDs and the admitted repetition count;
- no required raw output, attempt record, receipt, criterion, reviewer decision or digest is missing;
- candidate denial failures equal zero;
- comparison verdict is `PROMOTE`;
- comparison lower 95% paired-bootstrap bound is at least `-0.02`;
- candidate paired quality delta is positive;
- no unresolved human-review disagreement exists;
- all integrity, replay, security and reproduction gates pass;
- latency and LLM-call overhead are disclosed;
- a human owner signs the promotion decision.

A baseline denial failure is preserved exactly as the v1 comparator handles it. If that causes `REJECT`, do not reinterpret or patch the result after seeing outcomes. Record that the v1 comparison contract rejected the trial. Any revised comparator must be a separately versioned experiment with a new preregistration.

## 7. Versioned record contracts

`evaluation/contract.py` owns the canonical Python record validation. Do not duplicate semantic validation in the runner, adjudicator and replay scripts.

Required record families:

- `recursiveintell.evaluation-experiment.v1`;
- `recursiveintell.evaluation-attempt.v1`;
- `recursiveintell.evaluation-adjudication.v1`;
- existing `recursiveintell.comparison-receipt.v1`;
- `recursiveintell.evaluation-replay-receipt.v1`.

Every record includes its schema string, source revision, corpus/comparator digests, arm/spec digest, model/provider label, prompt digest, run ID, node/attempt terminal state, output digest and parent experiment ID where applicable.

Offline replay verifies retained bytes and recomputes adjudication aggregation plus comparator output. It must not call a model or mutate graph/runtime state.

## 8. Phase plan

### P0 — Source admission and containment

**Dependencies:** none  
**Estimate:** 2 / 4 / 8 hours

1. Capture the current canonical checkout’s branch, HEAD, remotes, porcelain-v2 status and relevant path hashes into the run evidence directory.
2. Confirm the target revision exists locally and on `origin/main`.
3. Create a fresh isolated worktree and branch; never clean/reset the live checkout:

```bash
if git -C /home/sikmindz/Coding/agent-graph-mcp-release \
  show-ref --verify --quiet refs/heads/proof/governed-agent-reliability-v1; then
  echo "proof branch already exists; choose a new explicit suffix" >&2
  exit 1
fi
git -C /home/sikmindz/Coding/agent-graph-mcp-release worktree add \
  -b proof/governed-agent-reliability-v1 \
  /home/sikmindz/Coding/worktrees/agent-graph-mcp-proof-v1 \
  ba9fba16c6093e5410f2f54613fb254dc5819248
```

If the branch already exists, stop and select a new explicit suffix; do not overwrite it.

4. Record Rust/Cargo/Python/uv versions and `cargo metadata --no-deps --format-version 1`.
5. Bind the runtime under test: service owner, `ExecStart`, PID, `/proc/<pid>/exe`, installed binary hash, built candidate hash, provider label and data/socket paths. If source/build/live identities diverge, stop or launch an isolated candidate daemon through the admitted deployment procedure; do not modify the live service.

**Acceptance:** isolated worktree is clean at the admitted revision; live dirty status digest is unchanged.  
**Rollback:** remove only the new worktree/branch after evidence retention; never touch live uncommitted paths.

### P1 — Baseline gates and clean RED

**Dependencies:** P0  
**Estimate:** 4 / 8 / 12 hours

Run and retain full output for:

```bash
cargo build
cargo build --release
cargo fmt --check
cargo test --lib -- --test-threads=1
cargo test --tests -- --test-threads=1
cargo clippy --all-targets -- -D warnings
cargo deny check
uv run --with pytest pytest -q tests/test_release_scripts.py
```

Classify every failure as current source, known fixture, environment/tool availability, or invalid command shape. No blanket waiver is allowed.

Add `tests/test_evaluation_harness.py` before implementation. The first RED must fail only because the evaluation package/contracts are absent—not because of an import, fixture or command error.

**Acceptance:** valid baseline receipt plus clean product-specific RED.  
**Rollback:** delete only new RED-test edits if the contract is rejected; retain failure receipts.

### P2 — Freeze and vendor the evaluation contract

**Dependencies:** P1  
**Estimate:** 4 / 8 / 12 hours

1. Copy the corpus and comparator byte-for-byte into `evaluation/`.
2. Write `evaluation/SOURCE_MANIFEST.json` with source repository, source revision, source paths, destination paths and exact SHA-256 digests.
3. Add tests that compare vendored digests to the admitted values.
4. Inspect and document all six tasks, criteria, comparator inputs, output schema, random seed and CLI exit-code caveat.
5. Write the baseline and candidate specs described in section 6.
6. Validate both specs through `graph_create(action="validate")`; then execute one evidence sentinel per arm and assert the task sentinel reaches every required LLM lane and final response.

**Acceptance:** six immutable tasks; byte-identical imported assets; both specs validate; canary outputs are nonempty and role-relevant.  
**Rollback:** remove the proof package; never edit the legacy source files.

### P3 — Implement the runner

**Dependencies:** P2  
**Estimate:** 12 / 20 / 32 hours

Files: `evaluation/contract.py`, `evaluation/runner.py`, both spec files, runner tests.

Implement:

- strict corpus loading with exactly six unique task IDs;
- a bounded JSON-RPC MCP subprocess client with explicit command argument, timeout and output ceilings;
- create/execute/wait/get-state/get-receipt flow using unique graph and idempotency IDs;
- separate terminal attempt state from nonempty output admission;
- raw stdout/stderr and parsed response retention;
- arm parity checks for model, final-output ceiling, task bytes and retry policy;
- bounded scheduling with at most two concurrent candidate task runs;
- typed failures for timeout, empty output, missing node, missing receipt, malformed JSON, cancelled lane, model mismatch and idempotency collision;
- no credential logging and `[REDACTED]` replacement for known secret fields.

**RED tests:** missing task, duplicate task, empty lane, cancelled attempt, wrong model, missing receipt, malformed JSONL, oversized response, subprocess timeout.  
**GREEN:** canary emits admitted records for both arms while preserving every failed diagnostic.  
**Rollback:** invalidate generated experiment ID and remove only isolated branch changes/artifacts.

### P4 — Implement blinded criterion adjudication

**Dependencies:** P2; parallel with P3/P5  
**Estimate:** 8 / 16 / 24 hours

Files: `evaluation/contract.py`, `evaluation/adjudicate.py`, adjudication fixtures/tests.

Implement:

- arm-blinded output IDs and deterministic shuffled review order;
- complete criterion matrix generation;
- evidence-span bounds checks against immutable output bytes;
- advisory automated draft import;
- explicit human confirmation record;
- disagreement/inconclusive handling;
- aggregate task-score and denial-failure generation;
- prohibition on post-unblinding criterion edits without a superseding versioned record.

**RED tests:** omitted criterion, invalid span, changed criterion text, missing reviewer class, unauthorized promotion, unresolved disagreement.  
**GREEN:** every admitted output has exactly one terminal decision per criterion and aggregation is deterministic.  
**Rollback:** preserve candidate adjudications as rejected; regenerate a new version rather than editing history.

### P5 — Implement receipts, offline replay and comparator wrapper

**Dependencies:** P2; parallel with P3/P4  
**Estimate:** 8 / 16 / 24 hours

Files: `evaluation/contract.py`, `evaluation/replay.py`, comparator wrapper in `replay.py`, fixtures/tests.

Implement:

- SHA-256 content manifest over every retained source/input/output/adjudication/receipt file;
- canonical stable ordering for digest computation;
- output and graph-spec digest verification;
- attempt/run/receipt cross-checks;
- offline score re-aggregation;
- byte-identical comparator invocation;
- comparator-output digest and semantic comparison to retained receipt;
- wrapper exit nonzero for both `SHADOW` and `REJECT`;
- no provider/runtime calls during offline replay.

**RED tests:** changed byte, missing file, changed arm label, stale run ID, duplicate task, changed criterion, altered comparison receipt, comparator `REJECT` with process exit zero.  
**GREEN:** clean bundle replays identically offline; every mutation fails with a typed diagnostic.  
**Rollback:** quarantine the bundle; never “repair” evidence in place.

### P6 — Integration, failure and security matrix

**Dependencies:** P3, P4, P5  
**Estimate:** 12 / 24 / 40 hours

Run evaluator tests plus current runtime boundaries:

```bash
uv run --with pytest pytest -q tests/test_evaluation_harness.py tests/test_release_scripts.py
cargo test --test codex_app_server -- --test-threads=1
cargo test --test proxy_stdio --test proxy_confinement -- --test-threads=1
cargo test --test process_boundary --test daemon_recovery -- --test-threads=1
cargo test --test operator_authority --test tool_runtime -- --test-threads=1
cargo test --test terminal_projection --test mcp_integration -- --test-threads=1
```

Required negatives:

- graph validation rejects malformed specs;
- missing/cancelled candidate lane blocks synthesis admission;
- missing or incomplete durable receipt blocks experiment admission;
- wrong model/provider generation blocks arm parity;
- idempotency replay returns the original identity; changed payload with reused key is rejected;
- tampered raw output/adjudication/receipt fails offline replay;
- interrupted process does not emit a false success record;
- approval/authority paths cannot be granted by task text or analyst output;
- secret patterns never enter artifacts or logs;
- six-provider bound produces no OOM, daemon restart or unrecorded attempt loss.

**Acceptance:** zero fail-open cases, zero secret disclosures, full matrix receipt.  
**Rollback:** stop before full corpus run; preserve failures and last admitted revision.

### P7 — Execute the canary and primary experiment

**Dependencies:** P6  
**Estimate:** 8 / 16 / 32 hours

1. Preregister experiment ID, hashes, model/provider generation, topology specs, attempt policy, repetition policy, score rule and promotion gate.
2. Run `t1` once per arm.
3. Audit canary parity, raw output, attempt records, lane completeness, receipts and replay.
4. Run six tasks × three repetitions × two arms in deterministic randomized order.
5. Complete blinded advisory adjudication, then named human review.
6. Unblind only after adjudications are immutable.
7. Aggregate, invoke the comparator and run offline replay.
8. Escalate to five repetitions only under the preregistered rule; produce a new experiment version rather than appending selectively.

**Acceptance:** exactly 6/6 tasks in both arms; admitted repetitions complete; candidate denials zero; comparator and replay receipts complete.  
**Stop conditions:** credential request, live-daemon identity mismatch, memory/OOM event, lane loss, receipt incompleteness, asymmetric context, post-unblinding criterion change, or any attempt to replace a failed run silently.  
**Rollback:** no runtime deployment occurs; stop new runs, retain the experiment as `REJECT` or `SHADOW`.

### P8 — Fresh-checkout reproduction

**Dependencies:** P7  
**Estimate:** 4 / 8 / 16 hours

Required tiers:

1. **Offline reproduction:** fresh clone/worktree verifies vendored hashes and replays the published evidence bundle without model/provider credentials.
2. **Live canary reproduction:** fresh worktree with an already configured provider reruns `t1` in both arms without source edits.
3. **Full live reproduction:** desirable before strong public language; if unavailable, state that only offline replay plus same-host live canary were reproduced.

A reproducer follows only `docs/evaluation/reproducibility-v1.md`. No undocumented sibling checkout, private absolute path or manual evidence correction is permitted.

**Acceptance:** offline receipt digest matches; canary generates valid new run/receipt identities; instructions require no source edits.  
**Rollback:** mark reproduction failed and block docs/outreach; do not weaken instructions to hide the failure.

### P9 — Release quality, documentation and evidence-safe case study

**Dependencies:** P8 and all canonical source gates  
**Estimate:** 8 / 16 / 24 hours

1. Rerun all P1 and P6 gates on the final source generation.
2. Run `git diff --check` and review every changed path against the approved proof scope.
3. Write `evaluation/README.md`, reproducibility guide and case study.
4. Add a concise `README.md` link only after its contract tests still pass.
5. If CI is desired, create a new `.github/workflows/evaluation.yml` only after repository owner approval; it may run offline fixture/replay tests without provider secrets. Live model evaluation is not a pull-request gate.
6. Produce a claim ledger mapping every public sentence to source/result evidence.
7. Produce rollback instructions and a closeout bundle.

Allowed wording: exact six-task observed results, exact runtime/model/spec identities, exact denial/receipt/replay behavior, costs/latencies, and limitations.

Blocked wording: production-ready, secure, enterprise-ready, customer-validated, benchmark-superior, universally reliable, or any demand/revenue claim.

**Acceptance:** final diff scoped; tests/lint pass or an explicit non-release verdict is issued; fresh replay succeeds; claim ledger has no unsupported line.  
**Rollback:** revert documentation/CI changes on the proof branch; retain private evidence and do not publish.

### P10 — Targeted hiring/pilot validation

**Dependencies:** P9 plus explicit approval to publish and contact external parties  
**Estimate:** 4 / 8 / 16 engineering hours; response wait is separate

1. Discover ten current, individually relevant targets only after the proof topic is fixed:
   - five hiring targets with current agent-infrastructure/reliability roles;
   - five founder/engineering targets where governed-agent evaluation is plausibly relevant.
2. Inspect each original source—job listing, company product, or direct public technical context—before drafting.
3. Draft one evidence-safe message per target, linking only the approved proof artifact.
4. Store names/contact data in an untracked private tracker, never in the public repository or semantic memory.
5. Obtain approval before sending.
6. Record sent date, source URL, response, “understood concrete use case,” and concrete follow-up.

Decision thresholds after ten contacts:

- **Proceed:** at least 3/10 understand a concrete use case and at least 1 requests a technical follow-up, trial discussion, or interview.
- **Reframe:** at least 3 understand but none follows up.
- **Narrow:** fewer than 3 understand but one follows up.
- **Stop expansion:** fewer than 3 understand and none follows up.

No response is not evidence of product failure; it is weak channel/target/message evidence. Do not fabricate customers or pilots from technical interest.

## 9. Dependency graph and parallel execution

```text
P0 source admission
 └─ P1 baseline + clean RED
     └─ P2 contract + vendored assets + spec canaries
         ├─ P3 runner ──────────────┐
         ├─ P4 adjudication ────────┼─ P6 integration/security
         └─ P5 receipts/replay ─────┘
                                      └─ P7 experiment
                                          └─ P8 reproduction
                                              └─ P9 release/docs
                                                  └─ P10 outreach
```

P3, P4 and P5 may run concurrently only with non-overlapping file ownership; `evaluation/contract.py` has one designated owner and is integrated before the three lanes merge. All controller verification is rerun after integration.

## 10. Phase acceptance matrix

| Phase | Completion evidence | Block/quarantine trigger |
|---|---|---|
| P0 | clean isolated worktree + unchanged live status digest | source/build/live identity conflict |
| P1 | full command logs + valid product RED | unexplained compile/test/lint failure |
| P2 | byte hashes + six tasks + two valid canaries | source drift or missing lane |
| P3 | exact records for both arms | empty/skipped/cancelled/mismatched attempt |
| P4 | complete criterion matrix + human authority | unresolved disagreement or invalid span |
| P5 | identical offline replay + fail-closed mutations | mutable/missing evidence |
| P6 | security/failure matrix green | any fail-open or secret disclosure |
| P7 | 6/6 paired experiment + comparator receipt | denial, asymmetry, incomplete receipt |
| P8 | fresh offline replay and live canary | hidden dependency or source edit required |
| P9 | scoped diff, gates, claim ledger | unsupported public sentence |
| P10 | 10 verified targets + recorded outcomes | no approval or proof no longer current |

## 11. Rollback and quarantine policy

- The live dirty checkout is never reset, cleaned, formatted or used for proof implementation.
- No service deployment is part of the evaluator experiment.
- Every failed run and adjudication remains immutable; corrections create superseding records.
- Generated evidence is quarantined by moving only the artifact directory under a `rejected/` namespace and retaining its manifest.
- Remove a proof worktree only after receipts and hashes are copied to the controller evidence directory.
- Never delete or rewrite the legacy repository to make canonical gates pass.
- External material can be withdrawn; the underlying result and correction record remain retained.

## 12. Unknowns to resolve at P0/P1

- Exact installed-daemon source/binary identity at implementation time.
- Whether the canonical clean revision has the same strict-Clippy state as the live dirty checkout.
- Whether `cargo deny` is installed and its advisory database is available.
- Current provider availability and whether exact sampling controls beyond reasoning effort are exposed.
- Whether a second independent machine is available for full live reproduction.
- Where the final public portfolio entry should live; no current web repository path has been admitted.
- Which named human will sign criterion decisions and publication approval.

## 13. TOTAL TIME

Estimates are engineering effort, not promises. They exclude open-ended external response time and assume the model/provider remains available.

| Estimate | Total labor | Critical path before reserve | Critical path with 20% reserve | Working days at 8 h/day | Approx. calendar days |
|---|---:|---:|---:|---:|---:|
| Low | 74 h | 58 h | 69.6 h | 8.7 days | 12.2 days |
| **Likely** | **144 h** | **112 h** | **134.4 h** | **16.8 days** | **23.5 days** |
| High | 240 h | 192 h | 230.4 h | 28.8 days | 40.3 days |

**Planning-council runtime actually measured:** 153,816 ms = **2 minutes 33.816 seconds** for all twelve LLM executions and deterministic graph nodes.

**Likely time to a reproducible proof, approved case study, and first ten outbound messages:** about **17 working days / 24 calendar days**.

**Likely time to an initial market/job signal after allowing a two-week response window:** about **38 calendar days**. Low/high decision-window bounds are approximately **19–61 calendar days**.