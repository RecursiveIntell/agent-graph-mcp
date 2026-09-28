"""Lane05 regressions for fail-closed terminal artifact HMAC verification.

These tests use only GraphFixture's disposable private daemon and SQLite store.
The graph is a deterministic state-transform fixture and makes no provider call.
"""
import json
import unittest

from support import GraphFixture


class TerminalArtifactIntegrityTests(unittest.TestCase):
    def _assert_tamper_is_rejected(self, column, artifact, error_code, mutate):
        with GraphFixture() as fixture:
            run_id = fixture.run({"fixture_value": "original"})
            db = fixture.db()
            try:
                row = db.execute(
                    f"SELECT {column} FROM terminal_receipts WHERE run_id = ?",
                    (run_id,),
                ).fetchone()
                self.assertIsNotNone(row, "fixture run must have a durable receipt")
                original = row[0]
                changed = json.loads(original)
                mutate(changed)
                db.execute(
                    f"UPDATE terminal_receipts SET {column} = ? WHERE run_id = ?",
                    (json.dumps(changed, separators=(",", ":")), run_id),
                )
                db.commit()

                reply = fixture.raw(
                    "graph_run_artifact",
                    {"run_id": run_id, "artifact": artifact, "offset": 0, "limit": 16384},
                )
                self.assertFalse(reply["ok"], reply)
                self.assertEqual(reply["error_code"], error_code, reply)
                self.assertIsNone(reply.get("data"), "integrity failure must return no page bytes")

                # Restore the byte-for-byte stored value before checking that
                # the same verified read succeeds again.
                db.execute(
                    f"UPDATE terminal_receipts SET {column} = ? WHERE run_id = ?",
                    (original, run_id),
                )
                db.commit()
            finally:
                db.close()

            pages = fixture.pages(run_id, artifact=artifact)
            self.assertTrue(pages)
            self.assertTrue(all(page["data"] for page in pages))
            recovered = json.loads("".join(page["data"] for page in pages))
            if artifact == "receipt":
                self.assertEqual(recovered["run_id"], run_id)
            else:
                self.assertEqual(recovered["payload"]["run_id"], run_id)

    def test_corrupt_receipt_hmac_withholds_receipt_page_and_restores(self):
        self._assert_tamper_is_rejected(
            "receipt_json",
            "receipt",
            "RECEIPT_INTEGRITY_FAILURE",
            lambda receipt: receipt.__setitem__("fixture_tamper", True),
        )

    def test_corrupt_bundle_hmac_withholds_bundle_page_and_restores(self):
        self._assert_tamper_is_rejected(
            "bundle_json",
            "bundle",
            "BUNDLE_INTEGRITY_FAILURE",
            lambda bundle: bundle["payload"].__setitem__("fixture_tamper", True),
        )


if __name__ == "__main__":
    unittest.main()
