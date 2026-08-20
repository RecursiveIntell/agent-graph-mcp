from __future__ import annotations

import argparse
import json
import random
import statistics
from pathlib import Path
from typing import Any


def bootstrap(deltas: list[float], n: int = 2000) -> list[float]:
    random.seed(42)
    return [statistics.mean(random.choice(deltas) for _ in deltas) for _ in range(n)]


def compare(path: str | Path, margin: float = -0.02) -> dict[str, Any]:
    envelopes = [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for e in envelopes:
        grouped.setdefault((e["task_id"], e["configuration"]), []).append(e)
    baseline = {task: statistics.mean(e["outcome"]["score"] for e in grouped.get((task, "baseline"), [])) for task, _ in grouped if (task, "baseline") in grouped}
    candidate = {task: statistics.mean(e["outcome"]["score"] for e in grouped.get((task, "candidate"), [])) for task, _ in grouped if (task, "candidate") in grouped}
    task_ids = sorted(set(baseline) | set(candidate))
    paired = [(t, baseline[t], candidate[t]) for t in task_ids if t in baseline and t in candidate]
    missing = sorted(set(task_ids) - {t for t, _, _ in paired})
    deltas = [c - b for _, b, c in paired]
    denial_failures = [e for e in envelopes if e["outcome"].get("denial_failures")]
    ci = [None, None]
    if len(deltas) >= 3:
        samples = sorted(bootstrap(deltas))
        ci = [samples[50], samples[1949]]
    mean_b = statistics.mean([b for _, b, _ in paired]) if paired else None
    mean_c = statistics.mean([c for _, _, c in paired]) if paired else None
    delta = mean_c - mean_b if mean_b is not None and mean_c is not None else None
    contract_fail = bool(missing)
    denial_fail = bool(denial_failures)
    quality_noninf = ci[0] is None or ci[0] >= margin
    if contract_fail:
        verdict, reason = "SHADOW", f"missing paired tasks: {missing}"
    elif denial_fail:
        verdict, reason = "REJECT", f"denial failures: {len(denial_failures)}"
    elif not quality_noninf:
        verdict, reason = "SHADOW", f"lower confidence bound {ci[0]} below margin {margin}"
    elif delta is not None and delta > 0:
        verdict, reason = "PROMOTE", f"positive paired delta {delta:.4f}"
    else:
        verdict, reason = "SHADOW", f"non-positive paired delta {delta}"
    return {"schema": "recursiveintell.comparison-receipt.v1", "sample": {"tasks_total": len(task_ids), "tasks_paired": len(paired), "tasks_missing": missing}, "quality": {"baseline_mean": mean_b, "candidate_mean": mean_c, "paired_delta": delta, "confidence_interval_95": ci, "bootstrap_iterations": 2000 if deltas else 0, "noninferiority_margin": margin}, "integrity": {"denial_hard_fail": denial_fail, "denial_count": len(denial_failures), "contract_fail": contract_fail}, "verdict": verdict, "reason": reason}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("envelopes")
    parser.add_argument("--margin", type=float, default=-0.02)
    parser.add_argument("--output")
    args = parser.parse_args()
    result = compare(args.envelopes, args.margin)
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        Path(args.output).write_text(text)
    else:
        print(text, end="")
    return 0 if result["verdict"] == "PROMOTE" else 1


if __name__ == "__main__":
    raise SystemExit(main())
