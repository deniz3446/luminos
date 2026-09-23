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

from update_agent import AgentConfig, _runtime_paths_and_urls, create_server  # noqa: E402


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build_package(path: Path, release_id: str = "1.2.1-agent-test") -> bytes:
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
        "release_id": release_id,
        "channel": "stable",
        "release_notes": "test",
        "files": [
            {"path": rel, "size": len(data), "sha256": digest(data)}
            for rel, data in sorted(files.items())
        ],
    }
    with tarfile.open(path, "w:gz") as tf:
        manifest_data = json.dumps(manifest).encode()
        info = tarfile.TarInfo("manifest.json")
        info.size = len(manifest_data)
        tf.addfile(info, io.BytesIO(manifest_data))
        for rel, data in files.items():
            info = tarfile.TarInfo(f"release/{rel}")
            info.size = len(data)
            tf.addfile(info, io.BytesIO(data))
    return path.read_bytes()


class AgentHttpTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        self.update_root = root / "update"
        self.releases_root = root / "releases"
        self.current_link = root / "current"
        self.token_file = root / "update-agent.env"
        self.db_path = root / "runtime" / "luminos.db"
        self.storage_root = root / "storage"
        self.helper_socket = root / "helper.sock"
        self.update_root.mkdir()
        self.releases_root.mkdir()
        self.db_path.parent.mkdir()
        self.db_path.write_bytes(b"sqlite-placeholder")
        self.storage_root.mkdir()
        self.token_file.write_text("PHOTOOS_UPDATE_TOKEN=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n", encoding="utf-8")

        release = self.releases_root / "1.2.1-current"
        (release / "client" / "dist").mkdir(parents=True)
        (release / "config").mkdir()
        (release / "migrations").mkdir()
        (release / "server").write_bytes(b"server")
        (release / "VERSION").write_text("1.2.1\n", encoding="utf-8")
        (release / "client" / "dist" / "index.html").write_text("PhotoOS", encoding="utf-8")
        (release / "config" / "config.toml").write_text("[release]\nversion='1.2.1'\n", encoding="utf-8")
        self.current_link.symlink_to(release)

        self.dispatched: list[dict] = []

        def helper_dispatch(payload: dict) -> dict:
            self.dispatched.append(payload)
            return {"ok": True, "operation": payload["operation"], "message": "accepted"}

        self.config = AgentConfig(
            update_root=self.update_root,
            releases_root=self.releases_root,
            current_link=self.current_link,
            token_env_file=self.token_file,
            helper_socket=self.helper_socket,
            config_file=root / "config.toml",
            database_path=self.db_path,
            storage_root=self.storage_root,
            server_health_url="http://127.0.0.1:9/api/v1/system/health",
            web_health_url="http://127.0.0.1:9/",
        )
        self.server = create_server(self.config, port=0, helper_dispatcher=helper_dispatch)
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
        payload = json.loads(raw.decode("utf-8")) if raw else None
        return response.status, payload

    def auth_headers(self, **extra: str) -> dict[str, str]:
        return {"X-PhotoOS-Update-Token": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", **extra}


    def test_default_server_probe_uses_public_database_backed_setup_status(self) -> None:
        config_file = Path(self.temp.name) / "server-config.toml"
        config_file.write_text("[server]\nport=8081\n", encoding="utf-8")
        config = AgentConfig(
            config_file=config_file,
            database_path=self.db_path,
            storage_root=self.storage_root,
        )

        _, _, server_health_url, _ = _runtime_paths_and_urls(config)

        self.assertEqual(server_health_url, "http://127.0.0.1:8081/api/v1/setup/status")

    def test_default_server_is_loopback_and_read_contracts_have_expected_shapes(self) -> None:
        self.assertEqual(self.host, "127.0.0.1")

        status_code, status = self.request("GET", "/status")
        self.assertEqual(status_code, 200)
        self.assertEqual(status["product"], "PhotoOS Update Agent")
        self.assertEqual(status["mode"], "canonical-v4")
        self.assertIn("healthy", status)
        self.assertIn("active_release", status["health"])

        status_code, releases = self.request("GET", "/releases")
        self.assertEqual(status_code, 200)
        self.assertEqual(releases["active_release"]["path"], str(self.current_link.resolve()))
        self.assertEqual(len(releases["releases"]), 1)
        self.assertTrue(releases["releases"][0]["active"])

        status_code, packages = self.request("GET", "/packages")
        self.assertEqual(status_code, 200)
        self.assertEqual(packages["packages"], [])

        status_code, history = self.request("GET", "/history")
        self.assertEqual(status_code, 200)
        self.assertEqual(history["history"], [])

    def test_mutating_endpoint_rejects_missing_or_wrong_token(self) -> None:
        body = json.dumps({"filename": "x.popkg"}).encode()
        for token in (None, "wrong"):
            headers = {"Content-Type": "application/json"}
            if token is not None:
                headers["X-PhotoOS-Update-Token"] = token
            with self.subTest(token=token):
                status, payload = self.request("POST", "/verify-package", body=body, headers=headers)
                self.assertEqual(status, 401)
                self.assertEqual(payload["error"], "unauthorized")

    def test_upload_verify_and_package_listing_round_trip(self) -> None:
        local = Path(self.temp.name) / "fixture.popkg"
        package_bytes = build_package(local)
        headers = self.auth_headers(
            **{
                "X-PhotoOS-Filename": "photoos-test.popkg",
                "Content-Type": "application/octet-stream",
                "Content-Length": str(len(package_bytes)),
            }
        )
        status, uploaded = self.request("POST", "/upload-package", body=package_bytes, headers=headers)
        self.assertEqual(status, 201)
        self.assertEqual(uploaded["filename"], "photoos-test.popkg")

        verify_body = json.dumps({"filename": "photoos-test.popkg"}).encode()
        status, verified = self.request(
            "POST",
            "/verify-package",
            body=verify_body,
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 200)
        self.assertTrue(verified["valid"])
        self.assertEqual(verified["version"], "1.2.1")

        status, packages = self.request("GET", "/packages")
        self.assertEqual(status, 200)
        self.assertEqual(len(packages["packages"]), 1)
        item = packages["packages"][0]
        self.assertEqual(item["filename"], "photoos-test.popkg")
        self.assertTrue(item["verified"])
        self.assertTrue(item["verification"]["valid"])

        status, history = self.request("GET", "/history")
        self.assertEqual(status, 200)
        self.assertGreaterEqual(len(history["history"]), 2)

    def test_upload_rejects_declared_body_over_limit_before_reading(self) -> None:
        status, payload = self.request(
            "POST",
            "/upload-package",
            body=b"",
            headers=self.auth_headers(
                **{
                    "X-PhotoOS-Filename": "huge.popkg",
                    "Content-Type": "application/octet-stream",
                    "Content-Length": str(512 * 1024 * 1024 + 1),
                }
            ),
        )
        self.assertEqual(status, 413)
        self.assertEqual(payload["error"], "package_too_large")

    def test_install_requires_current_verification_and_dispatches_exact_leaf_name(self) -> None:
        body = json.dumps({"filename": "photoos-test.popkg"}).encode()
        status, payload = self.request(
            "POST",
            "/install-package",
            body=body,
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 409)
        self.assertEqual(payload["error"], "package_not_verified")
        self.assertEqual(self.dispatched, [])

        local = Path(self.temp.name) / "fixture.popkg"
        package_bytes = build_package(local)
        upload_headers = self.auth_headers(
            **{
                "X-PhotoOS-Filename": "photoos-test.popkg",
                "Content-Type": "application/octet-stream",
                "Content-Length": str(len(package_bytes)),
            }
        )
        self.assertEqual(self.request("POST", "/upload-package", body=package_bytes, headers=upload_headers)[0], 201)
        verify = json.dumps({"filename": "photoos-test.popkg"}).encode()
        self.assertEqual(self.request("POST", "/verify-package", body=verify, headers=self.auth_headers(**{"Content-Type": "application/json"}))[0], 200)

        status, payload = self.request(
            "POST",
            "/install-package",
            body=body,
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 200)
        self.assertTrue(payload["ok"])
        self.assertEqual(self.dispatched[-1], {"operation": "install", "filename": "photoos-test.popkg"})

    def test_rollback_dispatches_only_safe_release_identifier(self) -> None:
        body = json.dumps({"version": "1.2.1-current"}).encode()
        status, payload = self.request(
            "POST",
            "/rollback",
            body=body,
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 200)
        self.assertTrue(payload["ok"])
        self.assertEqual(self.dispatched[-1], {"operation": "rollback", "version": "1.2.1-current"})

        bad = json.dumps({"version": "../etc"}).encode()
        status, payload = self.request(
            "POST",
            "/rollback",
            body=bad,
            headers=self.auth_headers(**{"Content-Type": "application/json"}),
        )
        self.assertEqual(status, 400)
        self.assertEqual(payload["error"], "invalid_version")


if __name__ == "__main__":
    unittest.main()
