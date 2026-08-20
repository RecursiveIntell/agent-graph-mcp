#!/usr/bin/env python3
"""SwarmCompiler — deterministic TaskProfile → topology → GraphSpec compilation.

Implements Field Guide sections 3 (task compilation) and 10 (proposed SwarmCompiler):
  1. Normalize request into a typed TaskProfile (schema recursiveintell.swarm-task-profile.v1).
  2. Classify effect and risk (policy function; profile is authoritative).
  3. Select topology family via the guide's deterministic 11-step decision order.
  4. Apply mandatory-gate eligibility (high risk → S20/S21/S25 wrap; required_gates; forbidden).
  5. Score candidates with the fit(g) heuristic (selector signal, never proof).
  6. Emit a GraphSpec from an approved blueprint template.

Usage:
  swarm_compiler.py compile --profile <task-profile.json> [--catalog <catalog.json>] [--templates <dir>]
  swarm_compiler.py selftest [--catalog <catalog.json>]

Exit codes: 0 = emitted spec (or selftest pass), 2 = ineligible/blocked, 3 = profile invalid.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

PROFILE_SCHEMA = "recursiveintell.swarm-task-profile.v1"

GOAL_TYPES = {"discover", "decide", "create", "execute", "verify", "monitor", "respond"}
DECOMP = {"known", "partial", "emergent"}
COUPLING = {"low", "medium", "high"}
CONTEXT = {"small", "long", "heterogeneous", "streaming", "live"}
EVIDENCE = {"central", "partitioned", "unknown", "contested"}
VERIFIABILITY = {"deterministic", "executable", "rubric", "human", "weak"}
EFFECT = {"none", "read_only", "reversible_write", "irreversible_write", "authority_change"}
RISK = {"low", "medium", "high", "critical"}
REVERSIBILITY = {"easy", "bounded", "difficult", "impossible"}
LATENCY = {"interactive", "minutes", "hours"}
DIVERSITY = {"low", "evidence", "tool", "model", "objective", "adversarial"}
INTERACTION = {"one_shot", "conversational", "event_driven", "long_running"}
FRESHNESS = {"snapshot", "current", "streaming"}

HIGH_RISK = {"high", "critical"}
IRREVERSIBLE = {"irreversible_write", "authority_change"}

# Guide section 3, deterministic decision order, rules 1-10 (rule 11 is a WRAP, applied separately).
SELECTOR_RULES = [
    (1,  lambda p: p["verifiability"] in ("deterministic", "executable") and p["decomposition_visibility"] == "known" and p["goal_type"] in ("verify", "respond") and p["context_shape"] == "small", "S00"),
    (2,  lambda p: p["decomposition_visibility"] == "known" and p["subtask_coupling"] == "high", "S02"),
    (3,  lambda p: p["goal_type"] == "respond" and p["effect_class"] == "none", "S01"),
    (4,  lambda p: p["decomposition_visibility"] == "known" and p["subtask_coupling"] == "low", "S03"),
    (5,  lambda p: p["decomposition_visibility"] == "emergent" and p["subtask_coupling"] in ("medium", "high") and p["risk"] not in HIGH_RISK, "S04"),
    (6,  lambda p: p["context_shape"] == "long" and p["latency_budget"] == "hours", "S05"),
    (7,  lambda p: p["evidence_distribution"] == "partitioned" and p["decomposition_visibility"] in ("partial", "emergent"), "S15"),
    (8,  lambda p: p["goal_type"] == "discover" and p["verifiability"] == "executable" and p["diversity_need"] != ["low"], "S13"),
    (9,  lambda p: p["goal_type"] == "discover" and p["verifiability"] == "rubric", "S10"),
    (10, lambda p: p["evidence_distribution"] == "contested" or p["diversity_need"] == "adversarial", "S09"),
]

# Rule 11: wrap high-risk / irreversible-effect shapes in a gate shape.
def wrap_shape(profile: dict) -> str | None:
    if profile["risk"] in HIGH_RISK or profile["effect_class"] in IRREVERSIBLE:
        # verify/discover audits → S21 (red-blue-purple braid → B06 hostile audit);
        # execute/monitor release paths → S20 (shadow twin / sentinel → B10 release gates).
        return "S21" if profile["goal_type"] in ("verify", "discover") else "S20"
    return None

# Shape → strongest blueprint id from the catalog (first blueprint whose shape list contains the shape).
def build_shape_map(catalog: dict) -> dict[str, str]:
    m: dict[str, str] = {}
    for bp in catalog.get("blueprints", []):
        for s in bp.get("shape", []):
            m.setdefault(s, bp["id"])
    return m

# Blueprints that are materialized as live graphs (registered + executed) or blocked.
MATERIALIZED = {"B00", "B01", "B02", "B04", "B06", "B07", "B10", "B11", "B13", "B14", "B15", "B16", "B19", "B20"}
BLOCKED_REASON = {
    "B03": "catalog: not true web research; needs tool-grounded source witnesses",
    "B05": "catalog: needs scoped leases, sandboxing, typed effects",
    "B08": "catalog: needs sandbox + authenticated approval authority",
    "B09": "catalog: needs effect envelopes, capability checks, remote resume",
    "B12": "catalog: needs tool-grounded benchmark execution",
    "B17": "catalog: needs durable scheduler/event layer",
    "B18": "catalog: approval-gated template unavailable pending operator authority",
    "B21": "catalog: SwarmCompiler tooling, not a graph",
}

GATE_WRAP = {"S20", "S21", "S25"}  # high-risk audit/release/action wraps


def validate_profile(raw: dict) -> dict:
    errs = []
    for field, allowed in [("goal_type", GOAL_TYPES), ("decomposition_visibility", DECOMP),
                           ("subtask_coupling", COUPLING), ("context_shape", CONTEXT),
                           ("evidence_distribution", EVIDENCE), ("verifiability", VERIFIABILITY),
                           ("effect_class", EFFECT), ("risk", RISK), ("reversibility", REVERSIBILITY),
                           ("latency_budget", LATENCY), ("interaction_mode", INTERACTION),
                           ("source_freshness", FRESHNESS)]:
        v = raw.get(field)
        if v not in allowed:
            errs.append(f"{field}: {v!r} not in {sorted(allowed)}")
    if "diversity_need" in raw:
        if not isinstance(raw["diversity_need"], list) or not set(raw["diversity_need"]) <= DIVERSITY:
            errs.append("diversity_need must be a list of: " + ", ".join(sorted(DIVERSITY)))
    else:
        errs.append("diversity_need: required")
    for num in ("latency_budget_ms",):
        if num in raw and not isinstance(raw[num], (int, float)):
            errs.append(f"{num}: must be numeric")
    if errs:
        raise ValueError("invalid task profile: " + "; ".join(errs))
    return raw


def select_shape(profile: dict) -> str:
    """Guide's deterministic decision order; first matching rule wins, else S03 fallback."""
    for _prio, pred, shape in sorted(SELECTOR_RULES):
        if pred(profile):
            return shape
    return "S03"


def eligibility(profile: dict, shape: str, blueprint: str, shape_map: dict) -> tuple[bool, list[str]]:
    notes = []
    if profile["risk"] in HIGH_RISK or profile["effect_class"] in IRREVERSIBLE:
        if shape not in GATE_WRAP and not set(shape_map.get(shape, [])).intersection(GATE_WRAP):
            notes.append(f"high-risk profile: wrap in a gate shape ({sorted(GATE_WRAP)})")
    for g in profile.get("required_gates", []):
        if g == "minority_report" and blueprint not in ("B06", "B13", "B02", "B07", "B11", "B14", "B16"):
            notes.append(f"required gate 'minority_report' unavailable in {blueprint}")
    for f in profile.get("forbidden", []):
        if f == "canonical_source_edits" and profile["effect_class"] not in ("none", "read_only"):
            notes.append("forbidden: canonical_source_edits conflicts with effect_class")
    return (not notes, notes)


def fit_score(profile: dict, shape: str) -> float:
    """fit(g) heuristic — selector signal only, never proof (guide section 3)."""
    s = 0.0
    if shape in ("S00", "S01", "S02", "S03"):
        s += 1.0 if profile["decomposition_visibility"] == "known" else -1.0
    if shape in ("S15", "S19"):
        s += 1.0 if profile["evidence_distribution"] == "partitioned" else -0.5
    if shape in ("S09", "S08"):
        s += 1.5 if profile["evidence_distribution"] == "contested" or "adversarial" in profile.get("diversity_need", []) else 0.0
    if shape in GATE_WRAP:
        s += 2.0 if profile["risk"] in HIGH_RISK else -0.5
    if profile["verifiability"] == "executable" and shape in ("S03", "S13", "S14"):
        s += 1.0
    if profile["effect_class"] in IRREVERSIBLE and shape not in GATE_WRAP:
        s -= 2.0
    return s


def blueprint_for(shape: str, shape_map: dict) -> str | None:
    return shape_map.get(shape)


def emit_spec(profile: dict, blueprint: str, template_dir: Path | None) -> dict:
    """Emit a ready-to-register GraphSpec from the approved blueprint template."""
    if template_dir is None:
        return {"status": "template_required", "blueprint": blueprint}
    matches = list(template_dir.glob(f"*{blueprint}*.json"))
    # Prefer the materialized swarm spec, else any matching template.
    matches = sorted(matches, key=lambda p: 0 if "swarm-" in p.name else 1)
    if not matches:
        return {"status": "no_template", "blueprint": blueprint}
    spec = json.loads(matches[0].read_text())
    rid = re.sub(r"[^A-Za-z0-9._-]", "-", profile.get("request_id", "req"))[:48]
    spec["name"] = f"{spec.get('name', blueprint)}-{rid}"
    budgets = profile.get("budgets", {})
    if budgets:
        spec.setdefault("budgets", {}).update(budgets)
    return spec


PROMOTION_STATES = ["draft", "static_validated", "offline_evaluated", "shadow", "canary", "default",
                    "deprecated", "quarantined", "superseded"]
EFFECT_CLASS_POLICY = {
    "none": {"mutation": False, "human_authority": False},
    "read_only": {"mutation": False, "human_authority": False},
    "reversible_write": {"mutation": True, "human_authority": False, "rollback_required": True},
    "irreversible_write": {"mutation": True, "human_authority": True, "rollback_required": True},
    "authority_change": {"mutation": True, "human_authority": True, "rollback_required": True},
}
EFFECT_ENVELOPE_SCHEMA = "recursiveintell.effect-envelope.v1"
REQUIRED_ENVELOPE_FIELDS = ["effect_id", "request_id", "actor", "target", "operation", "arguments_digest",
                            "capability", "idempotency_key", "preconditions", "postconditions",
                            "retry_policy", "rollback"]


def promotion_check(candidate: dict) -> tuple[bool, list[str]]:
    """Guide section 12: promotion requires all eight conditions; state transitions are ordered."""
    notes = []
    state = candidate.get("state")
    if state not in PROMOTION_STATES:
        notes.append(f"unknown promotion state {state!r}")
    if state in ("deprecated", "quarantined", "superseded"):
        return (False, notes + ["terminal state; no further promotion"])
    for field, label in [("corpus_labeled", "held-out task corpus with task-profile labels"),
                         ("versions_recorded", "current model/provider/tool versions recorded"),
                         ("baseline_exists", "single-agent and simpler-topology baselines"),
                         ("cost_within_budget", "cost and latency within budget"),
                         ("rollback_available", "rollback to previous template version")]:
        if not candidate.get(field):
            notes.append(f"missing: {label}")
    if candidate.get("denial_regression"):
        notes.append("denial-test regression present")
    if not candidate.get("benefit"):
        notes.append("no statistically and practically meaningful benefit shown")
    if candidate.get("public_claim_supported") is False:
        notes.append("unsupported public readiness claim")
    return (not notes, notes)


def effect_check(envelope: dict, effect_class: str | None = None) -> tuple[bool, list[str]]:
    """Guide section 13: mandatory effect envelope + class policy (contract layer only)."""
    notes = []
    if envelope.get("schema_version") != EFFECT_ENVELOPE_SCHEMA:
        notes.append(f"schema_version must be {EFFECT_ENVELOPE_SCHEMA}")
    for f in REQUIRED_ENVELOPE_FIELDS:
        if f not in envelope or envelope[f] in (None, "", []):
            notes.append(f"missing field: {f}")
    tgt = envelope.get("target") or {}
    for f in ("type", "id", "ref"):
        if f not in tgt:
            notes.append(f"target missing {f}")
    cap = envelope.get("capability") or {}
    for f in ("id", "scope", "expires_at", "one_shot", "revocable"):
        if f not in cap:
            notes.append(f"capability missing {f}")
    if envelope.get("retry_policy") == "manual_on_ambiguous":
        pass  # guide: ambiguous external response is terminal ambiguity, not retry permission
    elif envelope.get("retry_policy"):
        notes.append("retry_policy must be manual_on_ambiguous for external effects")
    if effect_class is not None:
        pol = EFFECT_CLASS_POLICY.get(effect_class)
        if pol is None:
            notes.append(f"unknown effect_class {effect_class!r}")
        else:
            if pol["mutation"] and envelope.get("operation", "").startswith(("read", "search", "fetch", "inspect")):
                notes.append(f"effect_class {effect_class} forbids read-only operation label")
            if pol.get("rollback_required") and not (envelope.get("rollback") or {}).get("tested"):
                notes.append("rollback required and must be tested")
            if pol["human_authority"] and not envelope.get("human_authority"):
                notes.append("irreversible/authority_change effects require human authority")
    if not (envelope.get("rollback") or {}).get("operation") and envelope.get("operation"):
        notes.append("rollback operation missing")
    return (not notes, notes)


def compile_profile(profile: dict, catalog: dict, template_dir: Path | None) -> dict:
    base_shape = select_shape(profile)
    wrap = wrap_shape(profile)
    shape = wrap or base_shape  # blueprint lookup uses the gate-wrapped shape
    shape_map = build_shape_map(catalog)
    bp = blueprint_for(shape, shape_map)
    eligible, notes = eligibility(profile, shape, bp or "", shape_map)
    score = fit_score(profile, shape)
    if wrap:
        notes.append(f"high-risk profile: base {base_shape} wrapped in {wrap} (guide rule 11)")
    result = {
        "schema_version": PROFILE_SCHEMA,
        "base_shape": base_shape,
        "wrap": wrap,
        "shape": shape,
        "blueprint": bp,
        "materialized": bp in MATERIALIZED,
        "blocked_reason": BLOCKED_REASON.get(bp or ""),
        "fit_signal": round(score, 2),
        "eligibility_notes": notes,
        "eligible": eligible,
    }
    if eligible and bp and bp in MATERIALIZED:
        result["spec"] = emit_spec(profile, bp, template_dir)
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description="SwarmCompiler (guide sections 3+10+12+13)")
    ap.add_argument("mode", choices=["compile", "promotion", "effect", "selftest"])
    ap.add_argument("--profile", type=Path)
    ap.add_argument("--candidate", type=Path, help="promotion-check candidate JSON (section 12)")
    ap.add_argument("--envelope", type=Path, help="effect-envelope JSON (section 13)")
    ap.add_argument("--effect-class", default=None, help="effect class policy to enforce")
    ap.add_argument("--catalog", type=Path, default=Path("RecursiveIntell_Swarm_Blueprint_Catalog_v1.json"))
    ap.add_argument("--templates", type=Path, default=Path("/tmp/swarm-blueprints"))
    args = ap.parse_args()

    catalog_path = args.catalog if args.catalog.exists() else Path("/home/sikmindz/Downloads/swarm-bundle") / args.catalog.name
    catalog = json.loads(catalog_path.read_text())

    if args.mode == "selftest":
        return selftest(catalog)

    if args.mode == "promotion":
        if not args.candidate:
            print("promotion requires --candidate <candidate.json>", file=sys.stderr)
            return 3
        cand = json.loads(args.candidate.read_text())
        ok, notes = promotion_check(cand)
        print(json.dumps({"state": cand.get("state"), "eligible": ok, "notes": notes}, indent=2))
        return 0 if ok else 2

    if args.mode == "effect":
        if not args.envelope:
            print("effect requires --envelope <envelope.json>", file=sys.stderr)
            return 3
        env = json.loads(args.envelope.read_text())
        ok, notes = effect_check(env, args.effect_class)
        print(json.dumps({"schema": EFFECT_ENVELOPE_SCHEMA, "valid": ok, "notes": notes}, indent=2))
        return 0 if ok else 2

    if not args.profile:
        print("compile requires --profile <task-profile.json>", file=sys.stderr)
        return 3
    try:
        profile = validate_profile(json.loads(args.profile.read_text()))
    except (ValueError, json.JSONDecodeError) as e:
        print(f"profile error: {e}", file=sys.stderr)
        return 3
    result = compile_profile(profile, catalog, args.templates)
    print(json.dumps(result, indent=2))
    return 0 if result["eligible"] and result.get("spec") else 2


def selftest(catalog: dict) -> int:
    cases = [
        # verify, small, executable → S00 (bounded agent + validator)
        ({"request_id": "req_verify_1", "goal_type": "verify", "decomposition_visibility": "known",
          "subtask_coupling": "low", "context_shape": "small", "evidence_distribution": "central",
          "verifiability": "executable", "effect_class": "read_only", "risk": "low",
          "reversibility": "easy", "latency_budget": "interactive", "cost_budget": "low",
          "diversity_need": ["low"], "interaction_mode": "one_shot", "source_freshness": "snapshot"}, "S00"),
        # contested tradeoffs + adversarial → S09 (double diamond)
        ({"request_id": "req_decide_1", "goal_type": "decide", "decomposition_visibility": "partial",
          "subtask_coupling": "medium", "context_shape": "heterogeneous", "evidence_distribution": "contested",
          "verifiability": "rubric", "effect_class": "none", "risk": "medium", "reversibility": "bounded",
          "latency_budget": "minutes", "cost_budget": "medium", "diversity_need": ["adversarial", "evidence"],
          "interaction_mode": "one_shot", "source_freshness": "current"}, "S09"),
        # partitioned evidence, partial decomposition → S15 (blackboard)
        ({"request_id": "req_discover_1", "goal_type": "discover", "decomposition_visibility": "partial",
          "subtask_coupling": "low", "context_shape": "heterogeneous", "evidence_distribution": "partitioned",
          "verifiability": "rubric", "effect_class": "read_only", "risk": "medium", "reversibility": "easy",
          "latency_budget": "minutes", "cost_budget": "medium", "diversity_need": ["evidence", "tool"],
          "interaction_mode": "one_shot", "source_freshness": "snapshot"}, "S15"),
        # high-risk audit → S21 (red-blue-purple braid / gate wrap)
        ({"request_id": "req_audit_1", "goal_type": "verify", "decomposition_visibility": "partial",
          "subtask_coupling": "low", "context_shape": "heterogeneous", "evidence_distribution": "partitioned",
          "verifiability": "executable", "effect_class": "read_only", "risk": "critical", "reversibility": "bounded",
          "latency_budget": "hours", "cost_budget": "high", "diversity_need": ["evidence", "adversarial"],
          "interaction_mode": "one_shot", "source_freshness": "snapshot",
          "required_gates": ["source_preflight", "reproduction", "minority_report"],
          "forbidden": ["canonical_source_edits"]}, "S21"),
        # combinatorial discover + executable → S13 (note: loop unsupported in runtime)
        ({"request_id": "req_search_1", "goal_type": "discover", "decomposition_visibility": "emergent",
          "subtask_coupling": "low", "context_shape": "small", "evidence_distribution": "central",
          "verifiability": "executable", "effect_class": "read_only", "risk": "medium", "reversibility": "easy",
          "latency_budget": "hours", "cost_budget": "high", "diversity_need": ["objective"],
          "interaction_mode": "one_shot", "source_freshness": "snapshot"}, "S13"),
    ]
    failures = 0
    for profile, want in cases:
        try:
            r = compile_profile(validate_profile(dict(profile)), catalog, Path("/tmp/swarm-blueprints"))
        except ValueError as e:
            print(f"FAIL {profile['request_id']}: profile rejected ({e})")
            failures += 1
            continue
        got = r["shape"]
        status = "ok" if got == want else "FAIL"
        if got != want:
            failures += 1
        print(f"[{status}] {profile['request_id']}: shape {got} (want {want}) wrap={r['wrap']}")
    # compile-level check for the audit profile (eligibility + spec emission)
    audit = json.loads(json.dumps(cases[3][0]))
    r = compile_profile(audit, catalog, Path("/tmp/swarm-blueprints"))
    if not r["eligible"] or not r.get("spec"):
        print(f"FAIL audit compile: eligible={r['eligible']} notes={r['eligibility_notes']}")
        failures += 1
    else:
        print(f"[ok] audit compile -> {r['blueprint']} (fit {r['fit_signal']})")
    # section 12: promotion gate (shadow candidate missing corpus/rollback must fail; complete passes)
    promo_good = {"state": "shadow", "corpus_labeled": True, "versions_recorded": True, "baseline_exists": True,
                  "cost_within_budget": True, "rollback_available": True, "denial_regression": False,
                  "benefit": True, "public_claim_supported": True}
    promo_bad = {**promo_good, "corpus_labeled": False, "rollback_available": False}
    if not promotion_check(promo_good)[0] or promotion_check(promo_bad)[0]:
        print("FAIL promotion gate")
        failures += 1
    else:
        print("[ok] promotion gate (good passes, incomplete candidate blocked)")
    # section 13: effect envelope (complete reversible envelope passes; missing capability fails; authority_change without human authority fails)
    env_good = {"schema_version": "recursiveintell.effect-envelope.v1", "effect_id": "eff_1", "request_id": "req_1",
                "actor": "executor", "target": {"type": "repository", "id": "owner/repo", "ref": "branch"},
                "operation": "push_commit", "arguments_digest": "sha256:00", "capability": {"id": "cap_1", "scope": "one_branch",
                "expires_at": "2026-08-07T00:00:00Z", "one_shot": True, "revocable": True},
                "idempotency_key": "idem_1", "preconditions": ["certification_receipt:x"], "postconditions": ["remote_ref_matches_expected"],
                "retry_policy": "manual_on_ambiguous", "rollback": {"operation": "restore_ref", "tested": True}}
    env_bad = {**env_good, "schema_version": "wrong", "capability": {}, "human_authority": False}
    if not effect_check(env_good, "reversible_write")[0] or effect_check(env_bad, "authority_change")[0]:
        print("FAIL effect envelope")
        failures += 1
    else:
        print("[ok] effect envelope (valid passes; malformed + missing authority blocked)")
    print(f"selftest: {'PASS' if failures == 0 else f'{failures} FAILURES'}")
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
