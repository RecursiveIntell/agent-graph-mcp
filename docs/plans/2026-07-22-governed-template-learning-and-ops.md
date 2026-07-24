# Governed Agent Graph Template Learning and Operations Plan

> **For Hermes:** Implement as local policy projections and edge hooks; do not widen Agent Graph runtime authority.

**Goal:** Make known-good templates reusable, let low-risk/easy work explore candidates, and retain an auditable operator-controlled promotion path.

**Architecture:** `~/.hermes/agent-graph-ops/template-policy.sqlite` is a rebuildable selection projection. Agent Graph SQLite receipts, witnesses, checkpoints, and approvals remain canonical runtime evidence. Hooks validate/select only at the policy boundary and write reference-only observations; they never mutate graph specs, fetch sources, approve actions, or resume execution.

**Learning contract:**
- `candidate`: discovered after successful `graph_create`; never selected for hard work.
- `approved`: promoted only by a named operator recording a positive outcome tied to a terminal receipt digest.
- `retired`: never selected.
- Hard/critical work: approved only, >=3 positive outcomes, zero known negative outcomes in the task family; otherwise return no recommendation.
- Easy work: epsilon-style exploration is permitted only for `candidate`/`approved` templates marked `non_effectful`; every run needs supported wall/node budgets and an explicit later outcome label.
- Neither receipt existence, model output, source-witness capture, nor graph completion constitutes quality evidence.

## Tasks

### 1. Template policy registry
**Files:** `~/.hermes/scripts/agent_graph_ops.py`; test fixture commands.

- Create SQLite tables for template descriptors, observed graph creations, outcome records, and receipt pointers.
- Implement `register`, `recommend`, `record-outcome`, `observe`, `watchdog`, and `approval-inbox` commands.
- Require canonical template JSON digest, named task family, difficulty/risk labels, and receipt digest for outcomes.
- `recommend --difficulty hard` returns only approved evidence-qualified templates; `--difficulty easy` may return one bounded non-effectful candidate for exploration.
- **Rollback:** delete only `~/.hermes/agent-graph-ops/`; it contains projections, never canonical graph data.

### 2. Operations skills
**Files:** user-local skills `agent-graph-operations`, `graph-source-witnessing`.

- Add preflight, template recommendation, budget, receipt, approval, and outcome-recording procedures.
- State explicit non-claims and rejected capabilities.
- Link all template selection to policy CLI output and operator outcome collection.

### 3. Hooks
**Files:** `~/.hermes/agent-hooks/agent-graph-guard.py`, `~/.hermes/agent-hooks/agent-graph-observer.py`.

- Guard only direct policy violations: reject unsupported `max_llm_calls`, prohibit approval decisions without actor/decision, and require nonempty template family/difficulty labels for planned experiment requests.
- Observer captures successful graph-create descriptors and receipt pointers; it fails open and writes no source/state/prompt contents.
- Configure using backup-backed YAML list entries and test direct hook wire payloads.

### 4. Issue-only operational checks
**Files:** `~/.hermes/scripts/agent_graph_watchdog.py`, `~/.hermes/scripts/agent_graph_approval_inbox.py`; Hermes cron registry.

- Report only integrity-key permission drift, SQLite integrity errors, stale/interrupted runs, expired/pending approvals, and unreviewed candidate templates.
- Empty stdout means no user notification. No automatic repairs or decisions.

### 5. Gates
- Script self-test on a temporary policy store.
- Direct guard/observer JSON-wire tests.
- `hermes hooks doctor`; config YAML parse/type check.
- `hermes mcp test agent_graph` and live gateway status.
- Cron jobs listed with issue-only `no_agent` scripts.

**Rollback:** restore `~/.hermes/backups/agent-graph-activation-20260722T071250Z/config.yaml.before` if hook configuration destabilizes Hermes; remove created cron jobs by their discovered IDs; retain canonical Agent Graph SQLite and integrity key.