"""Regression tests for hostile artifact-page metadata in graph_run_client."""
import importlib.util
import pathlib
import sys
import unittest


CLIENT_PATH = pathlib.Path(__file__).resolve().parents[2] / "scripts" / "graph_run_client.py"
SPEC = importlib.util.spec_from_file_location("lane10_graph_run_client", CLIENT_PATH)
client = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = client
SPEC.loader.exec_module(client)


RUN_ID = "run-fixture"
ARTIFACT = "output"
IDENTITY = {
    "run_id": RUN_ID,
    "graph_version": "v1",
    "artifact": ARTIFACT,
    "encoding": "json-utf8",
    "digest": "sha256:fixture",
    "artifact_id": "artifact-fixture",
    "total_bytes": 2,
}


def page(**overrides):
    result = {
        "schema": "agent-graph-artifact-page-v1",
        "encoding": "json-utf8",
        "run_id": RUN_ID,
        "graph_version": IDENTITY["graph_version"],
        "artifact": ARTIFACT,
        "digest": IDENTITY["digest"],
        "artifact_id": IDENTITY["artifact_id"],
        "total_bytes": 2,
        "offset": 0,
        "data": "x",
        "done": False,
        "next_offset": 1,
    }
    result.update(overrides)
    return result


class ValidatePageRegressionTests(unittest.TestCase):
    def rejects(self, candidate, *, offset=0, max_bytes=16):
        with self.assertRaises(client.ReadError):
            client.validate_page(candidate, RUN_ID, ARTIFACT, offset, None, 2, max_bytes)

    def test_offset_and_next_offset_must_be_strict_integers(self):
        # bool and float values can compare equal to an integer in Python.
        for candidate, offset in (
            (page(offset=True, total_bytes=3, next_offset=2), 1),
            (page(offset=1.0, total_bytes=3, next_offset=2), 1),
            (page(next_offset=True), 0),
            (page(next_offset=1.0), 0),
        ):
            with self.subTest(offset=offset, page_offset=candidate["offset"]):
                self.rejects(candidate, offset=offset)

    def test_negative_fractional_and_oversized_total_lengths_are_rejected(self):
        for total in (-1, 1.5, 17):
            with self.subTest(total_bytes=total):
                self.rejects(page(total_bytes=total), max_bytes=16)

    def test_empty_nonfinal_page_cannot_claim_progress(self):
        self.rejects(page(data="", next_offset=0))

    def test_invalid_completion_metadata_is_a_read_error(self):
        self.rejects(page(next_offset=10**100))
        self.rejects(page(done=1))
        self.rejects(page(done=True, next_offset=1, total_bytes=1, data="x"))


if __name__ == "__main__":
    unittest.main()
