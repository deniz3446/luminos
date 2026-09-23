# PhotoOS Final Source Reconstruction — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recover every usable original PhotoOS source artifact, build a provenance inventory, and create a new isolated `/home/photoos/PhotoOS-final-source` canonical workspace without modifying the active release.

**Architecture:** Phase 1 treats the currently running PhotoOS as immutable reference state. Historical archives and recovery trees are scanned read-only, candidate sources are extracted into an isolated reconstruction workspace, exact duplicates are de-duplicated by SHA256, and every accepted file receives provenance metadata. The canonical tree is assembled only from verified sources; unresolved modules remain explicitly marked in the gap report rather than silently fabricated.

**Tech Stack:** Debian 12, Bash, Python 3, Git, Rust/Cargo workspace layout, React/TypeScript/Vite source layout, SHA256, tar/zip archive inspection.

**Spec:** `docs/superpowers/specs/2026-09-12-photoos-final-source-design.md`

## Global Constraints

- The active release under `/opt/photoos/current` must not be overwritten during reconstruction.
- Existing recovery trees must not be deleted.
- Existing repair backups under `/var/backups` must not be deleted.
- Reconstruction work happens under `/home/photoos/PhotoOS-final-source` and `/home/photoos/PhotoOS-reconstruction`.
- Exact recovered source has priority over recreated source.
- Live runtime behavior has priority over incomplete recovery implementations.
- No hard-coded LAN IPs are allowed in canonical frontend source.
- No hard-coded JWT secret is allowed in canonical backend source.
- Final product version target is `1.2.1`.
- Build-specific release IDs use `1.2.1-final-YYYYMMDD-HHMMSS`.
- No production ISO may be built from canonical source until all later verification gates pass.

---

## Plan Decomposition

This design is too broad for one implementation plan. It is split into four independently reviewable plans:

1. **Phase 1 — Source recovery and canonical workspace** — this document.
2. **Phase 2 — Backend completion, auth/security, and API parity.**
3. **Phase 3 — Frontend reconstruction and UI parity.**
4. **Phase 4 — Installer/release integration, clean-install validation, and final ISO.**

Phase 2 may start only after Phase 1 produces a gap report and a clean canonical baseline.

---

## File Structure Locked by Phase 1

The canonical workspace will have this shape:

```text
/home/photoos/PhotoOS-final-source/
  .git/
  Cargo.toml
  Cargo.lock
  server/
  client/
  installer/
  systemd/
  nginx/
  scripts/
  docs/
    reconstruction/
      provenance.json
      provenance.md
      gaps.md
      source-selection.md
    release/
  tests/
  reports/
```

The reconstruction workspace will have this shape:

```text
/home/photoos/PhotoOS-reconstruction/
  extracted/
  candidates/
  indexes/
  reports/
  staging/
```

Responsibilities:

- `PhotoOS-reconstruction/extracted/`: extracted nested archives; never used directly for production build.
- `PhotoOS-reconstruction/candidates/`: de-duplicated candidate source files.
- `PhotoOS-reconstruction/indexes/source-index.json`: machine-readable candidate inventory.
- `PhotoOS-reconstruction/reports/`: scan and selection reports.
- `PhotoOS-final-source/`: only canonical accepted source.
- `docs/reconstruction/provenance.json`: exact origin and SHA256 of each accepted canonical file.
- `docs/reconstruction/gaps.md`: known missing/incomplete modules after recovery.
- `docs/reconstruction/source-selection.md`: why each source family was selected.

---

### Task 1: Freeze and Fingerprint Current Reference Product

**Files:**
- Create: `/home/photoos/PhotoOS-reconstruction/reports/live-reference.txt`
- Create: `/home/photoos/PhotoOS-reconstruction/indexes/live-reference.json`
- Test: shell verification commands in this task

**Interfaces:**
- Consumes: `/opt/photoos/current`, `/etc/photoos`, `/etc/nginx`, systemd units.
- Produces: immutable reference hashes and release identity used by all later parity checks.

- [ ] **Step 1: Create reconstruction directories**

Run:

```bash
sudo install -d -o photoos -g photoos \
  /home/photoos/PhotoOS-reconstruction/{extracted,candidates,indexes,reports,staging}
```

Expected: all five directories exist and are owned by `photoos:photoos`.

- [ ] **Step 2: Capture the active release target and file hashes**

Run as `photoos`:

```bash
CURRENT="$(readlink -f /opt/photoos/current)"
{
  echo "captured_at=$(date -Is)"
  echo "current=$CURRENT"
  echo "hostname=$(hostname)"
  echo "kernel=$(uname -r)"
  echo
  echo "== release manifest =="
  cat "$CURRENT/release-manifest.json" 2>/dev/null || true
  echo
  echo "== config =="
  sed -n '1,240p' /etc/photoos/config.toml
  echo
  echo "== main binary =="
  sha256sum "$CURRENT/server"
  echo
  echo "== frontend =="
  find "$CURRENT/client/dist" -type f -print0 | sort -z | xargs -0 sha256sum
} > /home/photoos/PhotoOS-reconstruction/reports/live-reference.txt
```

Expected: report contains the current release path, server hash, and all frontend hashes.

- [ ] **Step 3: Write machine-readable reference metadata**

Run:

```bash
python3 - <<'PY'
from pathlib import Path
import hashlib, json, os, subprocess, datetime

current = Path(os.path.realpath("/opt/photoos/current"))
files = {}

for p in [current / "server", *sorted((current / "client" / "dist").rglob("*"))]:
    if p.is_file():
        files[str(p)] = hashlib.sha256(p.read_bytes()).hexdigest()

out = {
    "captured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "current_release": str(current),
    "files": files,
}
Path("/home/photoos/PhotoOS-reconstruction/indexes/live-reference.json").write_text(
    json.dumps(out, indent=2, sort_keys=True) + "\n"
)
PY
```

Expected: JSON parses successfully.

- [ ] **Step 4: Verify reference files are readable and unchanged**

Run:

```bash
python3 -m json.tool \
  /home/photoos/PhotoOS-reconstruction/indexes/live-reference.json >/dev/null

test -s /home/photoos/PhotoOS-reconstruction/reports/live-reference.txt
```

Expected: exit code `0`.

- [ ] **Step 5: Commit reconstruction metadata only after canonical Git repo exists**

No commit in Task 1. The reconstruction workspace is evidence, not the final repository.

---

### Task 2: Deep Nested Archive Discovery

**Files:**
- Create: `/home/photoos/PhotoOS-reconstruction/indexes/archive-index.json`
- Create: `/home/photoos/PhotoOS-reconstruction/reports/archive-scan.txt`
- Test: archive index validation

**Interfaces:**
- Consumes: `/home/photoos/PhotoOS-recovery` and historical PhotoOS artifacts under `/home/photoos`.
- Produces: an archive index with path, type, size, mtime, SHA256, and extraction status.

- [ ] **Step 1: Write archive discovery script**

Create `/home/photoos/PhotoOS-reconstruction/staging/index_archives.py`:

```python
from __future__ import annotations

from pathlib import Path
import hashlib
import json
import os

ROOTS = [
    Path("/home/photoos/PhotoOS-recovery"),
    Path("/home/photoos"),
]

SUFFIXES = (
    ".tar.gz",
    ".tgz",
    ".tar",
    ".zip",
)

def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

seen: set[tuple[int, int]] = set()
items = []

for root in ROOTS:
    if not root.exists():
        continue
    for p in root.rglob("*"):
        if not p.is_file():
            continue
        name = p.name.lower()
        if not any(name.endswith(s) for s in SUFFIXES):
            continue

        st = p.stat()
        inode_key = (st.st_dev, st.st_ino)
        if inode_key in seen:
            continue
        seen.add(inode_key)

        items.append({
            "path": str(p),
            "size": st.st_size,
            "mtime_ns": st.st_mtime_ns,
            "sha256": sha256(p),
        })

items.sort(key=lambda x: x["path"])

out = Path("/home/photoos/PhotoOS-reconstruction/indexes/archive-index.json")
out.write_text(json.dumps(items, indent=2, sort_keys=True) + "\n")

report = Path("/home/photoos/PhotoOS-reconstruction/reports/archive-scan.txt")
report.write_text(
    "\n".join(
        f'{x["sha256"]}  {x["size"]:>12}  {x["path"]}'
        for x in items
    ) + "\n"
)

print(f"archives={len(items)}")
```

- [ ] **Step 2: Run archive discovery**

Run:

```bash
python3 /home/photoos/PhotoOS-reconstruction/staging/index_archives.py
```

Expected: prints `archives=N` where `N >= 1`.

- [ ] **Step 3: Validate archive index**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path

p = Path("/home/photoos/PhotoOS-reconstruction/indexes/archive-index.json")
items = json.loads(p.read_text())

assert isinstance(items, list)
assert all({"path", "size", "mtime_ns", "sha256"} <= set(x) for x in items)
assert len({x["sha256"] for x in items}) <= len(items)

print(f"PASS archive index: {len(items)} entries")
PY
```

Expected: `PASS archive index: ... entries`.

---

### Task 3: Safely Extract Nested Archives

**Files:**
- Create: `/home/photoos/PhotoOS-reconstruction/staging/extract_archives.py`
- Create/Modify: `/home/photoos/PhotoOS-reconstruction/indexes/archive-index.json`
- Create: `/home/photoos/PhotoOS-reconstruction/extracted/<sha256>/...`
- Test: archive path traversal test

**Interfaces:**
- Consumes: archive index from Task 2.
- Produces: safely extracted trees keyed by archive SHA256.

- [ ] **Step 1: Write a failing path-traversal unit test**

Create `/home/photoos/PhotoOS-reconstruction/staging/test_extract_archives.py`:

```python
from pathlib import Path
import io
import tarfile
import tempfile

from extract_archives import safe_tar_members

def test_rejects_parent_traversal():
    with tempfile.TemporaryDirectory() as td:
        tar_path = Path(td) / "bad.tar"
        with tarfile.open(tar_path, "w") as tf:
            info = tarfile.TarInfo("../../escape.txt")
            payload = b"x"
            info.size = len(payload)
            tf.addfile(info, io.BytesIO(payload))

        with tarfile.open(tar_path, "r") as tf:
            members = list(safe_tar_members(tf))

        assert members == []
```

- [ ] **Step 2: Run test and verify it fails because helper does not exist**

Run:

```bash
cd /home/photoos/PhotoOS-reconstruction/staging
python3 -m unittest test_extract_archives.py
```

Expected: FAIL importing `safe_tar_members`.

- [ ] **Step 3: Implement safe extraction**

Create `/home/photoos/PhotoOS-reconstruction/staging/extract_archives.py`:

```python
from __future__ import annotations

from pathlib import Path
import json
import shutil
import tarfile
import zipfile

INDEX = Path("/home/photoos/PhotoOS-reconstruction/indexes/archive-index.json")
OUT = Path("/home/photoos/PhotoOS-reconstruction/extracted")

def is_safe_name(name: str) -> bool:
    p = Path(name)
    return not p.is_absolute() and ".." not in p.parts

def safe_tar_members(tf: tarfile.TarFile):
    for m in tf.getmembers():
        if is_safe_name(m.name):
            yield m

def extract_one(item: dict) -> dict:
    src = Path(item["path"])
    dest = OUT / item["sha256"]
    dest.mkdir(parents=True, exist_ok=True)

    result = dict(item)
    result["extract_dir"] = str(dest)
    result["extract_status"] = "unsupported"

    lower = src.name.lower()

    try:
        if lower.endswith((".tar.gz", ".tgz", ".tar")):
            with tarfile.open(src, "r:*") as tf:
                tf.extractall(dest, members=safe_tar_members(tf))
            result["extract_status"] = "ok"
        elif lower.endswith(".zip"):
            with zipfile.ZipFile(src) as zf:
                for zinfo in zf.infolist():
                    if not is_safe_name(zinfo.filename):
                        continue
                    target = dest / zinfo.filename
                    if zinfo.is_dir():
                        target.mkdir(parents=True, exist_ok=True)
                        continue
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with zf.open(zinfo) as r, target.open("wb") as w:
                        shutil.copyfileobj(r, w)
            result["extract_status"] = "ok"
    except Exception as exc:
        result["extract_status"] = "error"
        result["extract_error"] = f"{type(exc).__name__}: {exc}"

    return result

def main():
    items = json.loads(INDEX.read_text())
    results = [extract_one(item) for item in items]
    INDEX.write_text(json.dumps(results, indent=2, sort_keys=True) + "\n")

if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run traversal test**

Run:

```bash
cd /home/photoos/PhotoOS-reconstruction/staging
python3 -m unittest test_extract_archives.py
```

Expected: PASS.

- [ ] **Step 5: Extract archives**

Run:

```bash
python3 /home/photoos/PhotoOS-reconstruction/staging/extract_archives.py
```

Expected: no traceback.

- [ ] **Step 6: Verify extraction result count**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path

items = json.loads(
    Path("/home/photoos/PhotoOS-reconstruction/indexes/archive-index.json").read_text()
)

ok = [x for x in items if x.get("extract_status") == "ok"]
errors = [x for x in items if x.get("extract_status") == "error"]

print(f"ok={len(ok)} errors={len(errors)}")
for x in errors:
    print("ERROR", x["path"], x.get("extract_error"))

assert len(ok) >= 1
PY
```

Expected: at least one archive extracted successfully.

---

### Task 4: Build Unified Source Candidate Index

**Files:**
- Create: `/home/photoos/PhotoOS-reconstruction/staging/index_sources.py`
- Create: `/home/photoos/PhotoOS-reconstruction/indexes/source-index.json`
- Create: `/home/photoos/PhotoOS-reconstruction/reports/source-index.txt`
- Test: source classification assertions

**Interfaces:**
- Consumes: recovery trees and extracted archives.
- Produces: one de-duplicated source index keyed by SHA256.

- [ ] **Step 1: Write source indexing script**

Create `/home/photoos/PhotoOS-reconstruction/staging/index_sources.py`:

```python
from __future__ import annotations

from collections import defaultdict
from pathlib import Path
import hashlib
import json

ROOTS = [
    Path("/home/photoos/PhotoOS-recovery"),
    Path("/home/photoos/PhotoOS-reconstruction/extracted"),
]

EXTENSIONS = {
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".css", ".html",
    ".toml", ".json", ".sh", ".py", ".service", ".timer",
    ".conf", ".md", ".txt",
}

SKIP_PARTS = {
    ".git", "target", "node_modules",
}

def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

def classify(path: Path) -> str:
    parts = {p.lower() for p in path.parts}
    name = path.name.lower()

    if "dist" in parts or name.endswith(".min.js"):
        return "generated"

    if path.suffix in {".tsx", ".ts", ".css"}:
        return "frontend-source"

    if path.suffix == ".rs":
        return "backend-source"

    if path.suffix in {".service", ".timer", ".conf"}:
        return "runtime-config"

    if path.suffix in {".sh", ".py"}:
        return "script-source"

    return "supporting-text"

groups = defaultdict(lambda: {
    "paths": [],
    "size": None,
    "class": None,
})

for root in ROOTS:
    if not root.exists():
        continue

    for p in root.rglob("*"):
        if not p.is_file():
            continue

        if any(part in SKIP_PARTS for part in p.parts):
            continue

        if p.suffix.lower() not in EXTENSIONS and p.name not in {"Cargo.toml", "Cargo.lock", "package.json"}:
            continue

        h = sha256(p)
        g = groups[h]
        g["paths"].append(str(p))
        g["size"] = p.stat().st_size
        g["class"] = classify(p)

out = []
for h, g in groups.items():
    out.append({
        "sha256": h,
        "size": g["size"],
        "class": g["class"],
        "paths": sorted(set(g["paths"])),
    })

out.sort(key=lambda x: (x["class"], x["paths"][0]))

Path("/home/photoos/PhotoOS-reconstruction/indexes/source-index.json").write_text(
    json.dumps(out, indent=2, sort_keys=True) + "\n"
)

Path("/home/photoos/PhotoOS-reconstruction/reports/source-index.txt").write_text(
    "\n".join(
        f'{x["class"]:16} {x["sha256"]} {x["size"]:>9} {x["paths"][0]}'
        for x in out
    ) + "\n"
)

print(f"unique_sources={len(out)}")
```

- [ ] **Step 2: Run source indexer**

Run:

```bash
python3 /home/photoos/PhotoOS-reconstruction/staging/index_sources.py
```

Expected: `unique_sources=N` with `N > 0`.

- [ ] **Step 3: Verify source classes**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path

items = json.loads(
    Path("/home/photoos/PhotoOS-reconstruction/indexes/source-index.json").read_text()
)

classes = {x["class"] for x in items}
assert "backend-source" in classes
assert "frontend-source" in classes

print("PASS classes:", ", ".join(sorted(classes)))
PY
```

Expected: backend and frontend source classes both present.

---

### Task 5: Detect Exact Missing Modern Frontend Sources

**Files:**
- Create: `/home/photoos/PhotoOS-reconstruction/staging/find_frontend_candidates.py`
- Create: `/home/photoos/PhotoOS-reconstruction/reports/frontend-candidates.json`
- Test: candidate report schema

**Interfaces:**
- Consumes: source index.
- Produces: candidates for modern live pages and shared API modules.

- [ ] **Step 1: Define required frontend targets**

Create `/home/photoos/PhotoOS-reconstruction/staging/find_frontend_candidates.py`:

```python
from pathlib import Path
import json

INDEX = Path("/home/photoos/PhotoOS-reconstruction/indexes/source-index.json")

TARGETS = [
    "App.tsx",
    "DashboardLayout.tsx",
    "HealthPage.tsx",
    "DevicesPage.tsx",
    "AlbumsPage.tsx",
    "AlbumDetailPage.tsx",
    "PhotosPage.tsx",
    "StoragePage.tsx",
    "RaidManagerPage.tsx",
    "BackupManagerPage.tsx",
    "PcBackupsPage.tsx",
    "TvMediaPage.tsx",
    "MirrorPage.tsx",
    "MirrorCard.tsx",
    "NotificationsPage.tsx",
    "LogsPage.tsx",
    "DockerManagerPage.tsx",
    "CloudSyncPage.tsx",
    "SettingsPage.tsx",
    "SetupPreferencesPage.tsx",
    "AutomationPage.tsx",
    "ControlPage.tsx",
    "SystemUpdatesPage.tsx",
    "LoginPage.tsx",
    "albums.ts",
    "photos.ts",
    "client.ts",
]

items = json.loads(INDEX.read_text())
result = {}

for target in TARGETS:
    matches = []
    for item in items:
        for path in item["paths"]:
            if Path(path).name == target:
                matches.append({
                    "sha256": item["sha256"],
                    "size": item["size"],
                    "class": item["class"],
                    "path": path,
                })
    result[target] = sorted(matches, key=lambda x: (x["size"], x["path"]), reverse=True)

Path("/home/photoos/PhotoOS-reconstruction/reports/frontend-candidates.json").write_text(
    json.dumps(result, indent=2, sort_keys=True) + "\n"
)

for target in TARGETS:
    print(f"{target}: {len(result[target])}")
```

- [ ] **Step 2: Run frontend candidate discovery**

Run:

```bash
python3 /home/photoos/PhotoOS-reconstruction/staging/find_frontend_candidates.py
```

Expected: one output line per target.

- [ ] **Step 3: Validate report**

Run:

```bash
python3 - <<'PY'
from pathlib import Path
import json

p = Path("/home/photoos/PhotoOS-reconstruction/reports/frontend-candidates.json")
data = json.loads(p.read_text())

assert "App.tsx" in data
assert "NotificationsPage.tsx" in data
assert "albums.ts" in data
assert isinstance(data["App.tsx"], list)

print("PASS frontend candidate report")
PY
```

Expected: PASS.

---

### Task 6: Detect Backend Stub and Security-Critical Candidates

**Files:**
- Create: `/home/photoos/PhotoOS-reconstruction/staging/find_backend_candidates.py`
- Create: `/home/photoos/PhotoOS-reconstruction/reports/backend-candidates.json`
- Test: stub scanner assertions

**Interfaces:**
- Consumes: source index.
- Produces: backend candidate list with stub/security markers.

- [ ] **Step 1: Write backend candidate classifier**

Create `/home/photoos/PhotoOS-reconstruction/staging/find_backend_candidates.py`:

```python
from pathlib import Path
import hashlib
import json
import re

ROOTS = [
    Path("/home/photoos/PhotoOS-recovery"),
    Path("/home/photoos/PhotoOS-reconstruction/extracted"),
]

TARGETS = {
    "app.rs",
    "jwt.rs",
    "mobile.rs",
    "cloud_accounts.rs",
    "logs.rs",
    "performance_alerts.rs",
    "pc_backups.rs",
    "notifications.rs",
    "photos.rs",
    "albums.rs",
    "user.rs",
    "cloud_sync.rs",
    "cloud_auth.rs",
    "docker.rs",
    "updates.rs",
    "config.rs",
}

MARKERS = {
    "recovery_incomplete": re.compile(r"recovery_incomplete", re.I),
    "pc_backup_unavailable": re.compile(r"pc_backup_recovery_unavailable", re.I),
    "hardcoded_jwt": re.compile(r"photoos-super-secret-key"),
    "todo": re.compile(r"\bTODO\b"),
    "fixme": re.compile(r"\bFIXME\b"),
}

def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()

result = {target: [] for target in TARGETS}

for root in ROOTS:
    if not root.exists():
        continue

    for p in root.rglob("*.rs"):
        if p.name not in TARGETS:
            continue
        if any(part in {".git", "target"} for part in p.parts):
            continue

        text = p.read_text(errors="replace")
        result[p.name].append({
            "path": str(p),
            "sha256": sha256(p),
            "size": p.stat().st_size,
            "markers": {
                key: bool(pattern.search(text))
                for key, pattern in MARKERS.items()
            },
        })

for entries in result.values():
    entries.sort(
        key=lambda x: (
            sum(x["markers"].values()),
            -x["size"],
            x["path"],
        )
    )

Path("/home/photoos/PhotoOS-reconstruction/reports/backend-candidates.json").write_text(
    json.dumps(result, indent=2, sort_keys=True) + "\n"
)

for name in sorted(result):
    clean = [
        x for x in result[name]
        if not any(x["markers"].values())
    ]
    print(f"{name}: total={len(result[name])} marker_free={len(clean)}")
```

- [ ] **Step 2: Run backend candidate scan**

Run:

```bash
python3 /home/photoos/PhotoOS-reconstruction/staging/find_backend_candidates.py
```

Expected: target summary lines.

- [ ] **Step 3: Confirm JWT risk is visible to the report**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path

data = json.loads(
    Path("/home/photoos/PhotoOS-reconstruction/reports/backend-candidates.json").read_text()
)

jwt = data.get("jwt.rs", [])
assert jwt, "jwt.rs candidate missing"
assert any(x["markers"]["hardcoded_jwt"] for x in jwt), "expected recovered JWT risk not detected"

print("PASS JWT risk classified")
PY
```

Expected: PASS.

---

### Task 7: Select the Baseline Source Family

**Files:**
- Create: `/home/photoos/PhotoOS-reconstruction/reports/baseline-selection.json`
- Create: `/home/photoos/PhotoOS-reconstruction/reports/baseline-selection.md`
- Test: baseline completeness checks

**Interfaces:**
- Consumes: source index, frontend candidates, backend candidates.
- Produces: an explicit selected base tree and file overrides for canonical assembly.

- [ ] **Step 1: Generate family score report**

Run:

```bash
python3 - <<'PY'
from pathlib import Path
import json

roots = [
    Path("/home/photoos/PhotoOS-recovery/reimplementation-v15-20260911-172343"),
    Path("/home/photoos/PhotoOS-recovery/source-recovered-20260910-010351"),
    Path("/home/photoos/PhotoOS-recovery/git-main-20260910-004803"),
]

rows = []

for root in roots:
    if not root.exists():
        continue

    frontend = list((root / "client" / "src").rglob("*.tsx")) if (root / "client" / "src").exists() else []
    rust = list((root / "server" / "src").rglob("*.rs")) if (root / "server" / "src").exists() else []

    rows.append({
        "root": str(root),
        "cargo_toml": (root / "Cargo.toml").exists(),
        "package_json": (root / "client" / "package.json").exists(),
        "frontend_tsx_count": len(frontend),
        "backend_rs_count": len(rust),
        "has_client_src": (root / "client" / "src").exists(),
        "has_server_src": (root / "server" / "src").exists(),
    })

Path("/home/photoos/PhotoOS-reconstruction/reports/baseline-selection.json").write_text(
    json.dumps(rows, indent=2, sort_keys=True) + "\n"
)

for x in rows:
    print(x)
PY
```

- [ ] **Step 2: Select `reimplementation-v15` as provisional baseline only if minimum structure is present**

Run:

```bash
python3 - <<'PY'
from pathlib import Path
import json

p = Path("/home/photoos/PhotoOS-reconstruction/reports/baseline-selection.json")
rows = json.loads(p.read_text())

preferred = next(
    (x for x in rows if "reimplementation-v15" in x["root"]),
    None,
)

assert preferred is not None
assert preferred["cargo_toml"]
assert preferred["has_client_src"]
assert preferred["has_server_src"]

Path("/home/photoos/PhotoOS-reconstruction/reports/baseline-selection.md").write_text(
    "# Baseline Selection\n\n"
    f"Selected provisional baseline: `{preferred['root']}`\n\n"
    "Reason: it is the most structurally complete recovered workspace and "
    "already passes backend `cargo check`; it is not yet considered final "
    "until per-file provenance and gap analysis complete.\n"
)

print("PASS provisional baseline selected")
PY
```

Expected: PASS.

---

### Task 8: Create Canonical Git Workspace

**Files:**
- Create: `/home/photoos/PhotoOS-final-source/**`
- Create: `/home/photoos/PhotoOS-final-source/.gitignore`
- Create: `/home/photoos/PhotoOS-final-source/docs/reconstruction/source-selection.md`
- Test: Git clean baseline and source structure checks

**Interfaces:**
- Consumes: provisional baseline from Task 7.
- Produces: isolated canonical repository.

- [ ] **Step 1: Assert canonical directory does not already contain unreviewed work**

Run:

```bash
if [ -e /home/photoos/PhotoOS-final-source ]; then
    echo "STOP: /home/photoos/PhotoOS-final-source already exists"
    exit 1
fi
```

Expected: no output and exit code `0`.

- [ ] **Step 2: Copy provisional baseline without build artifacts**

Run:

```bash
mkdir -p /home/photoos/PhotoOS-final-source

rsync -a \
  --exclude='.git/' \
  --exclude='target/' \
  --exclude='node_modules/' \
  --exclude='dist/' \
  /home/photoos/PhotoOS-recovery/reimplementation-v15-20260911-172343/ \
  /home/photoos/PhotoOS-final-source/
```

Expected: source files copied, no `target`, `node_modules`, or `dist`.

- [ ] **Step 3: Create canonical project support directories**

Run:

```bash
mkdir -p \
  /home/photoos/PhotoOS-final-source/installer \
  /home/photoos/PhotoOS-final-source/systemd \
  /home/photoos/PhotoOS-final-source/nginx \
  /home/photoos/PhotoOS-final-source/scripts \
  /home/photoos/PhotoOS-final-source/docs/reconstruction \
  /home/photoos/PhotoOS-final-source/docs/release \
  /home/photoos/PhotoOS-final-source/tests \
  /home/photoos/PhotoOS-final-source/reports
```

- [ ] **Step 4: Create `.gitignore`**

Create `/home/photoos/PhotoOS-final-source/.gitignore`:

```gitignore
/target/
/client/node_modules/
/client/dist/
*.log
*.tmp
.env
.env.*
.DS_Store
```

- [ ] **Step 5: Copy design into canonical documentation**

Run:

```bash
cp \
  /home/photoos/PhotoOS-reconstruction/reports/baseline-selection.md \
  /home/photoos/PhotoOS-final-source/docs/reconstruction/source-selection.md
```

Expected: source-selection doc exists.

- [ ] **Step 6: Initialize Git repository**

Run:

```bash
cd /home/photoos/PhotoOS-final-source
git init
git config user.name "PhotoOS Final Reconstruction"
git config user.email "photoos@localhost"
git add .
git commit -m "chore: establish recovered source baseline"
```

Expected: first commit succeeds.

- [ ] **Step 7: Confirm live release was not touched**

Run:

```bash
CURRENT="$(readlink -f /opt/photoos/current)"
test "$CURRENT" != "/home/photoos/PhotoOS-final-source"
systemctl is-active --quiet photoos.service
```

Expected: exit code `0`.

---

### Task 9: Import Runtime and Installer Source-of-Truth Files

**Files:**
- Create: `installer/install-to-disk.sh`
- Create: `systemd/*.service`
- Create: `systemd/*.timer`
- Create: `nginx/photoos-gateway`
- Create: `scripts/photoos-notification-aggregator.sh`
- Create: `scripts/photoos-disk-health.sh`
- Modify: `docs/reconstruction/provenance.json`
- Test: syntax and hash checks

**Interfaces:**
- Consumes: repaired live runtime files from Repairs 1–4.
- Produces: source-controlled copies of runtime fixes.

- [ ] **Step 1: Copy repaired installer and gateway files into canonical source**

Run:

```bash
cp -a \
  /opt/photoos-system-installer/install-to-disk.sh \
  /home/photoos/PhotoOS-final-source/installer/install-to-disk.sh

cp -a \
  /etc/nginx/sites-available/photoos-gateway \
  /home/photoos/PhotoOS-final-source/nginx/photoos-gateway

cp -a \
  /usr/local/sbin/photoos-notification-aggregator.sh \
  /home/photoos/PhotoOS-final-source/scripts/photoos-notification-aggregator.sh

cp -a \
  /usr/local/sbin/photoos-disk-health.sh \
  /home/photoos/PhotoOS-final-source/scripts/photoos-disk-health.sh
```

- [ ] **Step 2: Copy all PhotoOS systemd source units**

Run:

```bash
find /etc/systemd/system -maxdepth 1 -type f \
  \( -name 'photoos*.service' -o -name 'photoos*.timer' \) \
  -exec cp -a {} /home/photoos/PhotoOS-final-source/systemd/ \;
```

Expected: storage, setup, notification, disk-health, update, and main service units present.

- [ ] **Step 3: Run syntax checks**

Run:

```bash
bash -n /home/photoos/PhotoOS-final-source/installer/install-to-disk.sh
bash -n /home/photoos/PhotoOS-final-source/scripts/photoos-notification-aggregator.sh
bash -n /home/photoos/PhotoOS-final-source/scripts/photoos-disk-health.sh
```

Expected: exit code `0`.

- [ ] **Step 4: Validate copied Nginx configuration without activating it**

Run:

```bash
grep -q 'PHOTOOS FINAL AUTH GUARD BEGIN' \
  /home/photoos/PhotoOS-final-source/nginx/photoos-gateway

grep -q 'PHOTOOS FINAL AUTH GUARD EXTENSION BEGIN' \
  /home/photoos/PhotoOS-final-source/nginx/photoos-gateway
```

Expected: both repair guards present.

- [ ] **Step 5: Commit runtime source imports**

Run:

```bash
cd /home/photoos/PhotoOS-final-source
git add installer systemd nginx scripts
git commit -m "chore: import repaired runtime and installer sources"
```

Expected: commit succeeds.

---

### Task 10: Generate Per-File Provenance Manifest

**Files:**
- Create: `/home/photoos/PhotoOS-final-source/docs/reconstruction/provenance.json`
- Create: `/home/photoos/PhotoOS-final-source/docs/reconstruction/provenance.md`
- Test: every tracked source file has provenance

**Interfaces:**
- Consumes: canonical Git tree and source candidate index.
- Produces: provenance record for every canonical source file.

- [ ] **Step 1: Write provenance generator**

Create `/home/photoos/PhotoOS-reconstruction/staging/generate_provenance.py`:

```python
from pathlib import Path
import hashlib
import json
import subprocess

CANON = Path("/home/photoos/PhotoOS-final-source")
INDEX = Path("/home/photoos/PhotoOS-reconstruction/indexes/source-index.json")

source_index = json.loads(INDEX.read_text())
by_hash = {x["sha256"]: x["paths"] for x in source_index}

def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

tracked = subprocess.check_output(
    ["git", "-C", str(CANON), "ls-files"],
    text=True,
).splitlines()

records = []

for rel in tracked:
    p = CANON / rel
    if not p.is_file():
        continue

    h = sha256(p)
    origins = by_hash.get(h, [])

    if origins:
        provenance_type = "recovered-exact"
    elif rel.startswith(("installer/", "systemd/", "nginx/", "scripts/")):
        provenance_type = "live-runtime-import"
    else:
        provenance_type = "canonical-baseline-unmatched"

    records.append({
        "path": rel,
        "sha256": h,
        "provenance_type": provenance_type,
        "origins": origins,
    })

out_json = CANON / "docs/reconstruction/provenance.json"
out_json.write_text(json.dumps(records, indent=2, sort_keys=True) + "\n")

lines = [
    "# Canonical Source Provenance",
    "",
    "| Canonical path | Type | SHA256 | Origins |",
    "|---|---|---|---|",
]

for r in records:
    origins = "<br>".join(r["origins"]) if r["origins"] else "none"
    lines.append(
        f'| `{r["path"]}` | {r["provenance_type"]} | `{r["sha256"]}` | {origins} |'
    )

(CANON / "docs/reconstruction/provenance.md").write_text(
    "\n".join(lines) + "\n"
)
```

- [ ] **Step 2: Run provenance generator**

Run:

```bash
python3 /home/photoos/PhotoOS-reconstruction/staging/generate_provenance.py
```

Expected: both provenance files created.

- [ ] **Step 3: Verify every Git-tracked file except provenance outputs is represented**

Run:

```bash
python3 - <<'PY'
from pathlib import Path
import json
import subprocess

root = Path("/home/photoos/PhotoOS-final-source")
tracked = set(subprocess.check_output(
    ["git", "-C", str(root), "ls-files"],
    text=True,
).splitlines())

records = json.loads(
    (root / "docs/reconstruction/provenance.json").read_text()
)
recorded = {x["path"] for x in records}

missing = sorted(
    p for p in tracked
    if p not in recorded and not p.startswith("docs/reconstruction/provenance.")
)

assert not missing, missing
print("PASS provenance coverage")
PY
```

Expected: PASS.

- [ ] **Step 4: Commit provenance**

Run:

```bash
cd /home/photoos/PhotoOS-final-source
git add docs/reconstruction/provenance.json docs/reconstruction/provenance.md
git commit -m "docs: record canonical source provenance"
```

Expected: commit succeeds.

---

### Task 11: Produce Reconstruction Gap Report

**Files:**
- Create: `/home/photoos/PhotoOS-final-source/docs/reconstruction/gaps.md`
- Test: gap report includes all known blockers

**Interfaces:**
- Consumes: frontend/backend candidate reports and canonical tree.
- Produces: exact work list for Phase 2 and Phase 3.

- [ ] **Step 1: Generate gap report from known source state**

Create `/home/photoos/PhotoOS-final-source/docs/reconstruction/gaps.md` with:

```markdown
# PhotoOS Reconstruction Gaps

## Backend — must be resolved in Phase 2

- `server/src/auth/jwt.rs`
  - recovered copies contain a hard-coded JWT secret;
  - canonical implementation must load/generate a runtime secret.

- `server/src/app.rs`
  - recovered route-auth coverage is incomplete;
  - users/photos/albums/setup preferences/cloud account metadata must be backend protected.

- `mobile.rs`
  - recovery stub detected.

- `cloud_accounts.rs`
  - recovery stub detected.

- `logs.rs`
  - recovery stub detected.

- `performance_alerts.rs`
  - recovery stub/incomplete implementation detected.

- PC Backup operations
  - `pc_backup_recovery_unavailable` paths must be replaced or intentionally removed from the final UI/API contract.

## Frontend — must be resolved in Phase 3

The recovered frontend is not equivalent to the live product.

Known missing or incomplete areas:

- modern `App.tsx` route map;
- complete shared API client;
- `api/photos.ts`;
- `api/albums.ts`;
- Notifications page;
- Logs page;
- Docker Manager page;
- Cloud Sync page;
- Settings page;
- Setup Preferences page;
- PC Backup page;
- TV Media page;
- Mirror page;
- Devices/Health pages where exact source was not recovered;
- current mobile sidebar behavior;
- current Photos responsive layout.

## Runtime/Installer — imported but still needs Phase 4 finalization

- one canonical `1.2.1` release identity;
- installer metadata cleanup;
- final clean-install test;
- final ISO build manifest;
- final ISO SHA256.

## Media authorization

Nginx currently protects sensitive API metadata/listing endpoints.

Photo thumbnail/file delivery remains compatible with the existing frontend and requires a canonical frontend/backend media-auth design before final release.
```

- [ ] **Step 2: Confirm prohibited recovery markers are discoverable in canonical production code**

Run:

```bash
cd /home/photoos/PhotoOS-final-source

grep -RInE \
  'recovery_incomplete|pc_backup_recovery_unavailable|photoos-super-secret-key' \
  server client \
  > reports/phase1-known-risk-markers.txt || true

cat reports/phase1-known-risk-markers.txt
```

Expected: markers may exist now; every occurrence is accounted for by `gaps.md`.

- [ ] **Step 3: Commit gap report**

Run:

```bash
git add docs/reconstruction/gaps.md reports/phase1-known-risk-markers.txt
git commit -m "docs: record reconstruction gaps"
```

Expected: commit succeeds.

---

### Task 12: Baseline Build Verification

**Files:**
- Create: `/home/photoos/PhotoOS-final-source/reports/phase1-cargo-check.txt`
- Create: `/home/photoos/PhotoOS-final-source/reports/phase1-frontend-check.txt`
- Test: backend compile baseline; frontend failure captured precisely

**Interfaces:**
- Consumes: canonical source.
- Produces: reproducibility baseline for Phase 2/3.

- [ ] **Step 1: Run backend Cargo check**

Run:

```bash
cd /home/photoos/PhotoOS-final-source

export PATH="/home/photoos/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="/home/photoos/PhotoOS-reconstruction/staging/cargo-target"

cargo check --workspace --offline \
  2>&1 | tee reports/phase1-cargo-check.txt
```

Expected: exit code `0` based on the verified reimplementation baseline.

- [ ] **Step 2: Record frontend package state**

Run:

```bash
cd /home/photoos/PhotoOS-final-source/client

{
  echo "node=$(node --version 2>/dev/null || true)"
  echo "npm=$(npm --version 2>/dev/null || true)"
  echo
  echo "package.json:"
  cat package.json
  echo
  echo "missing imports:"
  python3 - <<'PY'
from pathlib import Path
import re

root = Path("src")
patterns = [
    re.compile(r'from\s+["\'](\.[^"\']+)["\']'),
    re.compile(r'import\s+["\'](\.[^"\']+)["\']'),
]

missing = set()

for f in list(root.rglob("*.ts")) + list(root.rglob("*.tsx")):
    text = f.read_text(errors="ignore")

    for pat in patterns:
        for match in pat.finditer(text):
            rel = match.group(1)
            p = f.parent / rel
            candidates = [
                p,
                p.with_suffix(".ts"),
                p.with_suffix(".tsx"),
                p.with_suffix(".js"),
                p.with_suffix(".jsx"),
                p / "index.ts",
                p / "index.tsx",
                p / "index.js",
                p / "index.jsx",
            ]

            if not any(x.exists() for x in candidates):
                missing.add((str(f), rel))

for f, rel in sorted(missing):
    print(f"MISSING {f} -> {rel}")

print(f"MISSING_IMPORT_COUNT={len(missing)}")
PY
} | tee /home/photoos/PhotoOS-final-source/reports/phase1-frontend-check.txt
```

Expected: known missing imports are captured, not hidden.

- [ ] **Step 3: Commit verification reports**

Run:

```bash
cd /home/photoos/PhotoOS-final-source
git add reports/phase1-cargo-check.txt reports/phase1-frontend-check.txt
git commit -m "test: record reconstruction baseline"
```

Expected: commit succeeds.

---

### Task 13: Phase 1 Acceptance Gate

**Files:**
- Create: `/home/photoos/PhotoOS-final-source/reports/phase1-acceptance.txt`
- Test: all Phase 1 invariants

**Interfaces:**
- Consumes: everything produced in Tasks 1–12.
- Produces: PASS/FAIL gate for starting backend reconstruction.

- [ ] **Step 1: Write acceptance script**

Create `/home/photoos/PhotoOS-reconstruction/staging/verify_phase1.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

CANON="/home/photoos/PhotoOS-final-source"
RECON="/home/photoos/PhotoOS-reconstruction"

test -d "$CANON/.git"
test -s "$CANON/docs/reconstruction/provenance.json"
test -s "$CANON/docs/reconstruction/provenance.md"
test -s "$CANON/docs/reconstruction/gaps.md"
test -s "$RECON/indexes/source-index.json"
test -s "$RECON/indexes/archive-index.json"

CURRENT="$(readlink -f /opt/photoos/current)"
test "$CURRENT" != "$CANON"

systemctl is-active --quiet photoos.service
systemctl is-active --quiet nginx

LISTEN="$(ss -lnt | awk '$4 ~ /:8081$/ {print $4}')"
echo "$LISTEN" | grep -q '127.0.0.1:8081'
! echo "$LISTEN" | grep -Eq '0\.0\.0\.0:8081$|^\*:8081$|^\[::\]:8081$'

cd "$CANON"
git status --short

echo "PHASE1_ACCEPTANCE=PASS"
```

- [ ] **Step 2: Run acceptance script**

Run:

```bash
chmod 700 /home/photoos/PhotoOS-reconstruction/staging/verify_phase1.sh

/home/photoos/PhotoOS-reconstruction/staging/verify_phase1.sh \
  | tee /home/photoos/PhotoOS-final-source/reports/phase1-acceptance.txt
```

Expected: final line `PHASE1_ACCEPTANCE=PASS`.

- [ ] **Step 3: Commit acceptance result**

Run:

```bash
cd /home/photoos/PhotoOS-final-source
git add reports/phase1-acceptance.txt
git commit -m "chore: close source recovery phase"
```

Expected: commit succeeds.

- [ ] **Step 4: Record final Phase 1 commit**

Run:

```bash
cd /home/photoos/PhotoOS-final-source
git log -1 --oneline
git status --short
```

Expected:
- latest commit is `chore: close source recovery phase`;
- working tree is clean.

---

## Phase 1 Completion Criteria

Phase 1 is complete only when all of the following are true:

- `/home/photoos/PhotoOS-final-source` exists and is a Git repository.
- The active `/opt/photoos/current` release has not been replaced.
- Nested recovery archives have been indexed and safely extracted.
- All source candidates have SHA256-based provenance.
- The most complete recovered baseline has been imported.
- Repaired live installer/runtime files are source controlled.
- Backend baseline `cargo check --workspace --offline` passes.
- Frontend gaps are explicitly reported rather than hidden.
- Security/recovery stubs are explicitly listed in `gaps.md`.
- `PHASE1_ACCEPTANCE=PASS`.
- The canonical Git working tree is clean.

At that point Phase 2 can begin: backend completion, backend-native auth protection, JWT secret management, and API parity.
