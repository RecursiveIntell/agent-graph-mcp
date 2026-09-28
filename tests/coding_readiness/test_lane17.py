"""Shell-fixture regressions for the canonical Codex phase runner."""
import json
import pathlib
import shutil
import subprocess
import tempfile
import unittest


RUNNER = pathlib.Path(__file__).resolve().parents[2] / "scripts" / "run_codex_phase.sh"


class CodexPhaseRunnerTests(unittest.TestCase):
    def run_fixture(self, agent_text, exit_code):
        temp = tempfile.TemporaryDirectory(prefix="lane17-phase-")
        self.addCleanup(temp.cleanup)
        root = pathlib.Path(temp.name)
        fixture_bin = root / "bin"
        fixture_bin.mkdir()
        runner = root / "run_codex_phase.sh"
        shutil.copyfile(RUNNER, runner)
        phase_spec = root / "phase.md"
        phase_spec.write_text("Fixture-only phase text: FIXTURE_SPEC_SENTINEL\n")
        codex = fixture_bin / "codex"
        codex.write_text(
            "#!/bin/sh\n"
            "cat <<'JSONL'\n"
            '{"type":"item.completed","item":{"type":"agent_message","text":'
            + json.dumps(agent_text)
            + "}}\n"
            "JSONL\n"
            f"exit {exit_code}\n"
        )
        codex.chmod(0o755)
        receipts = root / "receipts"
        env = {
            "PATH": f"{fixture_bin}:/usr/bin:/bin",
            "HOME": str(root),
            "RECEIPT_DIR": str(receipts),
            # The runner polls every five seconds and checks the cap after
            # sleeping, so allow a second poll to observe the quick fake exit.
            "MAX_WAIT": "10",
        }
        result = subprocess.run(
            ["bash", str(runner), "fixture", str(phase_spec), str(root)],
            cwd=root,
            env=env,
            text=True,
            capture_output=True,
            timeout=20,
            check=False,
        )
        return result, receipts

    def test_agent_message_does_not_mask_codex_exit_7(self):
        result, receipts = self.run_fixture("MODEL_OUTPUT_SENTINEL", 7)

        self.assertEqual(result.returncode, 7, result.stdout + result.stderr)
        self.assertEqual(
            (receipts / "PHASE_fixture.codex.exit").read_text().strip(), "7"
        )

    def test_recorded_message_is_from_jsonl_not_phase_spec(self):
        result, receipts = self.run_fixture("MODEL_OUTPUT_SENTINEL", 7)

        self.assertEqual(result.returncode, 7, result.stdout + result.stderr)
        message = (receipts / "PHASE_fixture.codex.txt").read_text()
        self.assertEqual(message, "MODEL_OUTPUT_SENTINEL\n")
        self.assertNotIn("FIXTURE_SPEC_SENTINEL", message)


if __name__ == "__main__":
    unittest.main()
