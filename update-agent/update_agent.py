#!/usr/bin/env python3
from __future__ import annotations

import hmac
import json
import os
import secrets
import socket
import subprocess
import tempfile
import threading
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Callable
from urllib.parse import urlsplit

from agent_core import AGENT_VERSION, PackageError, sha256_file, utc_now, validate_leaf_name, validate_release_id, verify_package

MAX_PACKAGE_BYTES = 512 * 1024 * 1024
MAX_JSON_BYTES = 64 * 1024
MAX_HELPER_RESPONSE = 128 * 1024
TOKEN_HEADER = "X-PhotoOS-Update-Token"
FILENAME_HEADER = "X-PhotoOS-Filename"


@dataclass(frozen=True)
class AgentConfig:
    update_root: Path = Path("/var/lib/photoos/update")
    releases_root: Path = Path("/opt/photoos/releases")
    current_link: Path = Path("/opt/photoos/current")
    token_env_file: Path = Path("/etc/photoos/update-agent.env")
    helper_socket: Path = Path("/run/photoos/update-helper.sock")
    config_file: Path = Path("/etc/photoos/config.toml")
    database_path: Path | None = None
    storage_root: Path | None = None
    server_health_url: str | None = None
    web_health_url: str | None = None
    bind_host: str = "127.0.0.1"
    port: int = 8091

    @property
    def packages_dir(self) -> Path:
        return self.update_root / "packages"

    @property
    def history_dir(self) -> Path:
        return self.update_root / "history"

    @property
    def verification_dir(self) -> Path:
        return self.update_root / "verification"

    @classmethod
    def from_env(cls) -> "AgentConfig":
        def path_env(name: str, default: str) -> Path:
            return Path(os.environ.get(name, default))

        port_text = os.environ.get("PHOTOOS_UPDATE_PORT", "8091")
        try:
            port = int(port_text)
        except ValueError as exc:
            raise SystemExit("PHOTOOS_UPDATE_PORT must be an integer") from exc
        if not 1 <= port <= 65535:
            raise SystemExit("PHOTOOS_UPDATE_PORT is outside 1..65535")

        database = os.environ.get("PHOTOOS_DATABASE_PATH")
        storage = os.environ.get("PHOTOOS_STORAGE_ROOT")
        return cls(
            update_root=path_env("PHOTOOS_UPDATE_ROOT", "/var/lib/photoos/update"),
            releases_root=path_env("PHOTOOS_RELEASES_ROOT", "/opt/photoos/releases"),
            current_link=path_env("PHOTOOS_CURRENT_LINK", "/opt/photoos/current"),
            token_env_file=path_env("PHOTOOS_UPDATE_TOKEN_FILE", "/etc/photoos/update-agent.env"),
            helper_socket=path_env("PHOTOOS_HELPER_SOCKET", "/run/photoos/update-helper.sock"),
            config_file=path_env("PHOTOOS_CONFIG", "/etc/photoos/config.toml"),
            database_path=Path(database) if database else None,
            storage_root=Path(storage) if storage else None,
            server_health_url=os.environ.get("PHOTOOS_SERVER_HEALTH_URL"),
            web_health_url=os.environ.get("PHOTOOS_WEB_HEALTH_URL"),
            bind_host="127.0.0.1",
            port=port,
        )


HelperDispatcher = Callable[[dict], dict]


def _ensure_layout(config: AgentConfig) -> None:
    for path in (config.update_root, config.packages_dir, config.history_dir, config.verification_dir):
        path.mkdir(parents=True, exist_ok=True)


def _read_token(config: AgentConfig) -> str:
    text = config.token_env_file.read_text(encoding="utf-8")
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key.strip() == "PHOTOOS_UPDATE_TOKEN":
            token = value.strip()
            if len(token) != 64 or any(char not in "0123456789abcdef" for char in token):
                raise RuntimeError("PHOTOOS_UPDATE_TOKEN must be 64 lowercase hexadecimal characters")
            return token
    raise RuntimeError("PHOTOOS_UPDATE_TOKEN is missing")


def _token_ok(config: AgentConfig, candidate: str | None) -> bool:
    if not candidate:
        return False
    try:
        expected = _read_token(config)
    except (OSError, RuntimeError):
        return False
    return hmac.compare_digest(candidate, expected)


def _load_runtime_config(config: AgentConfig) -> dict:
    try:
        with config.config_file.open("rb") as handle:
            parsed = tomllib.load(handle)
            return parsed if isinstance(parsed, dict) else {}
    except (OSError, tomllib.TOMLDecodeError):
        return {}


def _runtime_paths_and_urls(config: AgentConfig) -> tuple[Path, Path, str, str]:
    parsed = _load_runtime_config(config)
    database_cfg = parsed.get("database", {}) if isinstance(parsed.get("database"), dict) else {}
    storage_cfg = parsed.get("storage", {}) if isinstance(parsed.get("storage"), dict) else {}
    server_cfg = parsed.get("server", {}) if isinstance(parsed.get("server"), dict) else {}

    database = config.database_path or Path(str(database_cfg.get("path") or "/var/lib/photoos/runtime/luminos.db"))
    storage = config.storage_root or Path(str(storage_cfg.get("root") or "/srv/photoos"))
    try:
        port = int(server_cfg.get("port", 8080))
    except (TypeError, ValueError):
        port = 8080
    server_health = config.server_health_url or f"http://127.0.0.1:{port}/api/v1/setup/status"
    web_health = config.web_health_url or f"http://127.0.0.1:{port}/"
    return database, storage, server_health, web_health


def _http_ok(url: str) -> bool:
    try:
        with urllib.request.urlopen(url, timeout=0.8) as response:
            return 200 <= response.status < 400
    except (OSError, urllib.error.URLError, ValueError):
        return False


def _service_state(name: str) -> dict:
    try:
        result = subprocess.run(
            ["systemctl", "is-active", name],
            check=False,
            capture_output=True,
            text=True,
            timeout=2,
        )
        state = (result.stdout or result.stderr).strip() or "unknown"
        return {"state": state, "active": result.returncode == 0 and state == "active"}
    except (OSError, subprocess.SubprocessError):
        return {"state": "unknown", "active": False}


def _release_version(path: Path) -> str:
    try:
        value = (path / "VERSION").read_text(encoding="utf-8").strip()
        return value or path.name
    except OSError:
        return path.name


def _release_row(path: Path, active_target: Path | None) -> dict:
    try:
        resolved = path.resolve(strict=True)
    except OSError:
        resolved = path
    active = active_target is not None and resolved == active_target
    server_exists = (path / "server").is_file()
    config_exists = (path / "config" / "config.toml").is_file()
    migrations_exist = (path / "migrations").is_dir()
    web_exists = (path / "client" / "dist" / "index.html").is_file()
    return {
        "version": path.name,
        "product_version": _release_version(path),
        "path": str(path),
        "active": active,
        "valid": server_exists and web_exists and (path / "VERSION").is_file(),
        "server_exists": server_exists,
        "config_exists": config_exists,
        "migrations_exist": migrations_exist,
        "web_exists": web_exists,
    }


def active_release(config: AgentConfig) -> dict:
    try:
        target = config.current_link.resolve(strict=True)
    except OSError:
        return {"version": None, "release_id": None, "path": None, "valid": False}
    if target.parent != config.releases_root.resolve(strict=False):
        return {"version": None, "release_id": None, "path": str(target), "valid": False}
    row = _release_row(target, target)
    return {
        "version": row["product_version"],
        "release_id": target.name,
        "path": str(target),
        "valid": row["valid"],
    }


def build_releases(config: AgentConfig) -> dict:
    try:
        current_target = config.current_link.resolve(strict=True)
    except OSError:
        current_target = None
    rows = []
    if config.releases_root.is_dir():
        for path in sorted(config.releases_root.iterdir(), key=lambda item: item.name):
            if path.is_dir() and not path.is_symlink():
                rows.append(_release_row(path, current_target))
    return {"active_release": active_release(config), "releases": rows, "timestamp": utc_now()}


def build_status(config: AgentConfig) -> dict:
    database, storage, server_health_url, web_health_url = _runtime_paths_and_urls(config)
    current = active_release(config)
    server_service = _service_state("photoos.service")
    server_http = _http_ok(server_health_url)
    web_http = _http_ok(web_health_url)
    database_exists = database.is_file()
    storage_exists = storage.is_dir()
    healthy = bool(server_http and web_http and database_exists and storage_exists and current["valid"])
    return {
        "product": "PhotoOS Update Agent",
        "agent_version": AGENT_VERSION,
        "mode": "canonical-v4",
        "timestamp": utc_now(),
        "healthy": healthy,
        "health": {
            "healthy": healthy,
            "server_http": server_http,
            "web_http": web_http,
            "database_exists": database_exists,
            "storage_exists": storage_exists,
            "server_service": server_service,
            "web_service": server_service,
            "active_release": current,
        },
    }


def _verification_path(config: AgentConfig, filename: str) -> Path:
    safe = validate_leaf_name(filename, suffix=".popkg")
    return config.verification_dir / f"{safe}.json"


def _load_verification(config: AgentConfig, package: Path) -> dict | None:
    try:
        sidecar = _verification_path(config, package.name)
        payload = json.loads(sidecar.read_text(encoding="utf-8"))
        if not isinstance(payload, dict) or payload.get("valid") is not True:
            return None
        if payload.get("package_sha256") != sha256_file(package):
            return None
        return payload
    except (OSError, ValueError, json.JSONDecodeError):
        return None


def build_packages(config: AgentConfig) -> dict:
    packages = []
    if config.packages_dir.is_dir():
        for path in sorted(config.packages_dir.iterdir(), key=lambda item: item.name):
            if not path.is_file() or path.is_symlink() or not path.name.endswith(".popkg"):
                continue
            verification = _load_verification(config, path)
            packages.append(
                {
                    "filename": path.name,
                    "size_bytes": path.stat().st_size,
                    "sha256": sha256_file(path),
                    "verified": verification is not None,
                    "verification": verification,
                }
            )
    return {"packages": packages, "timestamp": utc_now()}


def _record_history(config: AgentConfig, action: str, filename: str, payload: dict) -> None:
    config.history_dir.mkdir(parents=True, exist_ok=True)
    record = {
        "action": action,
        "timestamp": utc_now(),
        **payload,
    }
    stamp = utc_now().replace(":", "").replace("-", "").replace(".", "")
    leaf = validate_leaf_name(filename) if filename else "event"
    target = config.history_dir / f"{stamp}-{secrets.token_hex(4)}-{leaf}.json"
    temp = target.with_suffix(target.suffix + ".tmp")
    temp.write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(temp, target)


def build_history(config: AgentConfig) -> dict:
    rows = []
    if config.history_dir.is_dir():
        for path in sorted(config.history_dir.glob("*.json"), reverse=True)[:100]:
            if path.is_symlink() or not path.is_file():
                continue
            try:
                payload = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                continue
            rows.append({"filename": path.name, "payload": payload if isinstance(payload, dict) else {"value": payload}})
    return {"history": rows, "timestamp": utc_now()}


def dispatch_to_helper(config: AgentConfig, payload: dict) -> dict:
    encoded = json.dumps(payload, separators=(",", ":")).encode("utf-8") + b"\n"
    if len(encoded) > MAX_JSON_BYTES:
        raise RuntimeError("helper request is too large")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(605)
        client.connect(str(config.helper_socket))
        client.sendall(encoded)
        client.shutdown(socket.SHUT_WR)
        chunks = []
        size = 0
        while True:
            chunk = client.recv(8192)
            if not chunk:
                break
            size += len(chunk)
            if size > MAX_HELPER_RESPONSE:
                raise RuntimeError("helper response is too large")
            chunks.append(chunk)
    try:
        result = json.loads(b"".join(chunks).decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RuntimeError("helper response is invalid") from exc
    if not isinstance(result, dict):
        raise RuntimeError("helper response must be an object")
    return result


class UpdateAgentHTTPServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, address, handler, config: AgentConfig, helper_dispatcher: HelperDispatcher):
        super().__init__(address, handler)
        self.config = config
        self.helper_dispatcher = helper_dispatcher


class UpdateAgentHandler(BaseHTTPRequestHandler):
    server_version = "PhotoOSUpdateAgent/1.0"
    protocol_version = "HTTP/1.1"

    @property
    def config(self) -> AgentConfig:
        return self.server.config  # type: ignore[attr-defined]

    def log_message(self, fmt: str, *args) -> None:
        print(f"update-agent {self.address_string()} {fmt % args}")

    def _json(self, status: int, payload: dict) -> None:
        data = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(data)

    def _error(self, status: int, code: str, message: str) -> None:
        self._json(status, {"error": code, "message": message, "timestamp": utc_now()})

    def _authorized(self) -> bool:
        return _token_ok(self.config, self.headers.get(TOKEN_HEADER))

    def _json_body(self) -> dict | None:
        raw_length = self.headers.get("Content-Length")
        try:
            length = int(raw_length or "0")
        except ValueError:
            self._error(400, "invalid_content_length", "Content-Length geçersiz.")
            return None
        if length <= 0 or length > MAX_JSON_BYTES:
            self._error(400, "invalid_json_size", "JSON gövdesi boş veya çok büyük.")
            return None
        raw = self.rfile.read(length)
        if len(raw) != length:
            self._error(400, "incomplete_body", "İstek gövdesi tamamlanamadı.")
            return None
        try:
            payload = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            self._error(400, "invalid_json", "Geçerli JSON gerekli.")
            return None
        if not isinstance(payload, dict):
            self._error(400, "invalid_json", "JSON nesnesi gerekli.")
            return None
        return payload

    def do_GET(self) -> None:  # noqa: N802
        path = urlsplit(self.path).path
        try:
            if path == "/status":
                self._json(200, build_status(self.config))
            elif path == "/releases":
                self._json(200, build_releases(self.config))
            elif path == "/packages":
                self._json(200, build_packages(self.config))
            elif path == "/history":
                self._json(200, build_history(self.config))
            else:
                self._error(404, "not_found", "Update Agent uç noktası bulunamadı.")
        except Exception as exc:  # endpoint boundary
            self._error(500, "agent_error", str(exc))

    def do_POST(self) -> None:  # noqa: N802
        if not self._authorized():
            self.close_connection = True
            self._error(401, "unauthorized", "Update Agent anahtarı geçersiz.")
            return
        path = urlsplit(self.path).path
        try:
            if path == "/upload-package":
                self._upload_package()
            elif path == "/verify-package":
                self._verify_package()
            elif path == "/install-package":
                self._install_package()
            elif path == "/rollback":
                self._rollback()
            else:
                self._error(404, "not_found", "Update Agent uç noktası bulunamadı.")
        except ValueError as exc:
            self._error(400, "invalid_request", str(exc))
        except PackageError as exc:
            self._error(422, "package_invalid", str(exc))
        except Exception as exc:  # endpoint boundary
            self._error(500, "agent_error", str(exc))

    def _upload_package(self) -> None:
        try:
            filename = validate_leaf_name(self.headers.get(FILENAME_HEADER, ""), suffix=".popkg")
        except ValueError:
            self._error(400, "invalid_filename", "Geçerli bir .popkg dosya adı gerekli.")
            return
        raw_length = self.headers.get("Content-Length")
        try:
            length = int(raw_length or "0")
        except ValueError:
            self._error(400, "invalid_content_length", "Content-Length geçersiz.")
            return
        if length <= 0:
            self._error(400, "empty_package", "Paket boş olamaz.")
            return
        if length > MAX_PACKAGE_BYTES:
            self.close_connection = True
            self._error(413, "package_too_large", "Paket 512 MB sınırını aşıyor.")
            return

        self.config.packages_dir.mkdir(parents=True, exist_ok=True)
        target = self.config.packages_dir / filename
        verification_path = _verification_path(self.config, filename)
        temp_name = None
        remaining = length
        try:
            with tempfile.NamedTemporaryFile(
                mode="wb", prefix=".upload-", suffix=".tmp", dir=self.config.packages_dir, delete=False
            ) as output:
                temp_name = Path(output.name)
                while remaining:
                    chunk = self.rfile.read(min(1024 * 1024, remaining))
                    if not chunk:
                        raise EOFError("upload body ended early")
                    output.write(chunk)
                    remaining -= len(chunk)
                output.flush()
                os.fsync(output.fileno())
            os.chmod(temp_name, 0o640)
            os.replace(temp_name, target)
            temp_name = None
            try:
                verification_path.unlink()
            except FileNotFoundError:
                pass
        except EOFError:
            if temp_name:
                temp_name.unlink(missing_ok=True)
            self._error(400, "incomplete_upload", "Paket aktarımı tamamlanamadı.")
            return

        package_sha = sha256_file(target)
        _record_history(
            self.config,
            "upload",
            filename,
            {"filename": filename, "size_bytes": length, "sha256": package_sha},
        )
        self._json(201, {"uploaded": True, "filename": filename, "size_bytes": length, "sha256": package_sha, "timestamp": utc_now()})

    def _verify_package(self) -> None:
        body = self._json_body()
        if body is None:
            return
        try:
            filename = validate_leaf_name(str(body.get("filename", "")), suffix=".popkg")
        except ValueError:
            self._error(400, "invalid_filename", "Geçerli bir .popkg dosya adı gerekli.")
            return
        package = self.config.packages_dir / filename
        try:
            verification = verify_package(package)
        except PackageError as exc:
            _record_history(self.config, "verify_failed", filename, {"filename": filename, "valid": False, "error": str(exc)})
            self._error(422, "package_invalid", str(exc))
            return
        self.config.verification_dir.mkdir(parents=True, exist_ok=True)
        sidecar = _verification_path(self.config, filename)
        temp = sidecar.with_suffix(sidecar.suffix + ".tmp")
        temp.write_text(json.dumps(verification, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        os.chmod(temp, 0o640)
        os.replace(temp, sidecar)
        _record_history(self.config, "verify", filename, {"filename": filename, **verification})
        self._json(200, verification)

    def _install_package(self) -> None:
        body = self._json_body()
        if body is None:
            return
        try:
            filename = validate_leaf_name(str(body.get("filename", "")), suffix=".popkg")
        except ValueError:
            self._error(400, "invalid_filename", "Geçerli bir .popkg dosya adı gerekli.")
            return
        package = self.config.packages_dir / filename
        if not package.is_file() or _load_verification(self.config, package) is None:
            self._error(409, "package_not_verified", "Paket güncel içerik için doğrulanmadı.")
            return
        result = self.server.helper_dispatcher({"operation": "install", "filename": filename})  # type: ignore[attr-defined]
        _record_history(self.config, "install", filename, {"filename": filename, "helper": result})
        self._json(200 if result.get("ok") else 409, result)

    def _rollback(self) -> None:
        body = self._json_body()
        if body is None:
            return
        try:
            version = validate_release_id(str(body.get("version", "")))
        except PackageError:
            self._error(400, "invalid_version", "Geçersiz release bilgisi.")
            return
        result = self.server.helper_dispatcher({"operation": "rollback", "version": version})  # type: ignore[attr-defined]
        _record_history(self.config, "rollback", version, {"version": version, "helper": result})
        self._json(200 if result.get("ok") else 409, result)


def create_server(
    config: AgentConfig,
    *,
    port: int | None = None,
    helper_dispatcher: HelperDispatcher | None = None,
) -> UpdateAgentHTTPServer:
    _ensure_layout(config)
    dispatcher = helper_dispatcher or (lambda payload: dispatch_to_helper(config, payload))
    return UpdateAgentHTTPServer((config.bind_host, config.port if port is None else port), UpdateAgentHandler, config, dispatcher)


def main() -> None:
    config = AgentConfig.from_env()
    server = create_server(config)
    host, port = server.server_address
    print(f"PhotoOS Update Agent {AGENT_VERSION} listening on http://{host}:{port}")
    try:
        server.serve_forever(poll_interval=0.5)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
