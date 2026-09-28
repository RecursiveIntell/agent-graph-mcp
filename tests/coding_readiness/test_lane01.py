"""Regression coverage for real MCP terminal artifact pages."""
import hashlib
import unittest

from support import GraphFixture


class ArtifactPageRegressionTests(unittest.TestCase):
    def test_utf8_artifact_reconstructs_across_16384_byte_boundary(self):
        # The repeated multibyte characters exercise UTF-8-safe page endings,
        # while the complete serialized output is larger than one MCP page.
        payload = {"text": "a" * 16378 + "é🙂終" + "b" * 64}
        with GraphFixture() as fixture:
            run_id = fixture.run(payload)
            pages = fixture.pages(run_id, limit=16384)

            self.assertGreaterEqual(len(pages), 2)
            self.assertEqual([page["offset"] for page in pages][0], 0)
            self.assertTrue(all(len(page["data"].encode("utf-8")) <= 16384 for page in pages))
            self.assertTrue(all(page["encoding"] == "json-utf8" for page in pages))

            reconstructed = "".join(page["data"] for page in pages).encode("utf-8")
            self.assertGreater(len(reconstructed), 16384)
            self.assertEqual(len(reconstructed), pages[0]["total_bytes"])
            self.assertEqual(
                pages[0]["digest"],
                "sha256:" + hashlib.sha256(reconstructed).hexdigest(),
            )
            self.assertEqual(pages[-1]["offset"] + len(pages[-1]["data"].encode("utf-8")), len(reconstructed))
            self.assertTrue(pages[-1]["done"])

    def test_read_at_artifact_eof_returns_empty_final_page(self):
        with GraphFixture() as fixture:
            run_id = fixture.run({"message": "end-of-file"})
            first = fixture.call(
                "graph_run_artifact",
                {"run_id": run_id, "artifact": "output", "offset": 0, "limit": 16384},
            )
            eof = fixture.call(
                "graph_run_artifact",
                {
                    "run_id": run_id,
                    "artifact": "output",
                    "offset": first["total_bytes"],
                    "limit": 16384,
                    "expected_digest": first["digest"],
                    "expected_artifact_id": first["artifact_id"],
                },
            )

            self.assertEqual(eof["offset"], first["total_bytes"])
            self.assertEqual(eof["data"], "")
            self.assertTrue(eof["done"])
            self.assertIsNone(eof["next_offset"])
            self.assertEqual(eof["digest"], first["digest"])
            self.assertEqual(eof["artifact_id"], first["artifact_id"])


if __name__ == "__main__":
    unittest.main()
