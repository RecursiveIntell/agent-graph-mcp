"""Regression coverage for reading one artifact across a private owner restart."""
import asyncio
import importlib.util
import json
import pathlib
import sys
import unittest

from support import GraphFixture, PROXY


CLIENT_PATH = pathlib.Path(__file__).resolve().parents[2] / "scripts" / "graph_run_client.py"
CLIENT_SPEC = importlib.util.spec_from_file_location("lane06_graph_run_client", CLIENT_PATH)
if CLIENT_SPEC is None or CLIENT_SPEC.loader is None:
    raise RuntimeError(f"cannot load graph run client from {CLIENT_PATH}")
CLIENT = importlib.util.module_from_spec(CLIENT_SPEC)
sys.modules[CLIENT_SPEC.name] = CLIENT
CLIENT_SPEC.loader.exec_module(CLIENT)


def execution_row_count(fixture):
    with fixture.db() as connection:
        return connection.execute("SELECT COUNT(*) FROM executions").fetchone()[0]


class RestartAfterFirstPageReader(CLIENT.RunReader):
    """Inject a real private-owner restart after one successful real read."""

    def __init__(self, fixture):
        super().__init__(fixture.socket, proxy=PROXY)
        self.fixture = fixture
        self.artifact_requests = []
        self.did_restart = False

    async def read(self, tool, args):
        result = await super().read(tool, args)
        if tool == "graph_run_artifact":
            self.artifact_requests.append(dict(args))
            if not self.did_restart:
                self.did_restart = True
                self.fixture.restart()
        return result


class Lane06ReadinessRegression(unittest.TestCase):
    def test_artifact_continuation_survives_private_owner_restart_without_execution(self):
        with GraphFixture() as fixture:
            run_id = fixture.run({"message": "stable payload", "items": list(range(20))})
            baseline_pages = fixture.pages(run_id, artifact="bundle", limit=64)
            self.assertGreater(len(baseline_pages), 1, "fixture artifact must require continuation pages")
            expected_bytes = "".join(page["data"] for page in baseline_pages).encode("utf-8")
            expected_digest = baseline_pages[0]["digest"]
            expected_artifact_id = baseline_pages[0]["artifact_id"]
            before = execution_row_count(fixture)

            reader = RestartAfterFirstPageReader(fixture)
            data, identity = asyncio.run(reader.artifact(run_id, "bundle", limit=64))

            self.assertTrue(reader.did_restart, "owner restart must occur after the first returned page")
            self.assertGreater(len(reader.artifact_requests), 1, "reader must continue across pages")
            self.assertEqual(reader.artifact_requests[0]["offset"], 0)
            self.assertGreater(reader.artifact_requests[1]["offset"], 0)
            self.assertEqual(reader.artifact_requests[1]["expected_digest"], expected_digest)
            self.assertEqual(reader.artifact_requests[1]["expected_artifact_id"], expected_artifact_id)
            self.assertEqual(data, expected_bytes, "reconstructed bytes must match the canonical fixture artifact")
            self.assertEqual(identity["digest"], expected_digest)
            self.assertEqual(identity["artifact_id"], expected_artifact_id)
            self.assertEqual(json.loads(data), json.loads(expected_bytes))
            self.assertEqual(execution_row_count(fixture), before, "read/reconnect/restart must not add executions")


class Lane06LostResponseTests(unittest.TestCase):
    def test_lost_response_retries_same_real_read_without_new_execution(self):
        with GraphFixture() as fixture:
            run_id = fixture.run({"proof": "lost read response", "text": "λ" * 100})
            before = execution_row_count(fixture)
            expected = "".join(p["data"] for p in fixture.pages(run_id, "output", 64)).encode()

            class DropOneResponse(CLIENT.RunReader):
                def __init__(self):
                    super().__init__(fixture.socket, proxy=PROXY)
                    self.requests = []
                    self.dropped = False

                async def _read_once(self, tool, args):
                    self.requests.append((tool, dict(args)))
                    value = await super()._read_once(tool, args)
                    if not self.dropped:
                        self.dropped = True
                        raise OSError("injected loss after real owner read, before caller delivery")
                    return value

            reader = DropOneResponse()
            data, _ = asyncio.run(reader.artifact(run_id, "output", limit=64))
            self.assertEqual(data, expected)
            self.assertEqual(reader.reconnections, 1)
            self.assertEqual(reader.requests[0], reader.requests[1])
            self.assertTrue(all(tool == "graph_run_artifact" for tool, _ in reader.requests))
            self.assertEqual(execution_row_count(fixture), before)


class Lane06ReaderUnitTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        from reader_unit_transport import stub_session_context
        stub_session_context(self, CLIENT.RunReader)

    async def test_transport_retry_repeats_the_same_read_without_execution_authority(self):
        class OneDropReader(CLIENT.RunReader):
            def __init__(self):
                super().__init__(socket="unused")
                self.calls = []

            async def _read_once(self, tool, args):
                self.calls.append((tool, dict(args)))
                if len(self.calls) == 1:
                    raise OSError("private test transport dropped")
                return {"status": "completed"}

        reader = OneDropReader()
        args = {"run_id": "fixture-run", "compact": True}
        result = await reader.read("graph_run_get", args)

        self.assertEqual(result, {"status": "completed"})
        self.assertEqual(reader.calls, [("graph_run_get", args), ("graph_run_get", args)])
        self.assertEqual(reader.reconnections, 1)


if __name__ == "__main__":
    unittest.main()
