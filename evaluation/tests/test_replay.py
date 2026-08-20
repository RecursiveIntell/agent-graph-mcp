import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT.parent))

from evaluation.replay import replay_envelopes


def test_replay_is_deterministic_and_preserves_hashes(tmp_path):
    envelope = {
        "schema": "recursiveintell.runner-envelope.v1",
        "task_id": "t1", "configuration": "baseline", "trial": 1,
        "raw_response": {"x": 1},
        "receipt": {"schema": "agent-graph-mcp-receipt-v2", "status": "completed"},
        "outcome": {"text": "verify before deletion", "score": 0.5, "denial_failures": []},
    }
    source = tmp_path / "envelopes.jsonl"
    source.write_text(json.dumps(envelope) + "\n")
    first = replay_envelopes(source)
    second = replay_envelopes(source)
    assert first == second
    assert first["source_sha256"].startswith("sha256:")
    assert first["valid"] is True
