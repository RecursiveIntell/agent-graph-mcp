"""Regression coverage for failed-run status and artifact projection."""
import json
import unittest

from support import GraphFixture


class FailedGraphProjectionTests(unittest.TestCase):
    def make_budget_failed_run(self, fixture):
        return fixture.run(
            {"marker": "ingress-only"},
            copies=2,
            budgets={"max_nodes": 1},
        )

    def read_json_artifact(self, fixture, run_id, artifact):
        return json.loads("".join(page["data"] for page in fixture.pages(run_id, artifact)))

    def test_compact_status_keeps_budget_failure_and_is_payload_free(self):
        with GraphFixture() as fixture:
            run_id = self.make_budget_failed_run(fixture)

            status = fixture.call("graph_run_get", {"run_id": run_id, "compact": True})

            self.assertEqual(status["projection"], "compact")
            self.assertEqual(status["status"], "failed")
            self.assertIs(status["success"], False)
            self.assertEqual(status["error_code"], "BUDGET_EXHAUSTED")
            self.assertEqual(status["budget_exhausted"], "max_nodes")
            self.assertEqual(status["budget_counters"]["nodes"], 1)
            self.assertEqual(status["persistence_status"], "durable_terminal")
            self.assertNotIn("final_state", status)
            self.assertNotIn("input", status)

    def test_receipt_is_readable_but_missing_declared_output_stays_unavailable(self):
        with GraphFixture() as fixture:
            run_id = self.make_budget_failed_run(fixture)

            receipt = self.read_json_artifact(fixture, run_id, "receipt")
            self.assertEqual(receipt["error_code"], "BUDGET_EXHAUSTED")
            self.assertEqual(receipt["terminal_output"]["state_key"], "value1")

            output = fixture.raw(
                "graph_run_artifact",
                {"run_id": run_id, "artifact": "output", "offset": 0, "limit": 16384},
            )
            self.assertIs(output["ok"], False)
            self.assertEqual(output["error_code"], "ARTIFACT_UNAVAILABLE")

    def test_restart_preserves_failed_compact_projection_and_receipt(self):
        with GraphFixture() as fixture:
            run_id = self.make_budget_failed_run(fixture)
            fixture.restart()

            status = fixture.call("graph_run_get", {"run_id": run_id, "compact": True})
            receipt = self.read_json_artifact(fixture, run_id, "receipt")

            self.assertEqual(status["status"], "failed")
            self.assertIs(status["success"], False)
            self.assertEqual(status["error_code"], "BUDGET_EXHAUSTED")
            self.assertEqual(status["budget_exhausted"], "max_nodes")
            self.assertEqual(status["persistence_status"], "durable_terminal")
            self.assertEqual(receipt["run_id"], run_id)
            self.assertEqual(receipt["error_code"], "BUDGET_EXHAUSTED")


if __name__ == "__main__":
    unittest.main()
