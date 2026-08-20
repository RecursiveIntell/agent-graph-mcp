#!/usr/bin/env python3
"""Deterministic comparison engine for agent-graph corpus trials.

Implements the deep-research report's statistical program:
  - Paired per-task scores from barrier envelopes
  - Quality noninferiority (lower-bound CI vs margin)
  - Denial-failure hard gates (any runner violation = fail)
  - Composite verdict: PROMOTE / SHADOW / REJECT

Usage:
  deterministic_comparator.py --baseline '{"scores":{...},"denial_failures":[...]}' \
                              --candidate '{"scores":{...},"denial_failures":[...]}' \
                              --noninferiority-margin -0.02
"""
import argparse
import json
import math
import random
import sys
from statistics import mean


def bootstrap_ci_paired(
    baseline_scores: list[float],
    candidate_scores: list[float],
    n_bootstrap: int = 2000,
    alpha: float = 0.05,
) -> tuple[float, float]:
    """Cluster-bootstrap per-task paired delta CI (percentile)."""
    tasks = list(zip(baseline_scores, candidate_scores))
    n = len(tasks)
    deltas = []
    for _ in range(n_bootstrap):
        sample = [tasks[random.randrange(n)] for _ in range(n)]
        d = mean(c - b for b, c in sample)
        deltas.append(d)
    deltas.sort()
    lo = deltas[int(alpha / 2 * n_bootstrap)]
    hi = deltas[int((1 - alpha / 2) * n_bootstrap)]
    return lo, hi


def compute_comparison(
    baseline: dict,
    candidate: dict,
    noninferiority_margin: float = -0.02,
    n_bootstrap: int = 2000,
) -> dict:
    """Produce a comparison receipt from two runner envelopes."""
    b_scores = baseline.get("scores") or {}
    c_scores = candidate.get("scores") or {}
    b_denials = baseline.get("denial_failures") or []
    c_denials = candidate.get("denial_failures") or []

    # Collect paired scores where both runners have a numeric value
    paired = []
    missing_tasks = []
    for task_id in set(list(b_scores.keys()) + list(c_scores.keys())):
        b = b_scores.get(task_id)
        c = c_scores.get(task_id)
        if isinstance(b, (int, float)) and isinstance(c, (int, float)):
            paired.append((task_id, b, c))
        else:
            missing_tasks.append(task_id)

    n_paired = len(paired)
    b_vals = [b for _, b, _ in paired]
    c_vals = [c for _, _, c in paired]

    mean_b = mean(b_vals) if b_vals else None
    mean_c = mean(c_vals) if c_vals else None
    delta = round((mean_c - mean_b), 6) if mean_b is not None and mean_c is not None else None

    ci_lo, ci_hi = (None, None)
    if n_paired >= 3 and b_vals and c_vals:
        random.seed(42)  # reproducible bootstrap
        ci_lo, ci_hi = bootstrap_ci_paired(b_vals, c_vals, n_bootstrap)

    # Hard gates
    denial_hard_fail = bool(b_denials or c_denials)
    contract_fail = bool(missing_tasks)

    # Noninferiority gate
    quality_noninf = True
    if ci_lo is not None:
        quality_noninf = ci_lo >= noninferiority_margin

    # Composite verdict
    if contract_fail:
        verdict = "SHADOW"
        reason = f"contract failure: {len(missing_tasks)} task(s) missing scores in one or both runners"
    elif denial_hard_fail:
        verdict = "REJECT"
        reason = f"denial-test failure: baseline={b_denials}, candidate={c_denials}"
    elif not quality_noninf:
        verdict = "SHADOW"
        reason = f"quality noninferiority failed: lower CI {ci_lo:.4f} < margin {noninferiority_margin}"
    elif delta is not None and delta > 0:
        verdict = "PROMOTE"
        reason = f"quality noninferior + positive delta {delta:.4f} (CI [{ci_lo:.4f},{ci_hi:.4f}])"
    else:
        verdict = "SHADOW"
        reason = f"quality noninferior but non-positive delta {delta} (CI [{ci_lo:.4f},{ci_hi:.4f}])"

    return {
        "schema": "recursiveintell.comparison-receipt.v1",
        "sample": {
            "tasks_total": len(b_scores.keys() | c_scores.keys()),
            "tasks_paired": n_paired,
            "tasks_missing": sorted(missing_tasks),
        },
        "quality": {
            "baseline_mean": mean_b,
            "candidate_mean": mean_c,
            "paired_delta": delta,
            "confidence_interval_95": [round(ci_lo, 6) if ci_lo is not None else None,
                                       round(ci_hi, 6) if ci_hi is not None else None],
            "bootstrap_iterations": n_bootstrap if ci_lo is not None else 0,
            "noninferiority_margin": noninferiority_margin,
        },
        "integrity": {
            "denial_hard_fail": denial_hard_fail,
            "baseline_denials": b_denials,
            "candidate_denials": c_denials,
            "contract_fail": contract_fail,
        },
        "gate_results": [
            {"gate": "quality-noninferiority", "result": "pass" if quality_noninf else "fail"},
            {"gate": "denial-hard-fail", "result": "fail" if denial_hard_fail else "pass"},
            {"gate": "contract-completeness", "result": "fail" if contract_fail else "pass"},
        ],
        "verdict": verdict,
        "reason": reason,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", required=True, help="JSON: {scores, denial_failures}")
    ap.add_argument("--candidate", required=True, help="JSON: {scores, denial_failures}")
    ap.add_argument("--noninferiority-margin", type=float, default=-0.02)
    ap.add_argument("--bootstrap", type=int, default=2000)
    ap.add_argument("--label", default="", help="human label for the receipt")
    args = ap.parse_args()

    baseline = json.loads(args.baseline)
    candidate = json.loads(args.candidate)
    result = compute_comparison(
        baseline, candidate,
        noninferiority_margin=args.noninferiority_margin,
        n_bootstrap=args.bootstrap,
    )
    result["label"] = args.label
    json.dump(result, sys.stdout, indent=2)
    print()
    return 0 if result["verdict"] != "SHADOW" else 1


if __name__ == "__main__":
    sys.exit(main())
