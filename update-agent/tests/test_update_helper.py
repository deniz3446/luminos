from __future__ import annotations

import hashlib
import io
import json
import os
import socket
import stat
import sys
import tarfile
import tempfile
import threading
import unittest
from pathlib import Path

AGENT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(AGENT_ROOT))

from update_helper import HelperConfig, ReleaseManager, _local_health_url, create_helper_server  # noqa: E402


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build_package(path: Path, release_id: str = "1.2.1-new-20260918") -> bytes:
    files = {
        "server": b"new-server-binary",
        "VERSION": b"1.2.1\n",
        "client/dist/index.html": b"<html>new</html>",
        "config/config.toml": b"[release]\nversion='1.2.1'\n",
        "migrations/0001.sql": b"select 1;\n",
    }
    manifest = {
        "product": "PhotoOS",
        "format_version": 1,
        "version": "1.2.1",
        "release_id": release_id,
        "channel": "stable",
        "release_notes": "helper test",
        "files": [
            {"path": rel, "size": len(data), "sha256": digest(data)}
            for rel, data in sorted(files.items())
        ],
    }
    with tarfile.open(path, "w:gz") as tf:
        raw = json.dumps(manifest).encode()
        info = tarfile.TarInfo("manifest.json")
        info.size = len(raw)
        tf.addfile(info, io.BytesIO(raw))
        for rel, data in files.items():
            info = tarfile.TarInfo(f"release/{rel}")
            info.size = len(data)
            tf.addfile(info, io.BytesIO(data))
    return path.read_bytes()


def create_release(root: Path, name: str, product_version: str = "1.2.0") -> Path:
    release = root / name
    (release / "client" / "dist").mkdir(parents=True)
    (release / "server").write_bytes(b"old-server")
    (release / "server").chmod(0o755)
    (release / "VERSION").write_text(product_version + "\n", encoding="utf-8")
    (release / "client" / "dist" / "index.html").write_text("old", encoding="utf-8")
    return release


class ReleaseManagerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.update_root = self.root / "update"
        self.packages = self.update_root / "packages"
        self.releases = self.root / "releases"
        self.current = self.root / "current"
        self.history = self.update_root / "history"
        self.socket_path = self.root / "run" / "helper.sock"
        self.packages.mkdir(parents=True)
        self.releases.mkdir()
        self.old = create_release(self.releases, "1.2.0-old")
        self.current.symlink_to(self.old)
        self.restart_calls: list[str] = []
        self.health_results: list[bool] = [True]

        def restart() -> None:
            self.restart_calls.append("restart")

        def health() -> bool:
            return self.health_results.pop(0) if self.health_results else True

        self.config = HelperConfig(
            update_root=self.update_root,
            releases_root=self.releases,
            current_link=self.current,
            socket_path=self.socket_path,
            config_file=self.root / "config.toml",
            health_url="http://127.0.0.1:9/api/v1/system/health",
        )
        self.manager = ReleaseManager(self.config, restart_service=restart, health_probe=health)

    def tearDown(self) -> None:
        self.temp.cleanup()


    def test_default_health_probe_uses_public_database_backed_setup_status(self) -> None:
        config_file = self.root / "photoos-config.toml"
        config_file.write_text("[server]\nport=8081\n", encoding="utf-8")
        config = HelperConfig(
            update_root=self.update_root,
            releases_root=self.releases,
            current_link=self.current,
            socket_path=self.socket_path,
            config_file=config_file,
        )

        self.assertEqual(_local_health_url(config), "http://127.0.0.1:8081/api/v1/setup/status")

    def test_request_schema_is_narrow_and_rejects_unknown_or_extra_fields(self) -> None:
        for payload in (
            {"operation": "shell", "command": "id"},
            {"operation": "install", "filename": "x.popkg", "command": "id"},
            {"operation": "rollback", "version": "1.2.0-old", "extra": True},
        ):
            with self.subTest(payload=payload):
                result = self.manager.handle(payload)
                self.assertFalse(result["ok"])
                self.assertEqual(result["error"], "invalid_request")

    def test_install_reverifies_package_creates_release_and_switches_current_atomically(self) -> None:
        package = self.packages / "new.popkg"
        build_package(package)

        result = self.manager.handle({"operation": "install", "filename": "new.popkg"})

        self.assertTrue(result["ok"])
        new_release = self.releases / "1.2.1-new-20260918"
        self.assertTrue(new_release.is_dir())
        self.assertEqual(self.current.resolve(), new_release.resolve())
        self.assertTrue(self.old.is_dir(), "previous release must be preserved")
        self.assertEqual((new_release / "server").read_bytes(), b"new-server-binary")
        self.assertTrue(os.access(new_release / "server", os.X_OK))
        self.assertEqual(stat.S_IMODE((new_release / "VERSION").stat().st_mode), 0o644)
        self.assertEqual(self.restart_calls, ["restart"])
        self.assertEqual(result["previous_release"], str(self.old))
        self.assertEqual(result["active_release"], str(new_release))

    def test_helper_reverification_blocks_corrupted_package_before_switch(self) -> None:
        package = self.packages / "bad.popkg"
        build_package(package)
        raw = bytearray(package.read_bytes())
        raw[-10:] = b"corruption"
        package.write_bytes(raw)

        result = self.manager.handle({"operation": "install", "filename": "bad.popkg"})

        self.assertFalse(result["ok"])
        self.assertIn(result["error"], {"package_invalid", "install_failed"})
        self.assertEqual(self.current.resolve(), self.old.resolve())
        self.assertEqual(self.restart_calls, [])

    def test_health_failure_restores_previous_release_and_restarts_again(self) -> None:
        package = self.packages / "new.popkg"
        build_package(package)
        self.health_results[:] = [False]

        result = self.manager.handle({"operation": "install", "filename": "new.popkg"})

        self.assertFalse(result["ok"])
        self.assertEqual(result["error"], "health_check_failed")
        self.assertEqual(self.current.resolve(), self.old.resolve())
        self.assertEqual(self.restart_calls, ["restart", "restart"])
        self.assertTrue((self.releases / "1.2.1-new-20260918").is_dir())

    def test_rollback_switches_to_exact_direct_child_and_rejects_traversal(self) -> None:
        newer = create_release(self.releases, "1.2.1-new", "1.2.1")
        os.unlink(self.current)
        self.current.symlink_to(newer)

        result = self.manager.handle({"operation": "rollback", "version": "1.2.0-old"})
        self.assertTrue(result["ok"])
        self.assertEqual(self.current.resolve(), self.old.resolve())

        bad = self.manager.handle({"operation": "rollback", "version": "../outside"})
        self.assertFalse(bad["ok"])
        self.assertEqual(bad["error"], "invalid_request")


class HelperSocketTests(unittest.TestCase):
    def test_unix_socket_round_trip_uses_json_only(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            config = HelperConfig(
                update_root=root / "update",
                releases_root=root / "releases",
                current_link=root / "current",
                socket_path=root / "run" / "helper.sock",
                config_file=root / "config.toml",
                health_url="http://127.0.0.1:9/health",
            )
            config.releases_root.mkdir()

            class FakeManager:
                def handle(self, payload: dict) -> dict:
                    return {"ok": True, "echo": payload}

            server = create_helper_server(config, manager=FakeManager(), manage_permissions=False)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
                    client.connect(str(config.socket_path))
                    client.sendall(b'{"operation":"rollback","version":"1.2.0"}\n')
                    client.shutdown(socket.SHUT_WR)
                    raw = b""
                    while chunk := client.recv(4096):
                        raw += chunk
                payload = json.loads(raw.decode())
                self.assertTrue(payload["ok"])
                self.assertEqual(payload["echo"], {"operation": "rollback", "version": "1.2.0"})
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=2)


if __name__ == "__main__":
    unittest.main()
