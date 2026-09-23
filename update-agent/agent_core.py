from __future__ import annotations

import gzip
import hashlib
import json
import re
import tarfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import BinaryIO

AGENT_VERSION = "1.0.0"
PACKAGE_FORMAT_VERSION = 1
PRODUCT = "PhotoOS"
CRITICAL_RELEASE_FILES = frozenset({"server", "VERSION", "client/dist/index.html"})
_SAFE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$")
_SHA256 = re.compile(r"^[0-9a-f]{64}$")


class PackageError(ValueError):
    pass


@dataclass(frozen=True)
class PackageLimits:
    max_compressed_bytes: int = 512 * 1024 * 1024
    max_expanded_bytes: int = 2 * 1024 * 1024 * 1024
    max_members: int = 100_000
    max_manifest_bytes: int = 4 * 1024 * 1024


DEFAULT_LIMITS = PackageLimits()


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path, chunk_size: int = 1024 * 1024) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(chunk_size):
            digest.update(chunk)
    return digest.hexdigest()


def validate_leaf_name(value: str, *, suffix: str | None = None) -> str:
    if not isinstance(value, str):
        raise ValueError("name must be text")
    name = value.strip()
    if not name or name in {".", ".."} or "\x00" in name:
        raise ValueError("unsafe leaf name")
    if "/" in name or "\\" in name:
        raise ValueError("unsafe leaf name")
    if suffix is not None:
        if not name.endswith(suffix) or len(name) <= len(suffix):
            raise ValueError(f"name must end with {suffix}")
    return name


def validate_release_id(value: str) -> str:
    if not isinstance(value, str) or not _SAFE_ID.fullmatch(value.strip()):
        raise PackageError("unsafe release identifier")
    return value.strip()


def _safe_relative_path(value: str) -> str:
    if not isinstance(value, str):
        raise PackageError("unsafe package path")
    if not value or "\x00" in value or "\\" in value or value.startswith("/"):
        raise PackageError("unsafe package path")
    pieces = value.split("/")
    if any(piece in {"", ".", ".."} for piece in pieces):
        raise PackageError("unsafe package path")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        raise PackageError("unsafe package path")
    return path.as_posix()


def _read_exact_member(tf: tarfile.TarFile, member: tarfile.TarInfo, maximum: int) -> bytes:
    if member.size < 0 or member.size > maximum:
        raise PackageError("package member exceeds allowed size")
    handle = tf.extractfile(member)
    if handle is None:
        raise PackageError("package member cannot be read")
    data = handle.read(maximum + 1)
    if len(data) != member.size or len(data) > maximum:
        raise PackageError("package member size mismatch")
    return data


def _hash_member(handle: BinaryIO) -> tuple[int, str]:
    digest = hashlib.sha256()
    size = 0
    while chunk := handle.read(1024 * 1024):
        size += len(chunk)
        digest.update(chunk)
    return size, digest.hexdigest()


def _parse_manifest(data: bytes) -> dict:
    try:
        payload = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PackageError("manifest is not valid UTF-8 JSON") from exc
    if not isinstance(payload, dict):
        raise PackageError("manifest must be an object")
    if payload.get("product") != PRODUCT:
        raise PackageError("manifest product mismatch")
    if payload.get("format_version") != PACKAGE_FORMAT_VERSION:
        raise PackageError("unsupported package format")
    version = payload.get("version")
    release_id = payload.get("release_id")
    if not isinstance(version, str) or not _SAFE_ID.fullmatch(version.strip()):
        raise PackageError("unsafe version identifier")
    validate_release_id(release_id)
    files = payload.get("files")
    if not isinstance(files, list) or not files:
        raise PackageError("manifest files list is required")
    return payload


def verify_package(path: Path, limits: PackageLimits = DEFAULT_LIMITS) -> dict:
    package = Path(path)
    if not package.is_file():
        raise PackageError("package file does not exist")
    size_on_disk = package.stat().st_size
    if size_on_disk <= 0 or size_on_disk > limits.max_compressed_bytes:
        raise PackageError("package compressed size is outside allowed limits")

    package_sha = sha256_file(package)
    try:
        expanded = 0
        with gzip.open(package, "rb") as stream:
            while chunk := stream.read(1024 * 1024):
                expanded += len(chunk)
                if expanded > limits.max_expanded_bytes:
                    raise PackageError("package expanded size exceeds limit")
    except PackageError:
        raise
    except (OSError, EOFError, gzip.BadGzipFile) as exc:
        raise PackageError("package gzip stream is corrupt") from exc

    try:
        tf = tarfile.open(package, mode="r:gz")
    except (tarfile.TarError, OSError) as exc:
        raise PackageError("package is not a valid gzip tar archive") from exc

    with tf:
        members = tf.getmembers()
        if not members or len(members) > limits.max_members:
            raise PackageError("package member count is outside allowed limits")

        by_name: dict[str, tarfile.TarInfo] = {}
        expanded_total = 0
        for member in members:
            name = _safe_relative_path(member.name)
            if name in by_name:
                raise PackageError(f"duplicate package member: {name}")
            by_name[name] = member
            if not member.isfile():
                raise PackageError(f"package member is not a regular file: {name}")
            if member.size < 0:
                raise PackageError("invalid package member size")
            expanded_total += member.size
            if expanded_total > limits.max_expanded_bytes:
                raise PackageError("package expanded size exceeds limit")

        manifest_member = by_name.get("manifest.json")
        if manifest_member is None:
            raise PackageError("manifest.json is missing")
        manifest = _parse_manifest(
            _read_exact_member(tf, manifest_member, limits.max_manifest_bytes)
        )

        declared: dict[str, dict] = {}
        for raw_entry in manifest["files"]:
            if not isinstance(raw_entry, dict):
                raise PackageError("manifest file entry must be an object")
            rel = _safe_relative_path(raw_entry.get("path"))
            if rel in declared:
                raise PackageError(f"duplicate manifest file: {rel}")
            expected_size = raw_entry.get("size")
            expected_sha = raw_entry.get("sha256")
            if not isinstance(expected_size, int) or expected_size < 0:
                raise PackageError(f"manifest file size is invalid: {rel}")
            if not isinstance(expected_sha, str) or not _SHA256.fullmatch(expected_sha):
                raise PackageError(f"manifest sha256 is invalid: {rel}")
            declared[rel] = raw_entry

        missing_critical = sorted(CRITICAL_RELEASE_FILES.difference(declared))
        if missing_critical:
            raise PackageError(
                "critical release files are missing: " + ", ".join(missing_critical)
            )

        archive_release: dict[str, tarfile.TarInfo] = {}
        for name, member in by_name.items():
            if name == "manifest.json":
                continue
            if not name.startswith("release/"):
                raise PackageError(f"unexpected package file outside release/: {name}")
            rel = name.removeprefix("release/")
            rel = _safe_relative_path(rel)
            archive_release[rel] = member

        if set(archive_release) != set(declared):
            missing = sorted(set(declared) - set(archive_release))
            extra = sorted(set(archive_release) - set(declared))
            detail = []
            if missing:
                detail.append("missing=" + ",".join(missing))
            if extra:
                detail.append("unlisted=" + ",".join(extra))
            raise PackageError("manifest file list mismatch: " + " ".join(detail))

        for rel, entry in declared.items():
            member = archive_release[rel]
            handle = tf.extractfile(member)
            if handle is None:
                raise PackageError(f"release file cannot be read: {rel}")
            actual_size, actual_sha = _hash_member(handle)
            if actual_size != entry["size"]:
                raise PackageError(f"file size mismatch: {rel}")
            if actual_sha != entry["sha256"]:
                raise PackageError(f"sha256 hash mismatch: {rel}")

    return {
        "valid": True,
        "version": manifest["version"].strip(),
        "release_id": manifest["release_id"].strip(),
        "verified_at": utc_now(),
        "verified_files": len(declared),
        "package_sha256": package_sha,
        "manifest": manifest,
    }


def safe_extract_verified(package: Path, destination: Path, verification: dict) -> None:
    package = Path(package)
    destination = Path(destination)
    expected_sha = verification.get("package_sha256") if isinstance(verification, dict) else None
    if not isinstance(expected_sha, str) or sha256_file(package) != expected_sha:
        raise PackageError("package changed after verification")

    fresh = verify_package(package)
    if fresh["package_sha256"] != expected_sha:
        raise PackageError("package changed after verification")

    destination.mkdir(parents=True, exist_ok=False)
    destination.chmod(0o755)
    root = destination.resolve()

    with tarfile.open(package, mode="r:gz") as tf:
        members = {member.name: member for member in tf.getmembers()}
        for entry in fresh["manifest"]["files"]:
            rel = _safe_relative_path(entry["path"])
            member = members.get(f"release/{rel}")
            if member is None or not member.isfile():
                raise PackageError(f"verified release member disappeared: {rel}")
            target = destination.joinpath(*PurePosixPath(rel).parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            resolved_parent = target.parent.resolve()
            if root != resolved_parent and root not in resolved_parent.parents:
                raise PackageError("extraction target escaped destination")
            if any(parent.is_symlink() for parent in [target.parent, *target.parents] if parent.exists()):
                raise PackageError("extraction path contains a symlink")

            current = target.parent
            while True:
                current.chmod(0o755)
                if current == destination:
                    break
                current = current.parent

            source = tf.extractfile(member)
            if source is None:
                raise PackageError(f"release member cannot be read: {rel}")
            with target.open("xb") as output:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
            target.chmod(0o755 if rel == "server" else 0o644)
