# Candidate Plan: Governed-Agent Reliability Proof

## Goal/architecture

Produce a planning-only, evidence-safe reliability proof for the runtime at:

`/home/sikmindz/Coding/agent-graph-mcp-release`, admitted at commit `ba9fba16c6093e541f0f2f54613fb254dc5819248`.

The proof compares a baseline and governed candidate over six immutable tasks from the legacy corpus. It must retain raw outputs, criterion-level adjudications, comparator envelopes, cryptographic manifests, receipts, and replay instructions.

Proposed evaluator layout, subject to repository inspection:

```text
agent-graph-mcp-release/
  evaluation/
    README.md
    runner.py
    model.py
    corpus_adapter.py
    execute.py
    adjudicate.py
    receipts.py
    replay.py
    schema/
      envelope.schema.json
    tests/
  scripts/
    run-evaluation.py
    replay-evaluation.py
```

The evaluator must fail closed for missing, malformed, incomplete, unauthorized, tampered, or non-reproducible evidence.

## Current-state evidence

The supplied memos establish:

- The canonical runtime repository and admitted commit are known.
- The canonical runtime checkout contains pre-existing dirty paths, including `Cargo.toml`, `src/main.rs`, `src/proxy.rs`, `src/spec.rs`, `tests/mcp_integration.rs`, `.hermes`, and `vendor/`.
- The corpus and deterministic comparator are legacy lineage inputs and must remain read-only.
- Required Rust, release-script, process-boundary, recovery, authority, proxy, and terminal-projection gates are identified.
- The exact evaluator API, comparator envelope, baseline definition, reviewer model, runtime invocation, CI workflow path, and site paths remain unresolved.
- No current compile, test, lint, comparator, or reproduction result has been supplied.

## Source inventory

| Source | Intended use |
|---|---|
| `/home/sikmindz/Coding/agent-graph-mcp-release` | Canonical runtime repository |
| `ba9fba16c6093e541f0f2f54613fb254dc5819248` | Initial admitted runtime commit |
| `/home/sikmindz/Coding/agent-graph-release/tools/corpus/held_out_audit_v1.json` | Six-task legacy corpus |
| `/home/sikmindz/Coding/agent-graph-release/tools/deterministic_comparator.py` | Legacy comparison interface and semantics |
| Legacy commit `0f1dda227b89272de4e74422556937997836b1d9` | Corpus/comparator lineage |
| `AGENTS.md` | Repository instructions and required gates |
| `Cargo.toml`, `Cargo.lock` if present | Dependency, MSRV, and build identity |
| `src/main.rs`, `src/proxy.rs`, `src/spec.rs` | Runtime behavior and lint scope |
| `tests/mcp_integration.rs` | Existing integration evidence |
| `tests/process_boundary.rs` | Process-boundary behavior |
| `tests/proxy_confinement.rs` | Proxy confinement behavior |
| `tests/proxy_stdio.rs` | Proxy stdio behavior |
| `tests/daemon_recovery.rs` | Recovery behavior |
| `tests/operator_authority.rs` | Authority and approval behavior |
| `tests/terminal_projection.rs` | Terminal receipt/projection behavior |
| `scripts/validate-release.py` | Release validation |
| `scripts/validate-advisories.py` | Advisory validation |
| `tests/test_release_scripts.py` | Release-script tests |
| `README.md`, `CHANGELOG.md` or existing release-note file | Documentation and verified gate status |
| `.github/workflows/ci.yml` or existing workflow path | CI parity |
| `council-outputs-all-12.json`, `council-extracts.md` | Planning/case-study evidence references only |
| `recursiveintell-web`, `stack-showcase` | Possible public integration targets; exact paths unknown |

## Assumptions

- Git and Rust toolchains are available.
- The admitted commit can be isolated without modifying the dirty canonical checkout.
- The legacy corpus and comparator remain usable.
- One engineer is available, with review/evaluation support in parallel.
- Five repetitions per arm may be feasible; otherwise three repetitions may be used and explicitly labeled as a limitation.
- The evaluator can inject controlled approval, receipt, credential, interruption, and tamper faults.
- The current repository conventions permit a separately identifiable evaluator, likely under `evaluation/` or `tools/evaluation/`.

## P0 blockers

Resolve before implementation or public claims:

1. Exact isolated worktree path and clean-worktree procedure.
2. Current repository state and actual gate commands from `AGENTS.md`.
3. Exact comparator CLI and envelope schema.
4. Exact corpus task fields and acceptance/denial criteria.
5. Baseline definition and candidate runtime configuration.
6. Runtime invocation and provider/model availability.
7. Reviewer identity, authority classes, and disagreement-resolution model.
8. Receipt and manifest schema.
9. Whether Python evaluator files and proposed documentation paths fit repository conventions.
10. Exact CI and public-site paths.
11. Whether independent fresh-environment reproduction is feasible within five working days.

## Phases in dependency order

1. **Source admission:** inspect repository, preserve live dirt, create isolated worktree, record source identity, establish gates.
2. **Lineage and evaluator contract:** inspect the corpus and comparator directly; freeze six-task parity, schema, authority, and receipt requirements.
3. **Runner implementation:** load exactly six tasks, execute both arms identically, retain raw outputs, and emit comparator-compatible envelopes.
4. **Criterion adjudication:** preserve immutable acceptance/denial criteria and require evidence-backed, authority-qualified decisions.
5. **Receipts and replay:** hash all inputs and outputs, create manifests, and implement fail-closed replay.
6. **Failure/security testing:** test malformed, missing, unauthorized, tampered, interrupted, replayed, and credential-invalid cases.
7. **Full execution and independent reproduction:** run both arms, compare results, and reproduce from a fresh checkout/environment.
8. **Release cleanup and documentation:** update only evidence-safe documentation and mirror verified gates in CI.
9. **Case study and outreach:** draft public material only after proof acceptance; begin outreach only after publication approval.

## Task table

| ID / repository | Files | RED command / expected failure | Minimal GREEN | Verification | Evidence path | Rollback | Dependencies | Hours low / likely / high |
|---|---|---|---|---|---|---|---|---:|
| T1 `/home/sikmindz/Coding/agent-graph-mcp-release` | Worktree state; `AGENTS.md`; `Cargo.toml`; `Cargo.lock` if present | `git status --short`; `cargo metadata --locked --no-deps`; expected dirty admission, metadata failure, or undocumented lineage | Detached isolated worktree at admitted commit with recorded identity and preserved live dirt | `git rev-parse HEAD`; `git remote -v`; `rustc --version`; metadata; status | `source-admission.json` or `admission-manifest.json`, transcripts | Remove only isolated worktree after archiving evidence | None | 1 / 2 / 3 |
| T2 same repository | Existing Rust sources/tests; release scripts | `cargo test --workspace`; strict Clippy/release commands; expected compile, test, lint, or release failure | Clean candidate passes applicable compile, test, fmt, lint, release, advisory, and script gates | `cargo fmt --all -- --check`; `cargo test --workspace`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; release checks | Retained command logs and failure records | Stop admission; discard isolated candidate only | T1 | 4 / 8 / 16 |
| T3 same repository | Legacy corpus and comparator paths; proposed evaluator schema/README | `python tools/deterministic_comparator.py --help`; evaluator fixture tests; expected unknown CLI/schema or missing-field failure | Frozen six-task contract and exact comparator-compatible envelope | Inspect `held_out_audit_v1.json` and comparator source; fixture tests | Contract/schema artifact and lineage manifest | Discard evaluator changes; do not alter legacy comparator | T1 | 4 / 8 / 12 |
| T4 same repository | `evaluation/runner.py`, `model.py`, `corpus_adapter.py`, `execute.py`, `scripts/run-evaluation.py` or repository-approved equivalents | `python scripts/run-evaluation.py ...`; expected nonzero result for missing task, malformed output, incomplete record, or parity mismatch | Both arms receive identical six inputs, policy, ordering, timeout/retry, and tool surface; exactly six records emitted | Baseline/candidate dry runs and envelope validation | `artifacts/<run-id>/...`, raw outputs, task records | Invalidate run artifacts; remove isolated implementation | T3 | 12 / 24 / 40 |
| T5 same repository | `evaluation/adjudicate.py`; evaluator tests | Omit criterion, span, reviewer, rationale, or receipt field; expected nonzero fail-closed result | Every criterion has `accept|deny|inconclusive`, evidence spans, rationale, authority, reviewer, timestamp, and rule digest; inconclusive is non-promotable | Criterion coverage, authority, malformed-output, denial-failure, and asymmetry tests | Adjudication ledger, evidence-span index, reviewer manifest | Revert evaluator branch/artifacts only | T3; may parallelize with T4 | 8 / 16 / 28 |
| T6 same repository | `evaluation/receipts.py`, `evaluation/replay.py`, schema and manifest files | `python scripts/replay-evaluation.py bundle/`; expected mismatch for altered/missing digest, output, approval, or task | SHA-256 manifest, content-addressed bundle, immutable receipt, and deterministic mismatch reporting | Replay accepted and rejected fixtures; `PROMOTE`, `REJECT`, and `SHADOW` branches | `run-manifest.json`, `receipt.json`, `sha256sums.txt`, replay results | Mark bundle invalid; retain for diagnosis | T3; can parallelize with T4/T5 | 8 / 16 / 28 |
| T7 same repository | `evaluation/tests/`; existing Rust boundary tests | Fault injection; expected any fail-open case to fail the proof | Zero unauthorized executions, accepted cross-run replays, tampered bundles, incomplete-success terminals, secret disclosures, or duplicate unauthorized effects | Six-task baseline/candidate fault matrix plus process-boundary, proxy, recovery, authority, and terminal suites | Fault matrix, raw logs, receipts, pass/fail summary | Stop publication and retain failed evidence | T4–T6; existing tests can begin after T1 | 12 / 24 / 40 |
| T8 same repository | Runner output and artifact bundle | `python tools/run_eval.py ...` / comparator invocation; expected stop on fewer than 6/6 records, denial failure, incomplete receipt, or replay failure | 6/6 records for both arms, zero candidate denial failures, complete comparator envelope and receipt | Repeated runs, fixed/logged seeds and ordering, comparator, independent replay | Baseline/candidate envelopes, comparator receipt, replay log | Return to last admitted commit; discard isolated candidate | T4–T7 | 16 / 32 / 48 |
| T9 same repository | `README.md`; `docs/reproducibility.md`; `docs/case-study-governed-agent-reliability.md`; CI/release files if confirmed | `python scripts/validate-release.py`; advisory tests; `git diff --check`; expected failure for undocumented or non-reproducible gates | Documentation maps every claim to evidence; CI mirrors verified gates; no production/performance claims | Fresh checkout and fresh-environment reproduction by an outsider | Reproduction transcript, documentation diff, source manifest | Revert documentation/site changes | T2; final proof T8 | 8 / 16 / 32 |
| T10 repository paths unresolved | `recursiveintell-web`, `stack-showcase`, possible `outreach/tracker.csv` | Publication/outreach review; expected stop for unsupported claim or missing proof | Evidence-safe case study and approved concise project entry; outreach only after approval | Claim-to-evidence audit; threshold tracking | Case-study evidence links and local tracker if authorized | Withdraw link, stop outreach, correct wording | T9 | 8 / 16 / 32 |

## Parallel execution map

After T1 and source inspection:

- Lane A: clean runtime gates, MSRV, lint, release scripts.
- Lane B: corpus/comparator lineage and evaluator schema.
- Lane C: receipt schema, hashing, replay fixtures.
- Lane D: failure/security fixtures and existing Rust boundary-suite review.
- Lane E: documentation and case-study drafting, without publication.
- Lane F: outreach copy drafting only, without sending.

T4–T6 converge before T7/T8. T9 follows accepted proof and independent reproduction. T10 is strictly last.

## Critical path

`T1 → T2/T3 → T4 → T5/T6 → T7 → T8 → T9 → T10`

The likely serial bottlenecks are comparator/schema discovery, runtime invocation, adjudication authority, complete six-task execution, and independent reproduction.

## Release/reproduction gates

Release remains blocked until all are true:

- isolated checkout is clean and based on the admitted commit;
- compile, format, test, strict Clippy, release, advisory, and applicable CI gates pass;
- baseline and candidate use identical six-task inputs and execution policy;
- both arms produce 6/6 complete records;
- every criterion has evidence-backed adjudication;
- candidate denial failures equal zero;
- receipts and manifests are complete and hash-verified;
- tamper, replay, approval, interruption, credential, and recovery tests fail closed;
- comparator output is complete and retained;
- independent checkout/environment replay succeeds without source edits or live-worktree access;
- the runner is completed within five working days, or the schedule risk is explicitly reassessed.

## Public claim boundary

Permitted only after the gates:

- exact observed results on the six-task corpus;
- tested approval/denial behavior;
- retained evidence, receipts, and replay procedure;
- exact runtime/evaluator commits, versions, and limitations.

Do not claim:

- production readiness;
- customer validation or demand;
- security guarantees;
- revenue or market opportunity;
- benchmark superiority;
- speed or reliability beyond the measured scope;
- broad generalization from six tasks;
- model judgment as authoritative without named human confirmation.

## Outreach decision tree

1. If any proof, denial, receipt, comparator, or reproduction gate fails: stop outreach and reassess.
2. If proof passes: obtain publication approval, then contact ten individually identified targets.
3. If at least 3 of 10 understand a concrete use case and at least 1 requests technical follow-up, trial discussion, or interview: proceed to a bounded next conversation.
4. If at least 3 understand but none follows up: revise explanation, artifact, or targeting; do not infer demand.
5. If fewer than 3 understand but 1 follows up: investigate role- or artifact-specific interest before expanding.
6. If fewer than 3 understand and none follows up: stop expansion and reassess positioning.
7. Do not invent names, employers, openings, endorsements, customer status, or market evidence.

## Hard-no list

- No edits to the live dirty checkout.
- No destructive reset or overwrite of pre-existing paths.
- No modification of legacy corpus or comparator semantics.
- No comparison with incomplete or asymmetric task records.
- No promotion after any candidate denial failure.
- No model-only unreviewed authority for disputed scoring.
- No fail-open handling of malformed, missing, unauthorized, tampered, interrupted, or replayed evidence.
- No unsupported security, production, customer, performance, demand, or superiority claims.
- No public publication or outreach before independent reproduction.
- No durable-memory capture from this unverified plan.

## TOTAL TIME

Calendar assumptions: one engineer; five working days per week; parallel review/evaluation support available; public outreach response time is excluded from implementation elapsed time.

| Estimate | Serial elapsed | Total labor |
|---|---:|---:|
| Low | 7 working days / approximately 9 calendar days | 73 hours |
| Likely | 14 working days / approximately 18 calendar days | 160 hours |
| High | 25 working days / approximately 35 calendar days | 279 hours |

These totals include implementation, verification, documentation, and release preparation. Outreach response time remains open-ended and is not included.
