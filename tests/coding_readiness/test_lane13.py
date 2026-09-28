"""Regression tests for graph_run_client terminal and coding-output admission."""
import asyncio
import hashlib
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest
from unittest import mock


CLIENT_PATH = pathlib.Path(__file__).resolve().parents[2] / "scripts" / "graph_run_client.py"
SPEC = importlib.util.spec_from_file_location("lane13_graph_run_client", CLIENT_PATH)
client = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = client
SPEC.loader.exec_module(client)


class TerminalAdmissionTests(unittest.TestCase):
    def setUp(self):
        from reader_unit_transport import stub_session_context
        stub_session_context(self, client.RunReader)

    def test_terminal_execution_is_not_admitted_while_persistence_is_pending_or_volatile(self):
        for status_name in ("completed", "failed", "cancelled"):
            for persistence in ("pending", "volatile_persistence_failed"):
                with self.subTest(status=status_name, persistence=persistence):
                    self.assertFalse(client.durably_terminal({
                        "status": status_name,
                        "persistence_status": persistence,
                    }))

    def test_durable_terminal_statuses_are_admitted_including_failed(self):
        for status_name in ("completed", "failed", "cancelled", "interrupted_non_resumable"):
            with self.subTest(status=status_name):
                self.assertTrue(client.durably_terminal({
                    "status": status_name,
                    "persistence_status": "durable_terminal",
                }))

    def test_running_and_unknown_status_are_never_terminal_even_when_durable(self):
        for status_name in ("running", "queued", "future_status", None):
            with self.subTest(status=status_name):
                self.assertFalse(client.durably_terminal({
                    "status": status_name,
                    "persistence_status": "durable_terminal",
                }))

    def test_failed_durable_run_never_admits_coding_output_even_if_success_flag_is_true(self):
        status = {
            "projection": "compact",
            "run_id": "fixture-failed",
            "status": "failed",
            "persistence_status": "durable_terminal",
            "success": True,
        }

        async def transport(_reader, tool, args):
            if tool == "graph_run_get":
                return status
            self.assertEqual(tool, "graph_run_artifact")
            self.assertIn(args["artifact"], ("receipt", "bundle"))
            data = json.dumps({"fixture": args["artifact"]}, separators=(",", ":"))
            encoded = data.encode("utf-8")
            return {
                "schema": "agent-graph-artifact-page-v1",
                "encoding": "json-utf8",
                "run_id": args["run_id"],
                "graph_version": "fixture-v1",
                "artifact": args["artifact"],
                "offset": args["offset"],
                "digest": "sha256:" + hashlib.sha256(encoded).hexdigest(),
                "artifact_id": "fixture-" + args["artifact"],
                "total_bytes": len(encoded),
                "data": data,
                "done": True,
                "next_offset": None,
            }

        with tempfile.TemporaryDirectory(prefix="lane13-client-") as directory:
            out = pathlib.Path(directory) / "read-output"
            with mock.patch.object(client.RunReader, "_read_once", transport), mock.patch.object(
                sys, "argv", ["graph_run_client.py", "--socket", "private-fixture", "--run-id",
                               "fixture-failed", "--out", str(out)],
            ):
                asyncio.run(client.main())

            self.assertTrue((out / "receipt.json").is_file())
            self.assertTrue((out / "bundle.json").is_file())
            self.assertFalse((out / "output.json").exists())
            receipt = json.loads((out / "READ_RECEIPT.json").read_text())
            self.assertEqual(set(receipt["artifacts"]), {"receipt", "bundle"})
            self.assertTrue(receipt["complete"])


if __name__ == "__main__":
    unittest.main()
