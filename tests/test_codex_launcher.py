"""Launcher isolation contracts without credentials or real provider calls."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class LauncherTests(unittest.TestCase):
    def run_launcher(self, discovery, *, preselected=None):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            codex = root / 'codex'
            codex.write_text('#!/usr/bin/env python3\n' + discovery)
            codex.chmod(0o700)
            daemon = root / 'daemon'
            daemon.write_text(
                '#!/usr/bin/env python3\n'
                'import json, os, sys\n'
                'print(json.dumps({"args":sys.argv[1:], "disabled":'
                'os.environ.get("AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON")}))\n'
            )
            daemon.chmod(0o700)
            env = {
                'HOME': str(root), 'PATH': os.defpath,
                'AGENT_GRAPH_CODEX_COMMAND': str(codex),
                'AGENT_GRAPH_DAEMON_COMMAND': str(daemon),
                'AGENT_GRAPH_MODEL': 'fixture-model',
            }
            if preselected is not None:
                env['AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON'] = preselected
            return subprocess.run(
                ['bash', str(ROOT / 'scripts/launch-codex-daemon.sh'), '--socket', 'fixture.sock'],
                env=env, capture_output=True, text=True, timeout=5,
            )

    def test_discovery_and_worker_suppression_agree(self):
        result = self.run_launcher(
            'import json, sys\n'
            'assert sys.argv[1:] == ["mcp", "list", "--json", "-c", '
            '"features.plugins=false", "-c", "features.apps=false"]\n'
            'print(json.dumps([{"name":"direct"}, {"name":"off","enabled":False}]))\n'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(json.loads(payload['disabled']), ['direct'])
        self.assertEqual(payload['args'], ['--base-url', 'codex-app-server://',
                                          '--model', 'fixture-model', '--socket', 'fixture.sock'])

    def test_failed_discovery_never_starts_daemon(self):
        result = self.run_launcher('import sys\nprint("[]")\nsys.exit(7)\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, '')

    def test_unsafe_server_name_never_starts_daemon(self):
        result = self.run_launcher('print(\'[ {"name":"bad.name"} ]\')\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, '')

    def test_explicit_selection_skips_discovery(self):
        result = self.run_launcher('raise AssertionError("discovery must not run")\n', preselected='[]')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)['disabled'], '[]')


if __name__ == '__main__':
    unittest.main()
