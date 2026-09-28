"""Fail-closed regressions for the graph_run_artifact MCP endpoint.

All daemon state is created by the private, disposable GraphFixture harness.
The deterministic graph is a fixture and does not represent live-provider evidence.
"""
import unittest
import asyncio

from support import GraphFixture


class GraphArtifactEndpointTests(unittest.TestCase):
    def test_unknown_run_is_reported_as_not_found(self):
        with GraphFixture() as fixture:
            reply = fixture.raw(
                "graph_run_artifact",
                {"run_id": "missing-lane07-run", "artifact": "receipt"},
            )

        self.assertFalse(reply.get("ok"), reply)
        self.assertEqual(reply.get("error_code"), "RUN_NOT_FOUND", reply)
        self.assertIsNone(reply.get("data"))

    def _assert_parameter_rejected(self, args):
        async def probe(fixture):
            async with fixture.session() as session:
                result = await session.call_tool("graph_run_artifact", args)
                self.assertTrue(result.is_error)
                raw = result.model_dump(mode="json", exclude_none=True)
                self.assertFalse(raw.get("structured_content") or raw.get("structuredContent"))
        with GraphFixture() as fixture:
            asyncio.run(probe(fixture))

    def test_unknown_artifact_enum_is_rejected(self):
        self._assert_parameter_rejected({"run_id":"missing-lane07-run", "artifact":"debug_dump"})

    def test_unknown_parameter_is_rejected(self):
        self._assert_parameter_rejected({"run_id":"missing-lane07-run", "artifact":"receipt", "include_unverified":True})

    def test_keyless_fixture_cannot_return_verified_artifact_bytes(self):
        with GraphFixture(keyed=False) as fixture:
            run_id = fixture.run({"lane": "keyless-fixture"})
            reply = fixture.raw(
                "graph_run_artifact",
                {"run_id": run_id, "artifact": "receipt"},
            )

        self.assertFalse(reply.get("ok"), reply)
        self.assertEqual(reply.get("error_code"), "INTEGRITY_KEY_REQUIRED", reply)
        self.assertIsNone(reply.get("data"))
        self.assertNotIn("bytes", reply)


if __name__ == "__main__":
    unittest.main()
