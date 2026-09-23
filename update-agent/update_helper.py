#!/usr/bin/env python3
from __future__ import annotations

import grp
import json
import os
import pwd
import secrets
import socket
import socketserver
import stat
import struct
import subprocess
import time
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from agent_core import PackageError, safe_extract_verified, utc_now, validate_leaf_name, validate_release_id, verify_package

MAX_REQUEST_BYTES = 64 * 1024
REQUIRED_RELEASE_FILES = ("server", "VERSION", "client/dist/index.html")


@dataclass(frozen=True)
class HelperConfig:
    update_root: Path = Path("/var/lib/photoos/update")
    releases_root: Path = Path("/opt/photoos/releases")
    current_link: Path = Path("/opt/photoos/current")
    socket_path: Path = Path("/run/photoos/update-helper.sock")
    config_file: Path = Path("/etc/photoos/config.toml")
    health_url: str | None = None

    @property
    def packages_dir(self) -> Path:
        return self.update_root / "packages"

    @property
    def history_dir(self) -> Path:
        return self.update_root / "history"

    @classmethod
    def from_env(cls) -> "HelperConfig":
        def p(name: str, default: str) -> Path:
            return Path(os.environ.get(name, default))

        return cls(
            update_root=p("PHOTOOS_UPDATE_ROOT", "/var/lib/photoos/update"),
            releases_root=p("PHOTOOS_RELEASES_ROOT", "/opt/photoos/releases"),
            current_link=p("PHOTOOS_CURRENT_LINK", "/opt/photoos/current"),
            socket_path=p("PHOTOOS_HELPER_SOCKET", "/run/photoos/update-helper.sock"),
            config_file=p("PHOTOOS_CONFIG", "/etc/photoos/config.toml"),
            health_url=os.environ.get("PHOTOOS_SERVER_HEALTH_URL"),
        )


def _local_health_url(config: HelperConfig) -> str:
    if config.health_url:
        return config.health_url
    port = 8080
    try:
        with config.config_file.open("rb") as handle:
            parsed = tomllib.load(handle)
        server = parsed.get("server", {}) if isinstance(parsed, dict) else {}
        if isinstance(server, dict):
            port = int(server.get("port", 8080))
    except (OSError, ValueError, TypeError, tomllib.TOMLDecodeError):
        pass
    return f"http://127.0.0.1:{port}/api/v1/setup/status"


def _restart_photoos() -> None:
    subprocess.run(
        ["systemctl", "restart", "photoos.service"],
        check=True,
        timeout=30,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def _health_probe(config: HelperConfig) -> bool:
    url = _local_health_url(config)
    for _ in range(30):
        try:
            with urllib.request.urlopen(url, timeout=2) as response:
                if 200 <= response.status < 400:
                    return True
        except (OSError, urllib.error.URLError, ValueError):
            pass
        time.sleep(1)
    return False


def _release_valid(path: Path) -> bool:
    return path.is_dir() and not path.is_symlink() and all((path / rel).is_file() for rel in REQUIRED_RELEASE_FILES)


def _write_json_atomic(path: Path, payload: dict, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(f".{path.name}.{secrets.token_hex(4)}.tmp")
    temp.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.chmod(temp, mode)
    os.replace(temp, path)


class ReleaseManager:
    def __init__(
        self,
        config: HelperConfig,
        *,
        restart_service: Callable[[], None] | None = None,
        health_probe: Callable[[], bool] | None = None,
    ) -> None:
        self.config = config
        self.restart_service = restart_service or _restart_photoos
        self.health_probe = health_probe or (lambda: _health_probe(config))

    def handle(self, payload: dict) -> dict:
        if not isinstance(payload, dict):
            return self._failure("invalid_request", "İstek JSON nesnesi olmalıdır.")
        operation = payload.get("operation")
        if operation == "install":
            if set(payload) != {"operation", "filename"}:
                return self._failure("invalid_request", "Install isteği yalnız operation ve filename içerebilir.")
            try:
                filename = validate_leaf_name(str(payload.get("filename", "")), suffix=".popkg")
            except ValueError:
                return self._failure("invalid_request", "Geçersiz paket adı.")
            return self._install(filename)
        if operation == "rollback":
            if set(payload) != {"operation", "version"}:
                return self._failure("invalid_request", "Rollback isteği yalnız operation ve version içerebilir.")
            try:
                version = validate_release_id(str(payload.get("version", "")))
            except PackageError:
                return self._failure("invalid_request", "Geçersiz release adı.")
            return self._rollback(version)
        return self._failure("invalid_request", "Desteklenmeyen helper işlemi.")

    def _failure(self, code: str, message: str, **extra) -> dict:
        return {"ok": False, "error": code, "message": message, "timestamp": utc_now(), **extra}

    def _current_target(self) -> Path | None:
        if not os.path.lexists(self.config.current_link):
            return None
        if not self.config.current_link.is_symlink():
            raise RuntimeError("/opt/photoos/current is not a symlink")
        target = self.config.current_link.resolve(strict=True)
        releases_root = self.config.releases_root.resolve(strict=True)
        if target.parent != releases_root:
            raise RuntimeError("current release is outside releases root")
        return target

    def _atomic_switch(self, target: Path) -> Path | None:
        releases_root = self.config.releases_root.resolve(strict=True)
        resolved_target = target.resolve(strict=True)
        if resolved_target.parent != releases_root or not _release_valid(resolved_target):
            raise RuntimeError("target release is invalid")
        previous = self._current_target()
        self.config.current_link.parent.mkdir(parents=True, exist_ok=True)
        temp_link = self.config.current_link.parent / f".{self.config.current_link.name}.update-{secrets.token_hex(8)}"
        os.symlink(str(resolved_target), temp_link)
        try:
            os.replace(temp_link, self.config.current_link)
        finally:
            if os.path.lexists(temp_link):
                os.unlink(temp_link)
        return previous

    def _restore_previous(self, previous: Path | None) -> None:
        if previous is None:
            if os.path.lexists(self.config.current_link):
                os.unlink(self.config.current_link)
            return
        temp_link = self.config.current_link.parent / f".{self.config.current_link.name}.restore-{secrets.token_hex(8)}"
        os.symlink(str(previous), temp_link)
        try:
            os.replace(temp_link, self.config.current_link)
        finally:
            if os.path.lexists(temp_link):
                os.unlink(temp_link)

    def _activate_with_health(self, target: Path) -> tuple[bool, Path | None, str | None]:
        try:
            previous = self._atomic_switch(target)
        except Exception as exc:
            return False, None, f"release switch failed: {exc}"
        try:
            self.restart_service()
        except Exception as exc:
            self._restore_previous(previous)
            try:
                if previous is not None:
                    self.restart_service()
            except Exception:
                pass
            return False, previous, f"photoos restart failed: {exc}"
        if self.health_probe():
            return True, previous, None
        self._restore_previous(previous)
        try:
            if previous is not None:
                self.restart_service()
        except Exception:
            pass
        return False, previous, "health check failed"

    def _record(self, action: str, name: str, payload: dict) -> None:
        self.config.history_dir.mkdir(parents=True, exist_ok=True)
        stamp = utc_now().replace(":", "").replace("-", "").replace(".", "")
        safe_name = validate_leaf_name(name)
        path = self.config.history_dir / f"{stamp}-{secrets.token_hex(4)}-helper-{safe_name}.json"
        _write_json_atomic(path, {"action": action, "timestamp": utc_now(), **payload})

    def _install(self, filename: str) -> dict:
        package = self.config.packages_dir / filename
        if package.is_symlink() or not package.is_file():
            return self._failure("package_missing", "Paket bulunamadı.")
        try:
            verification = verify_package(package)
        except PackageError as exc:
            return self._failure("package_invalid", str(exc))
        except Exception as exc:
            return self._failure("install_failed", f"Paket doğrulanamadı: {exc}")

        release_id = verification["release_id"]
        try:
            validate_release_id(release_id)
        except PackageError:
            return self._failure("package_invalid", "Paket release kimliği geçersiz.")

        self.config.releases_root.mkdir(parents=True, exist_ok=True)
        destination = self.config.releases_root / release_id
        if os.path.lexists(destination):
            return self._failure("release_exists", "Aynı release zaten mevcut.", release_id=release_id)

        staging = self.config.releases_root / f".{release_id}.installing-{secrets.token_hex(8)}"
        try:
            safe_extract_verified(package, staging, verification)
            if not _release_valid(staging):
                raise RuntimeError("extracted release is incomplete")
            installation = {
                "product": "PhotoOS",
                "operation": "install",
                "installed_at": utc_now(),
                "version": verification["version"],
                "release_id": release_id,
                "package": filename,
                "package_sha256": verification["package_sha256"],
            }
            _write_json_atomic(staging / "INSTALLATION.json", installation)
            os.replace(staging, destination)
        except Exception as exc:
            if staging.exists():
                import shutil

                shutil.rmtree(staging, ignore_errors=True)
            return self._failure("install_failed", f"Release hazırlanamadı: {exc}")

        ok, previous, reason = self._activate_with_health(destination)
        previous_text = str(previous) if previous else None
        if not ok:
            result = self._failure(
                "health_check_failed" if reason == "health check failed" else "activation_failed",
                "Yeni release sağlık kontrolünü geçemedi; önceki release geri yüklendi." if reason == "health check failed" else str(reason),
                release_id=release_id,
                previous_release=previous_text,
                attempted_release=str(destination),
            )
            self._record("install_failed", filename, result)
            return result

        result = {
            "ok": True,
            "operation": "install",
            "message": "PhotoOS release kuruldu ve sağlık kontrolü geçti.",
            "version": verification["version"],
            "release_id": release_id,
            "previous_release": previous_text,
            "active_release": str(destination),
            "timestamp": utc_now(),
        }
        self._record("install", filename, result)
        return result

    def _rollback(self, version: str) -> dict:
        target = self.config.releases_root / version
        if target.is_symlink() or not _release_valid(target):
            return self._failure("release_missing", "Geri dönülecek release bulunamadı veya geçersiz.")
        try:
            current = self._current_target()
        except Exception as exc:
            return self._failure("current_release_invalid", str(exc))
        if current is not None and current.resolve() == target.resolve():
            return {
                "ok": True,
                "operation": "rollback",
                "message": "İstenen release zaten aktif.",
                "active_release": str(target),
                "timestamp": utc_now(),
            }

        ok, previous, reason = self._activate_with_health(target)
        previous_text = str(previous) if previous else None
        if not ok:
            result = self._failure(
                "health_check_failed" if reason == "health check failed" else "activation_failed",
                "Rollback hedefi sağlık kontrolünü geçemedi; önceki release geri yüklendi." if reason == "health check failed" else str(reason),
                previous_release=previous_text,
                attempted_release=str(target),
            )
            self._record("rollback_failed", version, result)
            return result

        result = {
            "ok": True,
            "operation": "rollback",
            "message": "PhotoOS release geri dönüşü tamamlandı.",
            "previous_release": previous_text,
            "active_release": str(target),
            "timestamp": utc_now(),
        }
        self._record("rollback", version, result)
        return result


class _ThreadingUnixServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, socket_path: str, handler, *, manager: ReleaseManager, allowed_uids: set[int] | None):
        self.manager = manager
        self.allowed_uids = allowed_uids
        super().__init__(socket_path, handler)


class HelperRequestHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        allowed_uids = self.server.allowed_uids  # type: ignore[attr-defined]
        if allowed_uids is not None:
            try:
                credentials = self.request.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i"))
                _, uid, _ = struct.unpack("3i", credentials)
            except (AttributeError, OSError, struct.error):
                self._send({"ok": False, "error": "peer_credentials_unavailable", "message": "Peer kimliği doğrulanamadı."})
                return
            if uid not in allowed_uids:
                self._send({"ok": False, "error": "forbidden_peer", "message": "Helper istemcisine izin verilmiyor."})
                return

        raw = self.rfile.read(MAX_REQUEST_BYTES + 1)
        if len(raw) > MAX_REQUEST_BYTES:
            self._send({"ok": False, "error": "request_too_large", "message": "Helper isteği çok büyük."})
            return
        lines = [line for line in raw.splitlines() if line.strip()]
        if len(lines) != 1:
            self._send({"ok": False, "error": "invalid_request", "message": "Tek JSON isteği gerekli."})
            return
        try:
            payload = json.loads(lines[0].decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            self._send({"ok": False, "error": "invalid_json", "message": "Geçerli JSON gerekli."})
            return
        result = self.server.manager.handle(payload)  # type: ignore[attr-defined]
        self._send(result)

    def _send(self, payload: dict) -> None:
        self.wfile.write(json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n")


def create_helper_server(
    config: HelperConfig,
    *,
    manager: ReleaseManager | None = None,
    manage_permissions: bool = True,
) -> _ThreadingUnixServer:
    config.socket_path.parent.mkdir(parents=True, exist_ok=True)
    if os.path.lexists(config.socket_path):
        if config.socket_path.is_symlink() or not stat_is_socket(config.socket_path):
            raise RuntimeError("refusing to replace non-socket helper path")
        os.unlink(config.socket_path)

    allowed_uids: set[int] | None = None
    group_gid = None
    if manage_permissions:
        try:
            photoos_uid = pwd.getpwnam("photoos").pw_uid
            group_gid = grp.getgrnam("photoos").gr_gid
        except KeyError as exc:
            raise RuntimeError("photoos user/group is required") from exc
        allowed_uids = {0, photoos_uid}

    server = _ThreadingUnixServer(str(config.socket_path), HelperRequestHandler, manager=manager or ReleaseManager(config), allowed_uids=allowed_uids)
    if manage_permissions:
        os.chown(config.socket_path, 0, group_gid)
        os.chmod(config.socket_path, 0o660)
    return server


def stat_is_socket(path: Path) -> bool:
    try:
        return stat.S_ISSOCK(path.lstat().st_mode)
    except OSError:
        return False


def main() -> None:
    if os.geteuid() != 0:
        raise SystemExit("PhotoOS update helper must run as root")
    config = HelperConfig.from_env()
    server = create_helper_server(config)
    print(f"PhotoOS Update Helper listening on {config.socket_path}")
    try:
        server.serve_forever(poll_interval=0.5)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        if os.path.lexists(config.socket_path):
            os.unlink(config.socket_path)


if __name__ == "__main__":
    main()
