import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT.parent))

from evaluation.contract import validate_corpus, validate_envelope


def test_corpus_requires_six_tasks_and_rubrics():
    corpus = json.loads((ROOT / "corpus" / "held_out_audit_v1.json").read_text())
    report = validate_corpus(corpus)
    assert report["valid"] is True
    assert report["tasks"] == 6
    assert report["errors"] == []


def test_envelope_requires_raw_receipt_and_typed_outcome():
    envelope = {
        "schema": "recursiveintell.runner-envelope.v1",
        "task_id": "t1",
        "configuration": "baseline",
        "trial": 1,
        "raw_response": {"result": {"data": {"run_id": "r1"}}},
        "receipt": {"schema": "agent-graph-mcp-receipt-v2", "status": "completed"},
        "outcome": {"text": "answer", "score": 0.8, "denial_failures": []},
    }
    assert validate_envelope(envelope)["valid"] is True


def test_envelope_rejects_missing_receipt():
    envelope = {
        "schema": "recursiveintell.runner-envelope.v1",
        "task_id": "t1",
        "configuration": "baseline",
        "trial": 1,
        "raw_response": {},
        "outcome": {"text": "answer", "score": 0.8, "denial_failures": []},
    }
    report = validate_envelope(envelope)
    assert report["valid"] is False
    assert "receipt" in " ".join(report["errors"])
