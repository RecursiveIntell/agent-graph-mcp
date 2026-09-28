"""Lane 12 regressions for the read-only run client."""
import asyncio
import importlib.util
import json
import pathlib
import subprocess
import sys
import unittest

from support import PROXY, GraphFixture


ROOT = pathlib.Path(__file__).resolve().parents[2]
CLIENT = ROOT / "scripts" / "graph_run_client.py"


def load_client():
    """Load the client module without invoking its command-line entrypoint."""
    spec = importlib.util.spec_from_file_location("lane12_graph_run_client", CLIENT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ReadOnlyClientTests(unittest.TestCase):
    def test_client_refuses_execution_tools_before_transport(self):
        client = load_client()
        reader = client.RunReader("unused-private-socket", proxy="unused-proxy")

        with self.assertRaisesRegex(client.ReadError, "READ_ONLY_METHOD_REQUIRED"):
            asyncio.run(reader.read("graph_run_start", {"graph_id": "must-not-run"}))

        self.assertEqual(reader.reconnections, 0)


class PrivateDaemonReadinessTests(unittest.TestCase):
    def test_cli_reads_existing_run_without_creating_an_execution(self):
        # The daemon and socket are confined to GraphFixture's disposable directory.
        value = {"lane": "lane12", "answer": 42}
        with GraphFixture() as fixture:
            run_id = fixture.run(value)

            def execution_count():
                conn = fixture.db()
                try:
                    return conn.execute("SELECT COUNT(*) FROM executions").fetchone()[0]
                finally:
                    conn.close()

            before = execution_count()
            self.assertGreaterEqual(before, 1)
            output_dir = fixture.root / "read-output"
            command = [
                sys.executable,
                str(CLIENT),
                "--socket",
                fixture.socket,
                "--run-id",
                run_id,
                "--out",
                str(output_dir),
                "--proxy",
                PROXY,
                "--wait-seconds",
                "10",
            ]
            result = subprocess.run(command, capture_output=True, text=True, timeout=30)

            self.assertEqual(result.returncode, 0, msg=f"stdout={result.stdout}\nstderr={result.stderr}")
            self.assertEqual(json.loads((output_dir / "output.json").read_text()), value)
            receipt = json.loads((output_dir / "READ_RECEIPT.json").read_text())
            self.assertTrue(receipt["complete"])
            self.assertEqual(receipt["run_id"], run_id)
            self.assertEqual(receipt["effects"], "read_only_no_dispatch")
            self.assertEqual(execution_count(), before)


if __name__ == "__main__":
    unittest.main()
