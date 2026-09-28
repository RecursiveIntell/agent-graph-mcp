"""MCP boundary regressions for verified terminal artifact paging."""
import unittest

from support import GraphFixture


class ArtifactPagingBoundaryTests(unittest.TestCase):
    def _fixture_run(self, fixture):
        run_id = fixture.run("é")
        first = fixture.call(
            "graph_run_artifact",
            {"run_id": run_id, "artifact": "output", "offset": 0, "limit": 16384},
        )
        return run_id, first

    def _continuation(self, run_id, first, *, offset, limit):
        return {
            "run_id": run_id,
            "artifact": "output",
            "offset": offset,
            "limit": limit,
            "expected_digest": first["digest"],
            "expected_artifact_id": first["artifact_id"],
        }

    def _assert_error(self, fixture, args, code):
        reply = fixture.raw("graph_run_artifact", args)
        self.assertIs(reply.get("ok"), False, reply)
        self.assertEqual(reply.get("error_code"), code, reply)

    def test_offset_inside_utf8_codepoint_is_invalid(self):
        with GraphFixture() as fixture:
            run_id, first = self._fixture_run(fixture)
            encoded = first["data"].encode("utf-8")
            codepoint = encoded.index("é".encode("utf-8"))
            self.assertGreaterEqual(codepoint, 0)
            self._assert_error(
                fixture,
                self._continuation(run_id, first, offset=codepoint + 1, limit=8),
                "INVALID_OFFSET",
            )

    def test_offsets_past_artifact_and_u64_max_are_invalid(self):
        with GraphFixture() as fixture:
            run_id, first = self._fixture_run(fixture)
            for offset in (first["total_bytes"] + 1, (1 << 64) - 1):
                with self.subTest(offset=offset):
                    self._assert_error(
                        fixture,
                        self._continuation(run_id, first, offset=offset, limit=8),
                        "INVALID_OFFSET",
                    )

    def test_page_limits_zero_and_above_maximum_are_invalid(self):
        with GraphFixture() as fixture:
            run_id = fixture.run("é")
            for limit in (0, 16385):
                with self.subTest(limit=limit):
                    self._assert_error(
                        fixture,
                        {
                            "run_id": run_id,
                            "artifact": "output",
                            "offset": 0,
                            "limit": limit,
                        },
                        "INVALID_LIMIT",
                    )

    def test_too_small_page_rejects_without_zero_progress(self):
        with GraphFixture() as fixture:
            run_id, first = self._fixture_run(fixture)
            encoded = first["data"].encode("utf-8")
            codepoint = encoded.index("é".encode("utf-8"))
            reply = fixture.raw(
                "graph_run_artifact",
                self._continuation(run_id, first, offset=codepoint, limit=1),
            )
            self.assertIs(reply.get("ok"), False, reply)
            self.assertEqual(reply.get("error_code"), "LIMIT_TOO_SMALL", reply)
            self.assertIsNone(reply.get("data"), "no page bytes may accompany failure")


if __name__ == "__main__":
    unittest.main()
