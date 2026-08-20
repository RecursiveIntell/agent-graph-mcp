import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT.parent))

from evaluation.offline_runner import run_fixture


def test_offline_fixture_is_explicitly_non_live(tmp_path):
    result = run_fixture(ROOT / "corpus" / "held_out_audit_v1.json", tmp_path, trials=2)
    assert result["schema"] == "recursiveintell.offline-proof-run.v1"
    assert result["evidence_class"] == "synthetic_fixture_only"
    assert result["live_graph_calls"] == 0
    assert result["provider_calls"] == 0
    assert result["envelopes"] == 24


def test_offline_fixture_has_replayable_raw_records(tmp_path):
    result = run_fixture(ROOT / "corpus" / "held_out_audit_v1.json", tmp_path, trials=1)
    envelope_lines = (tmp_path / "offline-envelopes.jsonl").read_text().splitlines()
    raw_lines = (tmp_path / "offline-raw.jsonl").read_text().splitlines()
    assert len(envelope_lines) == len(raw_lines) == 12
    assert all(json.loads(line)["receipt"]["schema"] == "offline-fixture-receipt-v1" for line in envelope_lines)
    assert result["comparison"]["sample"]["tasks_paired"] == 6
