from __future__ import annotations

import hashlib
import http.client
import io
import json
import sys
import tarfile
import tempfile
import threading
import unittest
from pathlib import Path

AGENT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(AGENT_ROOT))

from update_agent import AgentConfig, _read_token, create_server  # noqa: E402

TOKEN = "b" * 64


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _build_package(path: Path) -> bytes:
    files = {
        "server": b"server-binary",
        "VERSION": b"1.2.1\n",
        "client/dist/index.html": b"<html>PhotoOS</html>",
        "config/config.toml": b"[release]\nversion='1.2.1'\n",
    }
    manifest = {
        "product": "PhotoOS",
        "format_version": 1,
        "version": "1.2.1",
        "release_id": "1.2.1-contract-test",
        "channel": "stable",
        "release_notes": "contract fixture",
        "files": [
            {"path": rel, "size": len(data), "sha256": _digest(data)}
            for rel, data in sorted(files.items())
        ],
    }
    with tarfile.open(path, "w:gz") as tf:
        encoded = json.dumps(manifest).encode("utf-8")
        info = tarfile.TarInfo("manifest.json")
        info.size = len(encoded)
        tf.addfile(info, io.BytesIO(encoded))
        for rel, data in files.items():
            info = tarfile.TarInfo(f"release/{rel}")
            info.size = len(data)
            tf.addfile(info, io.BytesIO(data))
    return path.read_bytes()


class UpdateCenterContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        update_root = root / "update"
        releases_root = root / "releases"
        current_link = root / "current"
        token_file = root / "update-agent.env"
        db_path = root / "runtime" / "luminos.db"
        storage_root = root / "storage"
        update_root.mkdir()
        releases_root.mkdir()
        db_path.parent.mkdir()
        db_path.write_bytes(b"sqlite-placeholder")
        storage_root.mkdir()
        token_file.write_text(f"PHOTOOS_UPDATE_TOKEN={TOKEN}\n", encoding="utf-8")

        release = releases_root / "1.2.1-contract-current"
        (release / "client" / "dist").mkdir(parents=True)
        (release / "config").mkdir()
        (release / "migrations").mkdir()
        (release / "server").write_bytes(b"server")
        (release / "VERSION").write_text("1.2.1\n", encoding="utf-8")
        (release / "client" / "dist" / "index.html").write_text("PhotoOS", encoding="utf-8")
        (release / "config" / "config.toml").write_text("[release]\nversion='1.2.1'\n", encoding="utf-8")
        current_link.symlink_to(release)

        self.config = AgentConfig(
            update_root=update_root,
            releases_root=releases_root,
            current_link=current_link,
            token_env_file=token_file,
            helper_socket=root / "helper.sock",
            config_file=root / "config.toml",
            database_path=db_path,
            storage_root=storage_root,
            server_health_url="http://127.0.0.1:9/api/v1/system/health",
            web_health_url="http://127.0.0.1:9/",
        )
        self.dispatched: list[dict] = []

        def helper(payload: dict) -> dict:
            self.dispatched.append(payload)
            return {
                "ok": True,
                "operation": payload["operation"],
                "message": "accepted",
                "timestamp": "2026-09-18T00:00:00Z",
            }

        self.server = create_server(self.config, port=0, helper_dispatcher=helper)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.host, self.port = self.server.server_address

    def tearDown(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)
        self.temp.cleanup()

    def request(self, method: str, path: str, *, body: bytes | None = None, headers: dict[str, str] | None = None):
        connection = http.client.HTTPConnection(self.host, self.port, timeout=3)
        connection.request(method, path, body=body, headers=headers or {})
        response = connection.getresponse()
        raw = response.read()
        connection.close()
        return response.status, json.loads(raw.decode("utf-8"))

    def auth_headers(self, **extra: str) -> dict[str, str]:
        return {"X-PhotoOS-Update-Token": TOKEN, **extra}

    def _upload_and_verify(self) -> tuple[dict, dict]:
        fixture = Path(self.temp.name) / "fixture.popkg"
        data = _build_package(fixture)
        status, upload = self.request(
            "POST",
            "/upload-package",
            body=data,
            headers=self.auth_headers(
                **{
                    "X-PhotoOS-Filename": "contract.popkg",
                    "Content-Type": "application/octet-stream",
                    "Content-Length": str(len(data)),
                }
            ),
        )
        self.assertEqual(status, 201)
        status, verification = self.request(
            "POST",
            "/verify-package",
            body=json.dumps({"filename": "contract.popkg"}).encode("utf-8"),
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 200)
        return upload, verification

    def test_status_shape_matches_system_updates_page(self) -> None:
        status_code, payload = self.request("GET", "/status")
        self.assertEqual(status_code, 200)
        self.assertTrue({"product", "agent_version", "mode", "timestamp", "healthy", "health"} <= payload.keys())
        self.assertTrue(
            {"healthy", "server_http", "web_http", "database_exists", "storage_exists", "server_service", "web_service", "active_release"}
            <= payload["health"].keys()
        )
        self.assertTrue({"version", "path", "valid"} <= payload["health"]["active_release"].keys())

    def test_releases_shape_matches_system_updates_page(self) -> None:
        status_code, payload = self.request("GET", "/releases")
        self.assertEqual(status_code, 200)
        self.assertTrue({"active_release", "releases", "timestamp"} <= payload.keys())
        self.assertTrue({"version", "path", "valid"} <= payload["active_release"].keys())
        self.assertGreaterEqual(len(payload["releases"]), 1)
        self.assertTrue(
            {"version", "path", "active", "server_exists", "config_exists", "migrations_exist", "web_exists"}
            <= payload["releases"][0].keys()
        )

    def test_packages_and_verification_shapes_match_system_updates_page(self) -> None:
        _, verification = self._upload_and_verify()
        self.assertTrue(
            {"valid", "version", "verified_at", "verified_files", "package_sha256", "manifest"}
            <= verification.keys()
        )
        self.assertTrue({"version", "release_notes", "channel"} <= verification["manifest"].keys())

        status_code, payload = self.request("GET", "/packages")
        self.assertEqual(status_code, 200)
        self.assertTrue({"packages", "timestamp"} <= payload.keys())
        item = payload["packages"][0]
        self.assertTrue({"filename", "size_bytes", "sha256", "verified", "verification"} <= item.keys())
        self.assertTrue(item["verification"]["valid"])

    def test_history_shape_matches_system_updates_page(self) -> None:
        self._upload_and_verify()
        status_code, payload = self.request("GET", "/history")
        self.assertEqual(status_code, 200)
        self.assertTrue({"history", "timestamp"} <= payload.keys())
        self.assertGreaterEqual(len(payload["history"]), 2)
        self.assertTrue({"filename", "payload"} <= payload["history"][0].keys())
        self.assertIsInstance(payload["history"][0]["payload"], dict)

    def test_mutating_success_bodies_are_json_objects_with_stable_fields(self) -> None:
        upload, verification = self._upload_and_verify()
        self.assertTrue({"uploaded", "filename", "size_bytes", "sha256", "timestamp"} <= upload.keys())
        self.assertTrue({"valid", "version", "package_sha256"} <= verification.keys())

        status, install = self.request(
            "POST",
            "/install-package",
            body=json.dumps({"filename": "contract.popkg"}).encode("utf-8"),
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 200)
        self.assertTrue({"ok", "operation", "message", "timestamp"} <= install.keys())

        status, rollback = self.request(
            "POST",
            "/rollback",
            body=json.dumps({"version": "1.2.1-contract-current"}).encode("utf-8"),
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 200)
        self.assertTrue({"ok", "operation", "message", "timestamp"} <= rollback.keys())

    def test_token_file_must_contain_exactly_64_lowercase_hex_characters(self) -> None:
        self.config.token_env_file.write_text("PHOTOOS_UPDATE_TOKEN=short-token\n", encoding="utf-8")
        with self.assertRaises(RuntimeError):
            _read_token(self.config)


if __name__ == "__main__":
    unittest.main()
