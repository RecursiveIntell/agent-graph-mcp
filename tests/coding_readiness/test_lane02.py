"""Regression tests for MCP artifact continuation identity guards."""
import unittest

from support import GraphFixture


class ArtifactContinuationTests(unittest.TestCase):
    def _first_page(self, fixture, run_id):
        reply = fixture.raw(
            "graph_run_artifact",
            {"run_id": run_id, "artifact": "output", "offset": 0, "limit": 8},
        )
        self.assertTrue(reply.get("ok"), reply)
        return reply["data"]

    def _continuation(self, fixture, run_id, page, **overrides):
        args = {
            "run_id": run_id,
            "artifact": "output",
            "offset": page["next_offset"],
            "limit": 8,
        }
        args.update(overrides)
        return fixture.raw("graph_run_artifact", args)

    def test_continuation_requires_digest(self):
        with GraphFixture() as fixture:
            run_id = fixture.run("continuation-guard-value-" + "x" * 80)
            first = self._first_page(fixture, run_id)
            self.assertFalse(first["done"])
            reply = self._continuation(
                fixture,
                run_id,
                first,
                expected_artifact_id=first["artifact_id"],
            )
            self.assertFalse(reply["ok"])
            self.assertEqual(reply["error_code"], "ARTIFACT_IDENTITY_REQUIRED")

    def test_continuation_requires_artifact_id(self):
        with GraphFixture() as fixture:
            run_id = fixture.run("continuation-guard-value-" + "x" * 80)
            first = self._first_page(fixture, run_id)
            self.assertFalse(first["done"])
            reply = self._continuation(
                fixture, run_id, first, expected_digest=first["digest"]
            )
            self.assertFalse(reply["ok"])
            self.assertEqual(reply["error_code"], "ARTIFACT_IDENTITY_REQUIRED")

    def test_continuation_rejects_wrong_digest(self):
        with GraphFixture() as fixture:
            run_id = fixture.run("continuation-guard-value-" + "x" * 80)
            first = self._first_page(fixture, run_id)
            self.assertFalse(first["done"])
            reply = self._continuation(
                fixture,
                run_id,
                first,
                expected_digest="sha256:" + "0" * 64,
                expected_artifact_id=first["artifact_id"],
            )
            self.assertFalse(reply["ok"])
            self.assertEqual(reply["error_code"], "ARTIFACT_DIGEST_MISMATCH")

    def test_continuation_rejects_wrong_artifact_identity(self):
        with GraphFixture() as fixture:
            run_id = fixture.run("continuation-guard-value-" + "x" * 80)
            first = self._first_page(fixture, run_id)
            self.assertFalse(first["done"])
            reply = self._continuation(
                fixture,
                run_id,
                first,
                expected_digest=first["digest"],
                expected_artifact_id="sha256:" + "0" * 64,
            )
            self.assertFalse(reply["ok"])
            self.assertEqual(reply["error_code"], "ARTIFACT_ID_MISMATCH")

    def test_equal_output_bytes_from_distinct_runs_cannot_share_continuation(self):
        with GraphFixture() as fixture:
            value = "same-output-bytes-" + "x" * 80
            run_a = fixture.run(value)
            run_b = fixture.run(value)
            first_a = self._first_page(fixture, run_a)
            first_b = self._first_page(fixture, run_b)

            self.assertEqual(first_a["digest"], first_b["digest"])
            self.assertNotEqual(first_a["artifact_id"], first_b["artifact_id"])
            reply = self._continuation(
                fixture,
                run_b,
                first_a,
                expected_digest=first_a["digest"],
                expected_artifact_id=first_a["artifact_id"],
            )
            self.assertFalse(reply["ok"])
            self.assertEqual(reply["error_code"], "ARTIFACT_ID_MISMATCH")


if __name__ == "__main__":
    unittest.main()
