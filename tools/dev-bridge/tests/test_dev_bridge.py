import unittest
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from photoos_dev_bridge import (
    backend_gate_command,
    frontend_gate_commands,
    remote_commander_command,
    sanitize_branch_name,
)


class DevBridgeContractTests(unittest.TestCase):
    def test_branch_name_is_ascii_safe_and_prefixed(self):
        self.assertEqual(
            sanitize_branch_name("Fotoğraflar UI Düzelt"),
            "ai/fotograflar-ui-duzelt",
        )

    def test_windows_frontend_uses_npm_cmd(self):
        commands = frontend_gate_commands(windows=True)
        self.assertEqual(commands[0][0], "npm.cmd")
        self.assertIn(["npm.cmd", "run", "contract-check"], commands)
        self.assertIn(["npm.cmd", "run", "build"], commands)

    def test_backend_gate_is_isolated_and_has_no_live_actions(self):
        command = backend_gate_command(
            "/home/photoos/PhotoOS-worktrees/ai-task-example"
        )
        self.assertIn("cargo test --workspace", command)
        self.assertIn("cargo check --workspace", command)
        self.assertIn("/home/photoos/PhotoOS-worktrees/ai-task-example", command)
        lowered = command.lower()
        self.assertNotIn("systemctl", lowered)
        self.assertNotIn("/opt/photoos/current", lowered)
        self.assertNotIn("update/install", lowered)

    def test_remote_commander_uses_npx_cmd(self):
        self.assertEqual(
            remote_commander_command(windows=True),
            [
                "npx.cmd",
                "@wonderwhy-er/desktop-commander@latest",
                "remote",
            ],
        )

    def test_frontend_default_gates_do_not_deploy(self):
        text = " ".join(" ".join(cmd) for cmd in frontend_gate_commands(True)).lower()
        for forbidden in ("deploy", "systemctl", "restart", "update/install"):
            self.assertNotIn(forbidden, text)


if __name__ == "__main__":
    unittest.main()


class DevBridgeStatusTests(unittest.TestCase):
    def test_repo_snapshot_reports_only_public_git_state(self):
        from photoos_dev_bridge import repo_snapshot

        values = {
            ("branch",): "ai/example\n",
            ("status",): "",
            ("head",): "abc123\n",
            ("remote",): "https://github.com/deniz3446/luminos.git\n",
        }

        def reader(kind):
            return values[kind]

        result = repo_snapshot(reader=reader)
        self.assertEqual(result["branch"], "ai/example")
        self.assertTrue(result["clean"])
        self.assertEqual(result["head"], "abc123")
        self.assertEqual(result["remote"], "https://github.com/deniz3446/luminos.git")
        self.assertNotIn("token", result)
        self.assertNotIn("secret", result)


class DevBridgePresentationTests(unittest.TestCase):
    def test_status_lines_present_repo_and_verification(self):
        from photoos_dev_bridge import status_lines

        lines = status_lines(
            {
                "branch": "main",
                "clean": True,
                "head": "abcdef1234567890",
                "remote": "https://github.com/deniz3446/luminos.git",
            },
            {"verification": "PASS", "frontend": "PASS", "backend": "PASS"},
        )
        text = "\n".join(lines)
        self.assertIn("Branch: main", text)
        self.assertIn("Çalışma ağacı: TEMİZ", text)
        self.assertIn("abcdef123456", text)
        self.assertIn("Doğrulama: PASS", text)
        self.assertNotIn("token", text.lower())


class DevBridgeStatusFileTests(unittest.TestCase):
    def test_status_file_round_trip(self):
        import tempfile
        from photoos_dev_bridge import load_status_file, save_status_file

        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "status.json"
            self.assertEqual(load_status_file(path), {})
            payload = {"verification": "PASS", "frontend": "PASS"}
            save_status_file(payload, path)
            self.assertEqual(load_status_file(path), payload)
