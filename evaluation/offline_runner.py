from __future__ import annotations

import argparse
import hashlib
import json
import time
from pathlib import Path
from typing import Any

from .adjudicate import adjudicate_text
from .compare import compare
from .contract import validate_corpus, validate_envelope


def _digest(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()


def _fixture_text(task: dict[str, Any], configuration: str) -> str:
    acceptance = task["acceptance_criteria"]
    if configuration == "baseline":
        text = f"Initial assessment: {acceptance[0]}. Further evidence and verification remain required."
        if task["id"] == "t2":
            text += " The missing successful-run evidence is not recorded."
        elif task["id"] == "t5":
            text += " Separate pre-existing warnings from release blockers."
        elif task["id"] == "t6":
            text += " Require a pilot experiment before any swap."
        return text
    return "Decision-grade fixture report. " + " ".join(acceptance) + ". Separate verified facts from inference; record unknowns, contradiction handling, rollback, and the next executable verification gate."


def run_fixture(corpus_path: str | Path, output: str | Path, trials: int = 3) -> dict[str, Any]:
    corpus_path = Path(corpus_path)
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    corpus = json.loads(corpus_path.read_text())
    corpus_report = validate_corpus(corpus)
    if not corpus_report["valid"]:
        raise ValueError(corpus_report)
    envelopes: list[dict[str, Any]] = []
    raw_records: list[dict[str, Any]] = []
    envelope_path = output / "offline-envelopes.jsonl"
    raw_path = output / "offline-raw.jsonl"
    envelope_path.write_text("")
    raw_path.write_text("")
    for task in corpus["tasks"]:
        for configuration in ("baseline", "candidate"):
            for trial in range(1, trials + 1):
                text = _fixture_text(task, configuration)
                outcome = adjudicate_text(task, text)
                envelope = {
                    "schema": "recursiveintell.offline-envelope.v1",
                    "evidence_class": "synthetic_fixture_only",
                    "task_id": task["id"], "configuration": configuration, "trial": trial,
                    "raw_response": {"transport": "none", "graph_calls": 0, "provider_calls": 0, "fixture": "deterministic-v1"},
                    "receipt": {"schema": "offline-fixture-receipt-v1", "status": "synthetic", "text_sha256": _digest(text), "run_id": f"offline-{task['id']}-{configuration}-{trial}"},
                    "outcome": {**outcome, "text": text, "status": "synthetic"},
                }
                report = validate_envelope(envelope)
                if not report["valid"]:
                    raise ValueError(report)
                raw = {"task_id": task["id"], "configuration": configuration, "trial": trial, "request": task, "response": envelope["raw_response"], "receipt": envelope["receipt"]}
                envelopes.append(envelope)
                raw_records.append(raw)
                with envelope_path.open("a") as handle:
                    handle.write(json.dumps(envelope, sort_keys=True) + "\n")
                with raw_path.open("a") as handle:
                    handle.write(json.dumps(raw, sort_keys=True) + "\n")
    comparison = compare(envelope_path)
    manifest = {
        "schema": "recursiveintell.offline-proof-run.v1", "evidence_class": "synthetic_fixture_only", "created_at_unix": time.time(),
        "corpus": str(corpus_path), "corpus_sha256": _digest(corpus_path.read_text()), "trials": trials,
        "envelopes": len(envelopes), "live_graph_calls": 0, "provider_calls": 0,
        "comparison": comparison, "artifacts": [str(envelope_path), str(raw_path)],
    }
    (output / "offline-manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    (output / "offline-comparison.json").write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", default="evaluation/corpus/held_out_audit_v1.json")
    parser.add_argument("--output", required=True)
    parser.add_argument("--trials", type=int, default=3)
    args = parser.parse_args()
    result = run_fixture(args.corpus, args.output, args.trials)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
