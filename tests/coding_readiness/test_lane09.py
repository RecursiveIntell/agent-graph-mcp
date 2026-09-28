"""Focused regression tests for read-only RunReader transport retries."""
import asyncio
import importlib.util
import pathlib
import sys
import unittest


CLIENT_PATH = pathlib.Path(__file__).resolve().parents[2] / "scripts" / "graph_run_client.py"
SPEC = importlib.util.spec_from_file_location("lane09_graph_run_client", CLIENT_PATH)
assert SPEC is not None and SPEC.loader is not None
CLIENT = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CLIENT
SPEC.loader.exec_module(CLIENT)


class RunReaderRetryTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        from reader_unit_transport import stub_session_context
        stub_session_context(self, CLIENT.RunReader)

    async def test_transient_oserror_retries_same_read_once_without_start(self):
        reader = CLIENT.RunReader("unused-private-socket", proxy="unused-proxy")
        calls = []
        expected_args = {"run_id": "run-fixture-1", "compact": True}

        async def transport(tool, args):
            calls.append((tool, args.copy()))
            if len(calls) == 1:
                raise OSError("temporary transport interruption")
            return {"run_id": "run-fixture-1", "projection": "compact"}

        reader._read_once = transport
        result = await reader.read("graph_run_get", expected_args)

        self.assertEqual(result["run_id"], "run-fixture-1")
        self.assertEqual(calls, [("graph_run_get", expected_args), ("graph_run_get", expected_args)])
        self.assertNotIn("graph_run_start", [tool for tool, _ in calls])
        self.assertEqual(reader.reconnections, 1)

    async def test_second_transport_failure_surfaces_after_one_retry(self):
        reader = CLIENT.RunReader("unused-private-socket", proxy="unused-proxy")
        calls = []
        first_error = OSError("first transport interruption")
        second_error = OSError("second transport interruption")

        async def transport(tool, args):
            calls.append((tool, args.copy()))
            raise first_error if len(calls) == 1 else second_error

        reader._read_once = transport
        args = {"run_id": "run-fixture-2"}
        with self.assertRaises(OSError) as raised:
            await reader.read("graph_run_artifact", args)

        self.assertIs(raised.exception, second_error)
        self.assertEqual(calls, [("graph_run_artifact", args), ("graph_run_artifact", args)])
        self.assertNotIn("graph_run_start", [tool for tool, _ in calls])
        self.assertEqual(reader.reconnections, 1)

    async def test_start_tool_is_rejected_without_transport_call(self):
        reader = CLIENT.RunReader("unused-private-socket", proxy="unused-proxy")
        calls = []

        async def transport(tool, args):
            calls.append((tool, args))
            return None

        reader._read_once = transport
        with self.assertRaisesRegex(CLIENT.ReadError, "READ_ONLY_METHOD_REQUIRED"):
            await reader.read("graph_run_start", {"graph_id": "must-not-dispatch"})

        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
