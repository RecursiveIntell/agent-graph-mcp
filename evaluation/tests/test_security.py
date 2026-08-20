import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT.parent))

from evaluation.compare import compare
from evaluation.contract import validate_envelope
from evaluation.replay import replay_envelopes


def test_contract_rejects_non_object_envelope():
    report = validate_envelope([])
    assert report["valid"] is False
    assert "envelope must be an object" in report["errors"]


def test_offline_envelope_requires_synthetic_evidence_class():
    envelope = {
        "schema": "recursiveintell.offline-envelope.v1",
        "task_id": "t1", "configuration": "candidate", "trial": 1,
        "raw_response": {}, "receipt": {"schema": "offline-fixture-receipt-v1"}, "outcome": {},
    }
    report = validate_envelope(envelope)
    assert report["valid"] is False
    assert "offline envelopes must declare synthetic_fixture_only evidence" in report["errors"]


def test_comparison_hard_fails_denial_failures(tmp_path):
    record = {
        "schema": "recursiveintell.runner-envelope.v1", "task_id": "t1", "configuration": "baseline", "trial": 1,
        "raw_response": {}, "receipt": {"schema": "agent-graph-mcp-receipt-v1"},
        "outcome": {"score": 1.0, "denial_failures": ["unsafe"]},
    }
    pair = dict(record)
    pair["configuration"] = "candidate"
    path = tmp_path / "envelopes.jsonl"
    path.write_text(json.dumps(record) + "\n" + json.dumps(pair) + "\n")
    result = compare(path)
    assert result["verdict"] == "REJECT"
    assert result["integrity"]["denial_hard_fail"] is True


def test_replay_detects_tampered_line(tmp_path):
    source = tmp_path / "source.jsonl"
    source.write_text('{"a":1}\n')
    receipt = replay_envelopes(source)
    source.write_text('{"a":2}\n')
    assert receipt["source_sha256"] != replay_envelopes(source)["source_sha256"]
