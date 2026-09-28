"""Regression coverage for oversized legacy run status responses."""
import asyncio
import json
import unittest

from support import GraphFixture


LARGE_VALUE = "x" * 30_000


class OversizedLegacyRunGetTests(unittest.TestCase):
    def test_oversized_legacy_response_then_compact_get_on_same_session(self):
        with GraphFixture() as fixture:
            run_id = fixture.run(LARGE_VALUE, copies=12)

            async def exercise_same_session():
                async with fixture.session() as session:
                    try:
                        await fixture.raw_async(session, "graph_run_get", {"run_id": run_id})
                    except Exception as error:
                        self.assertIn("RESPONSE_TOO_LARGE", str(error))
                    else:
                        self.fail("oversized legacy response must return the correlated transport error")

                    compact = await fixture.raw_async(
                        session,
                        "graph_run_get",
                        {"run_id": run_id, "compact": True},
                    )
                    self.assertTrue(compact["ok"], compact)
                    self.assertEqual(compact["data"]["projection"], "compact")
                    self.assertEqual(compact["data"]["run_id"], run_id)
                    self.assertEqual(compact["data"]["status"], "completed")

            asyncio.run(exercise_same_session())

    def test_canonical_output_pages_reassemble_to_the_complete_output(self):
        with GraphFixture() as fixture:
            run_id = fixture.run(LARGE_VALUE, copies=12)

            async def read_pages_on_one_session():
                pages = []
                offset = 0
                identity = None
                async with fixture.session() as session:
                    while True:
                        args = {
                            "run_id": run_id,
                            "artifact": "output",
                            "offset": offset,
                            "limit": 16_384,
                        }
                        if identity is not None:
                            args["expected_digest"] = identity["digest"]
                            args["expected_artifact_id"] = identity["artifact_id"]
                        reply = await fixture.raw_async(
                            session, "graph_run_artifact", args
                        )
                        self.assertTrue(reply["ok"], reply)
                        page = reply["data"]
                        self.assertEqual(page["schema"], "agent-graph-artifact-page-v1")
                        self.assertEqual(page["run_id"], run_id)
                        self.assertEqual(page["artifact"], "output")
                        self.assertEqual(page["offset"], offset)
                        self.assertEqual(page["encoding"], "json-utf8")
                        current_identity = {
                            key: page[key]
                            for key in (
                                "run_id",
                                "graph_version",
                                "artifact",
                                "encoding",
                                "digest",
                                "artifact_id",
                                "total_bytes",
                            )
                        }
                        if identity is None:
                            identity = current_identity
                        else:
                            self.assertEqual(current_identity, identity)
                        pages.append(page)
                        if page["done"]:
                            self.assertIsNone(page["next_offset"])
                            break
                        offset = page["next_offset"]
                        self.assertGreater(offset, page["offset"])

                return pages

            pages = asyncio.run(read_pages_on_one_session())
            self.assertGreater(len(pages), 1)
            self.assertEqual(pages[-1]["offset"] + len(pages[-1]["data"].encode("utf-8")),
                             pages[0]["total_bytes"])
            serialized_output = "".join(page["data"] for page in pages)
            self.assertEqual(json.loads(serialized_output), LARGE_VALUE)


if __name__ == "__main__":
    unittest.main()
