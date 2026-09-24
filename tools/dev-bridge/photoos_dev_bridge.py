from __future__ import annotations

import os
import re
import shlex
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = Path(r"C:\Users\MSI\PhotoOS-Dev")
STATUS_FILE = ROOT / "last-status.json"


def sanitize_branch_name(text: str) -> str:
    text = text.translate(str.maketrans({"ı": "i", "İ": "I"}))
    text = unicodedata.normalize("NFKD", text)
    text = "".join(ch for ch in text if not unicodedata.combining(ch))
    slug = re.sub(r"[^a-zA-Z0-9]+", "-", text).strip("-").lower()
    if not slug:
        slug = "task"
    return f"ai/{slug[:60]}"


def frontend_gate_commands(windows: bool | None = None) -> list[list[str]]:
    if windows is None:
        windows = os.name == "nt"
    npm = "npm.cmd" if windows else "npm"
    return [
        [npm, "run", "contract-check"],
        [npm, "run", "build"],
    ]


def backend_gate_command(worktree: str) -> str:
    safe = shlex.quote(worktree)
    return (
        f"cd {safe} && "
        "PATH=/home/photoos/.cargo/bin:$PATH cargo test --workspace && "
        "PATH=/home/photoos/.cargo/bin:$PATH cargo check --workspace"
    )


def remote_commander_command(windows: bool | None = None) -> list[str]:
    if windows is None:
        windows = os.name == "nt"
    npx = "npx.cmd" if windows else "npx"
    return [
        npx,
        "@wonderwhy-er/desktop-commander@latest",
        "remote",
    ]



def _default_repo_reader(kind: tuple[str, ...]) -> str:
    import subprocess

    commands = {
        ("branch",): ["git", "branch", "--show-current"],
        ("status",): ["git", "status", "--porcelain=v1"],
        ("head",): ["git", "rev-parse", "HEAD"],
        ("remote",): ["git", "remote", "get-url", "origin"],
    }
    return subprocess.check_output(
        commands[kind],
        cwd=REPO,
        text=True,
        stderr=subprocess.STDOUT,
    )


def repo_snapshot(reader=None) -> dict[str, object]:
    reader = reader or _default_repo_reader
    branch = reader(("branch",)).strip()
    status = reader(("status",)).strip()
    head = reader(("head",)).strip()
    remote = reader(("remote",)).strip()
    return {
        "branch": branch,
        "clean": not bool(status),
        "head": head,
        "remote": remote,
    }



def status_lines(
    repo_info: dict[str, object],
    last_status: dict[str, object] | None = None,
) -> list[str]:
    last_status = last_status or {}
    clean = "TEMİZ" if repo_info.get("clean") else "DEĞİŞİKLİK VAR"
    head = str(repo_info.get("head", ""))[:12]
    return [
        f"Branch: {repo_info.get('branch', '-')}",
        f"Çalışma ağacı: {clean}",
        f"Commit: {head or '-'}",
        f"Remote: {repo_info.get('remote', '-')}",
        f"Doğrulama: {last_status.get('verification', 'BİLİNMİYOR')}",
        f"Frontend: {last_status.get('frontend', 'BİLİNMİYOR')}",
        f"Backend: {last_status.get('backend', 'BİLİNMİYOR')}",
    ]



def load_status_file(path: Path = STATUS_FILE) -> dict[str, object]:
    import json

    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def save_status_file(
    payload: dict[str, object],
    path: Path = STATUS_FILE,
) -> None:
    import json

    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
