"""Regression coverage for private-fixture graph start idempotency and reads."""
import json
import time
import unittest
import uuid

from support import GraphFixture


class GraphRunStartIdempotencyTests(unittest.TestCase):
    def _create_start_request(self, fixture, payload):
        graph_id = "lane18-" + uuid.uuid4().hex
        spec = {
            "name": graph_id,
            "entry": "copy-input",
            "output_key": "result",
            "max_iterations": 3,
            "nodes": [
                {
                    "id": "copy-input",
                    "type": "state_transform",
                    "config": {
                        "operations": [
                            {"op": "copy", "from": "__input__", "path": "result"}
                        ]
                    },
                }
            ],
            "edges": [{"from": "copy-input", "to": "END"}],
        }
        created = fixture.call("graph_create", {"spec": spec})
        self.assertEqual(created["graph_id"], graph_id)
        request = {
            "graph_id": graph_id,
            "graph_version": created["graph_version"],
            "input": payload,
            "budgets": {"max_nodes": 3, "max_wall_clock_ms": 10000},
            "idempotency_key": "lane18-key-" + uuid.uuid4().hex,
        }
        return graph_id, request

    def _start_and_wait(self, fixture, request):
        started = fixture.call("graph_run_start", request)
        run_id = started["run_id"]
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            status = fixture.call("graph_run_get", {"run_id": run_id, "compact": True})
            if (
                status.get("status") in {"completed", "failed", "cancelled"}
                and status.get("persistence_status") == "durable_terminal"
            ):
                self.assertEqual(status.get("status"), "completed", status)
                return run_id
            time.sleep(0.02)
        self.fail("fixture run did not reach a durable terminal state")

    @staticmethod
    def _execution_count(fixture, graph_id):
        connection = fixture.db()
        try:
            return connection.execute(
                "SELECT COUNT(*) FROM executions WHERE graph_name = ?", (graph_id,)
            ).fetchone()[0]
        finally:
            connection.close()

    def test_same_explicit_start_material_reuses_run_without_new_execution(self):
        with GraphFixture() as fixture:
            graph_id, request = self._create_start_request(fixture, {"value": "stable"})
            first_run_id = self._start_and_wait(fixture, request)
            self.assertEqual(self._execution_count(fixture, graph_id), 1)

            repeated = fixture.call("graph_run_start", request)

            self.assertEqual(repeated["run_id"], first_run_id)
            self.assertEqual(self._execution_count(fixture, graph_id), 1)

    def test_paged_output_read_does_not_start_another_run(self):
        payload = {"text": "paged-artifact-" * 30}
        with GraphFixture() as fixture:
            graph_id, request = self._create_start_request(fixture, payload)
            run_id = self._start_and_wait(fixture, request)
            self.assertEqual(self._execution_count(fixture, graph_id), 1)

            pages = fixture.pages(run_id, artifact="output", limit=17)

            self.assertGreater(len(pages), 1)
            self.assertTrue(pages[-1]["done"])
            output = json.loads("".join(page["data"] for page in pages))
            self.assertEqual(output, payload)
            self.assertEqual(self._execution_count(fixture, graph_id), 1)


if __name__ == "__main__":
    unittest.main()
