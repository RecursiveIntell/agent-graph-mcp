# Twelve-Agent Unfinished-Work Direction Council — Independent Judgment

**Date:** 2026-08-08  
**Judge:** Hermes controller (not a voting council member)  
**Verdict:** **ACCEPT WITH CHANGES**

## Decision

For the next 7–14 days, stop expanding broad infrastructure and build one reproducible **governed-agent reliability evaluation** that converts the existing graph, approval, evidence, and receipt work into an externally legible proof.

The canonical runtime candidate is the current dedicated MCP repository:

- `/home/sikmindz/Coding/agent-graph-mcp-release`
- remote: `RecursiveIntell/agent-graph-mcp`
- committed public baseline inspected: `ba9fba16c6093e5410f2f54613fb254dc5819248` (`agent-graph-mcp` 0.3.0)

The older combined release repository supplies useful evaluation inputs, but is **not** accepted as the canonical product runtime:

- `/home/sikmindz/Coding/agent-graph-release`
- remote: `RecursiveIntell/ri-agent-graph`
- HEAD: `0f1dda227b89272de4e74422556937997836b1d9`
- embedded `agent-graph-mcp` package: 0.2.3

Use the latter's six-task corpus and comparator as source material, with explicit lineage, rather than presenting the repository wholesale as release-ready.

## Council execution evidence

Twelve valid LLM agent executions completed:

1. One admitted source-truth canary.
2. Eleven distinct role agents in one fan-out.
3. One deterministic non-LLM join containing all eleven fan-out outputs.
4. No model synthesis or thirteenth voting agent.

Durable execution evidence:

- Canary run: `run-19fe30a6987-1`
  - LLM calls: 1
  - graph version: `sha256:90659fa3fff968932784bcfc339c1206980c9d1f113646eccbff2dfe1eb642ae`
  - terminal persistence: `durable_terminal`
- Fan-out run: `run-19fe30ca1f7-1`
  - LLM calls: 11
  - graph version: `sha256:2b8c32acd11999977aa4f32f53dea62abaee56d6395f468858b9a39762afcfca`
  - completed lanes: 11/11
  - deterministic join: present
  - terminal persistence: `durable_terminal`
- Source-manifest witness: `witness-hmac-sha256:e1bfe972ed6355fff2b7667ca54dec52476a67520d6b04291cc05030deeab390`
- Complete outputs: `council-outputs-all-12.json`
- Human-readable extracts: `council-extracts.md`

All 12 admitted reports were nonempty and contained the required decision sections. One earlier diagnostic run, `run-19fe307aacd-1`, is explicitly excluded because the daemon did not deliver `context_file`; its model correctly refused to invent a ranking. It is not counted among the 12 valid outputs.

Both accepted run receipts label model evidence authority `structural_unverified`. The council was therefore advisory; the decisive claims below were independently checked against current files and real commands.

## Consensus

All twelve agents converged on the same strategic direction:

- stop broad subsystem expansion;
- use governed agent operations/reliability as the portfolio and product narrative;
- make approval, denial, evidence, and receipts externally demonstrable;
- run a bounded proof before investing further;
- use the public site/portfolio only after the proof is reproducible;
- measure real response from hiring managers, platform engineers, founders, or potential users.

There was no material strategic dissent. The contrarian lane dissented only from continuing large unfinished memory/agent infrastructure, reinforcing the primary conclusion.

## What survives independent review

### Accepted

1. **The portfolio is converging on governed agent infrastructure.** Current repositories show graph execution, approvals, source witnessing, receipts, semantic-memory work, and operator surfaces.
2. **Another subsystem is lower ROI than a proof package.** Several foundational repositories are dirty and carry large validation surfaces; activity is not user value.
3. **The evaluation assets are real.** `agent-graph-release` contains:
   - `tools/corpus/held_out_audit_v1.json` with six tasks and explicit acceptance/denial criteria;
   - `tools/deterministic_comparator.py` with paired bootstrap, noninferiority, contract-completeness, and denial gates;
   - `tools/operator_decide.py`;
   - `tools/mcp_raw.py`.
4. **The comparator's core branches execute.** Direct fixtures produced `PROMOTE`, `REJECT`, and `SHADOW` under the intended positive, denial-failure, and missing-contract cases.
5. **The older release workspace's tests pass.** `cargo test --workspace` completed all test targets successfully on the inspected clean HEAD.

### Accepted only after correction

1. **Repository ownership:** the dedicated `agent-graph-mcp-release` repository, not the embedded 0.2.3 copy in `agent-graph-release`, is the current MCP owner.
2. **Readiness:** a clean tree is not release certification. The older release repository's strict clippy gate fails with 27 library errors (and 28 in the lib-test target), including MSRV incompatibilities and suspicious file-open behavior.
3. **Evaluation state:** the corpus and comparator are source-present, but no discovered harness executes the six tasks, scores outputs against acceptance/denial criteria, and generates baseline/candidate envelopes. The council's proposed “run the existing corpus” step is therefore incomplete.
4. **Public surface:** `recursiveintell-web` and `stack-showcase` are distribution candidates, not verified deployment or demand evidence.
5. **Performance:** concurrency 16 and a 25% overhead target are not the first gate. Correctness, fail-closed behavior, and reproducibility must precede scale claims.

## First implementation gate

Build the missing **evaluation runner**, not another product subsystem.

It must:

1. Load all six tasks from the held-out corpus.
2. Execute a declared baseline and governed candidate against the same task inputs using a clean immutable runtime candidate.
3. Record runtime commit, model/provider labels, prompt/spec digests, tool versions, timestamps, and raw outputs.
4. Score every acceptance and denial criterion through an explicit, reviewable process. Model judgment alone must not silently become authority.
5. Fail closed on missing tasks, missing scores, malformed output, any candidate denial failure, or incomplete receipts.
6. Emit the exact baseline/candidate envelope consumed by `deterministic_comparator.py`.
7. Preserve raw outputs, scoring decisions, comparator receipt, and rerun instructions.
8. Run from a clean checkout without depending on Josh's live dirty worktree or installed-only behavior.

### Admission sequence

1. Create an isolated clean worktree at the admitted `agent-graph-mcp` revision (initial candidate: `ba9fba1`).
2. Run that revision's compile, test, and strict lint/release gates; do not inherit the older repository's green tests.
3. Add the evaluator as a proof artifact with explicit source lineage for the corpus/comparator.
4. Run baseline and governed candidate on all six tasks.
5. Re-run from a fresh environment or independent checkout.
6. Publish only the evidence-safe result, limitations, and commands.
7. Send it to ten targeted evaluators.

### Pass criteria

- 6/6 tasks executed for both baseline and candidate.
- 6/6 task records have complete raw outputs and criterion decisions.
- Zero candidate denial failures.
- No contract-completeness failure.
- Comparator result is supported by retained inputs and reproducible command output.
- A technically capable outsider reproduces the workflow without modifying source or receiving live handholding.
- At least three of ten targeted evaluators understand a concrete use case; at least one requests a technical follow-up, trial discussion, or interview.

### Fail/stop criteria

Stop expansion and reassess if:

- a clean runtime candidate cannot pass its own gates;
- the evaluator cannot produce complete envelopes within five working days;
- scoring cannot be made explicit and reviewable;
- fail-closed or denial behavior is inconsistent;
- independent reproduction fails;
- targeted outreach produces no concrete comprehension or follow-up signal.

## Rejected or parked directions

For this 7–14 day gate, park:

- broad semantic-memory rewrites;
- recursive-agent expansion;
- new collaboration protocol implementation;
- Gloss UI expansion;
- website redesign before proof;
- 12/16-way concurrency promotion claims;
- performance optimization without an accepted correctness harness;
- “production-ready,” customer, revenue, security-guarantee, or benchmark-superiority claims.

These are not rejected permanently. They are rejected as the next move because their cost-to-proof ratio is worse.

## Final judgment

**ACCEPT WITH CHANGES.** The council correctly identified the highest-leverage strategic move: convert governed-agent infrastructure into one narrow, reproducible proof and test demand. It incorrectly treated a clean older release repository and source-present evaluation assets as closer to an executable product than current evidence supports.

The immediate next direction is therefore:

> **Canonicalize a clean current runtime candidate, implement the missing six-task evaluation runner, produce a reproducible approval/denial evidence bundle, then use that artifact for targeted hiring and pilot outreach.**

Do not start another platform layer until this gate either passes or falsifies the direction.
