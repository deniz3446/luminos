from __future__ import annotations

import hashlib
import io
import json
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

AGENT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(AGENT_ROOT))

from agent_core import PackageError, verify_package, safe_extract_verified, validate_leaf_name  # noqa: E402


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def add_bytes(tf: tarfile.TarFile, name: str, data: bytes, mode: int = 0o644) -> None:
    info = tarfile.TarInfo(name=name)
    info.size = len(data)
    info.mode = mode
    tf.addfile(info, io.BytesIO(data))


def manifest_for(files: dict[str, bytes], *, release_id: str = "1.2.1-test-20260918-010203") -> dict:
    return {
        "product": "PhotoOS",
        "format_version": 1,
        "version": "1.2.1",
        "release_id": release_id,
        "channel": "stable",
        "release_notes": "test release",
        "files": [
            {"path": path, "size": len(data), "sha256": digest(data)}
            for path, data in sorted(files.items())
        ],
    }


def write_package(path: Path, files: dict[str, bytes], *, manifest: dict | None = None, extras=None) -> None:
    payload_manifest = manifest if manifest is not None else manifest_for(files)
    with tarfile.open(path, "w:gz") as tf:
        add_bytes(tf, "manifest.json", json.dumps(payload_manifest).encode("utf-8"))
        for rel, data in files.items():
            add_bytes(tf, f"release/{rel}", data, 0o755 if rel == "server" else 0o644)
        if extras:
            for extra in extras:
                tf.addfile(extra[0], extra[1] if len(extra) > 1 else None)


class PackageVerificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.valid_files = {
            "server": b"ELF-placeholder",
            "VERSION": b"1.2.1\n",
            "client/dist/index.html": b"<html>PhotoOS</html>",
            "config/config.toml": b"[release]\nversion='1.2.1'\n",
        }

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_valid_package_is_verified_and_extracts_exact_regular_files(self) -> None:
        package = self.root / "photoos.popkg"
        write_package(package, self.valid_files)

        verification = verify_package(package)

        self.assertTrue(verification["valid"])
        self.assertEqual(verification["version"], "1.2.1")
        self.assertEqual(verification["release_id"], "1.2.1-test-20260918-010203")
        self.assertEqual(verification["verified_files"], len(self.valid_files))
        self.assertEqual(verification["package_sha256"], digest(package.read_bytes()))

        destination = self.root / "extract"
        safe_extract_verified(package, destination, verification)
        for rel, data in self.valid_files.items():
            self.assertEqual((destination / rel).read_bytes(), data)
        self.assertFalse((destination / "manifest.json").exists())

    def test_path_traversal_member_is_rejected(self) -> None:
        package = self.root / "bad.popkg"
        with tarfile.open(package, "w:gz") as tf:
            add_bytes(tf, "manifest.json", json.dumps(manifest_for(self.valid_files)).encode())
            add_bytes(tf, "release/../escape", b"bad")
            for rel, data in self.valid_files.items():
                add_bytes(tf, f"release/{rel}", data)

        with self.assertRaisesRegex(PackageError, "unsafe|path|member"):
            verify_package(package)

    def test_symlink_member_is_rejected(self) -> None:
        package = self.root / "link.popkg"
        link = tarfile.TarInfo("release/evil-link")
        link.type = tarfile.SYMTYPE
        link.linkname = "/etc/passwd"
        with tarfile.open(package, "w:gz") as tf:
            add_bytes(tf, "manifest.json", json.dumps(manifest_for(self.valid_files)).encode())
            for rel, data in self.valid_files.items():
                add_bytes(tf, f"release/{rel}", data)
            tf.addfile(link)

        with self.assertRaisesRegex(PackageError, "regular|link|member"):
            verify_package(package)

    def test_duplicate_archive_member_is_rejected(self) -> None:
        package = self.root / "duplicate.popkg"
        with tarfile.open(package, "w:gz") as tf:
            add_bytes(tf, "manifest.json", json.dumps(manifest_for(self.valid_files)).encode())
            for rel, data in self.valid_files.items():
                add_bytes(tf, f"release/{rel}", data)
            add_bytes(tf, "release/server", self.valid_files["server"])

        with self.assertRaisesRegex(PackageError, "duplicate"):
            verify_package(package)

    def test_missing_critical_release_file_is_rejected(self) -> None:
        files = dict(self.valid_files)
        files.pop("client/dist/index.html")
        package = self.root / "missing.popkg"
        write_package(package, files)

        with self.assertRaisesRegex(PackageError, "critical"):
            verify_package(package)

    def test_hash_mismatch_is_rejected(self) -> None:
        package = self.root / "hash.popkg"
        manifest = manifest_for(self.valid_files)
        manifest["files"][0]["sha256"] = "0" * 64
        write_package(package, self.valid_files, manifest=manifest)

        with self.assertRaisesRegex(PackageError, "sha256|hash"):
            verify_package(package)

    def test_unlisted_release_file_is_rejected(self) -> None:
        package = self.root / "extra.popkg"
        manifest = manifest_for(self.valid_files)
        files = dict(self.valid_files)
        files["unexpected.txt"] = b"not in manifest"
        write_package(package, files, manifest=manifest)

        with self.assertRaisesRegex(PackageError, "manifest|listed|file"):
            verify_package(package)

    def test_leaf_name_rejects_traversal_and_wrong_suffix(self) -> None:
        self.assertEqual(validate_leaf_name("safe.popkg", suffix=".popkg"), "safe.popkg")
        for value in ("../x.popkg", "a/b.popkg", "a\\b.popkg", ".popkg", ""):
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    validate_leaf_name(value, suffix=".popkg")
        with self.assertRaises(ValueError):
            validate_leaf_name("safe.txt", suffix=".popkg")


if __name__ == "__main__":
    unittest.main()

# Wave2 permission hotfix regression test.
import hashlib as _wave2_hashlib
import io as _wave2_io
import json as _wave2_json
import os as _wave2_os
from pathlib import Path as _Wave2Path
import stat as _wave2_stat
import tarfile as _wave2_tarfile
import tempfile as _wave2_tempfile
import unittest as _wave2_unittest

from agent_core import safe_extract_verified as _wave2_safe_extract_verified
from agent_core import verify_package as _wave2_verify_package


class Wave2ReleaseDirectoryPermissionTests(_wave2_unittest.TestCase):
    def test_safe_extract_normalizes_release_directories_under_restrictive_umask(self):
        files = {
            "server": b"server-binary",
            "VERSION": b"1.2.1\n",
            "client/dist/index.html": b"<html>ok</html>",
            "nested/deeper/evidence.txt": b"ok\n",
        }

        def digest(data):
            return _wave2_hashlib.sha256(data).hexdigest()

        manifest = {
            "product": "PhotoOS",
            "format_version": 1,
            "version": "1.2.1",
            "release_id": "1.2.1-permission-regression",
            "channel": "stable",
            "release_notes": "permission regression",
            "files": [
                {"path": rel, "size": len(data), "sha256": digest(data)}
                for rel, data in sorted(files.items())
            ],
        }

        with _wave2_tempfile.TemporaryDirectory() as td:
            root = _Wave2Path(td)
            package = root / "permission.popkg"
            destination = root / "release"

            with _wave2_tarfile.open(package, "w:gz") as tf:
                raw = _wave2_json.dumps(manifest).encode("utf-8")
                info = _wave2_tarfile.TarInfo("manifest.json")
                info.size = len(raw)
                tf.addfile(info, _wave2_io.BytesIO(raw))
                for rel, data in files.items():
                    info = _wave2_tarfile.TarInfo(f"release/{rel}")
                    info.size = len(data)
                    info.mode = 0o755 if rel == "server" else 0o644
                    tf.addfile(info, _wave2_io.BytesIO(data))

            verification = _wave2_verify_package(package)
            previous_umask = _wave2_os.umask(0o027)
            try:
                _wave2_safe_extract_verified(package, destination, verification)
            finally:
                _wave2_os.umask(previous_umask)

            for rel in (".", "client", "client/dist", "nested", "nested/deeper"):
                mode = _wave2_stat.S_IMODE((destination / rel).stat().st_mode)
                self.assertEqual(
                    mode,
                    0o755,
                    f"{rel} must be traversable by photoos under restrictive helper umask",
                )
