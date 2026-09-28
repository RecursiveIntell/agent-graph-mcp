"""Regression coverage for compact and legacy run status projections."""
import unittest

from support import GraphFixture


IDENTITY_FIELDS = ("run_id", "graph_id", "graph_version")
COMPACT_FIELDS = {
    "run_id",
    "graph_id",
    "graph_version",
    "status",
    "success",
    "storage_class",
    "persistence_status",
    "budget_counters",
    "budget_exhausted",
    "replay_capability",
    "error_code",
    "projection",
}
PAYLOAD_FIELDS = {
    "input",
    "state",
    "final_state",
    "steps",
    "receipt",
    "bundle",
    "error",
    "persistence_error",
}


class CompactRunStatusTests(unittest.TestCase):
    def test_compact_and_legacy_status_match_without_changing_legacy_shape(self):
        with GraphFixture() as fixture:
            run_id = fixture.run({"answer": 42})
            compact = fixture.call(
                "graph_run_get", {"run_id": run_id, "compact": True}
            )
            legacy = fixture.call("graph_run_get", {"run_id": run_id})

            self.assertEqual(compact["projection"], "compact")
            self.assertLessEqual(set(compact), COMPACT_FIELDS)
            self.assertTrue(PAYLOAD_FIELDS.isdisjoint(compact))
            self.assertEqual(compact["status"], "completed")
            self.assertIs(compact["success"], True)
            self.assertEqual(compact["persistence_status"], "durable_terminal")
            for field in IDENTITY_FIELDS:
                self.assertEqual(compact[field], legacy[field], field)
            self.assertEqual(compact["status"], legacy["status"])

            # The default call remains the full legacy projection for a live run.
            self.assertTrue(
                {"trace", "state", "final_state", "steps", "receipt"}
                .issubset(legacy)
            )
            self.assertEqual(legacy["state"]["value0"], {"answer": 42})
            self.assertTrue(legacy["steps"])
            self.assertIsInstance(legacy["receipt"], dict)

    def test_compact_terminal_status_survives_private_daemon_restart(self):
        with GraphFixture() as fixture:
            run_id = fixture.run({"persisted": "yes"})
            before = fixture.call(
                "graph_run_get", {"run_id": run_id, "compact": True}
            )

            fixture.restart()
            after = fixture.call(
                "graph_run_get", {"run_id": run_id, "compact": True}
            )

            self.assertEqual(after["projection"], "compact")
            self.assertLessEqual(set(after), COMPACT_FIELDS)
            self.assertTrue(PAYLOAD_FIELDS.isdisjoint(after))
            for field in IDENTITY_FIELDS:
                self.assertEqual(after[field], before[field], field)
            self.assertEqual(after["status"], "completed")
            self.assertEqual(after["status"], before["status"])
            self.assertIs(after["success"], True)
            self.assertEqual(after["persistence_status"], "durable_terminal")


if __name__ == "__main__":
    unittest.main()
