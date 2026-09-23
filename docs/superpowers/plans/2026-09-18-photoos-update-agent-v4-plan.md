# PhotoOS Update Agent v4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the missing Update Agent as a canonical, testable, unprivileged loopback service with a narrow root helper and reproducible installer packaging.

**Architecture:** A Python stdlib HTTP Agent runs as `photoos` on `127.0.0.1:8091`; a separate root helper accepts only install/rollback requests over a group-restricted Unix socket. Both sides share a strict package verifier, and the helper re-verifies packages before an atomic release switch.

**Tech Stack:** Python 3.11 stdlib (`http.server`, `socketserver`, `tarfile`, `hashlib`, `hmac`, `tomllib`, `unittest`), systemd, Bash installer scripts, existing Rust/React Update Center proxy/UI.

**Spec:** `docs/superpowers/specs/2026-09-18-photoos-update-agent-v4-design.md`

## Global Constraints

- Bind Agent only to `127.0.0.1:8091`.
- Mutating HTTP requests require `X-PhotoOS-Update-Token`.
- Canonical token file is `/etc/photoos/update-agent.env`, owner `root:photoos`, mode `0640`.
- Root helper accepts only install/rollback over `/run/photoos/update-helper.sock`.
- Never execute package-provided scripts or commands.
- Never delete previous releases automatically.
- Installer builds Agent from repository source, never `/opt/photoos/update-agent`.
- No live deployment in this plan.

---

### Task 1: Strict package and path core

**Files:**
- Create: `update-agent/agent_core.py`
- Create: `update-agent/tests/test_agent_core.py`

**Interfaces:**
- Produces: `validate_leaf_name(value, suffix=None) -> str`, `verify_package(path: Path, limits: PackageLimits = DEFAULT_LIMITS) -> dict`, `safe_extract_verified(package, destination, verification) -> None`, `sha256_file(path) -> str`.

- [ ] **Step 1: Write failing tests** for traversal, symlink members, duplicate members, missing critical files, wrong SHA-256, and one valid package.
- [ ] **Step 2: Run** `python3 -m unittest -v update-agent/tests/test_agent_core.py` and confirm RED because `agent_core` does not exist.
- [ ] **Step 3: Implement the minimal verifier** using only regular tar members and exact manifest file hashes.
- [ ] **Step 4: Re-run the focused tests** and confirm GREEN.
- [ ] **Step 5: Commit** `feat(update-agent): add strict package verifier`.

### Task 2: Unprivileged HTTP Agent contract

**Files:**
- Create: `update-agent/update_agent.py`
- Create: `update-agent/tests/test_update_agent.py`

**Interfaces:**
- Consumes: core verifier from Task 1.
- Produces: loopback HTTP endpoints compatible with `server/src/handlers/updates.rs` and `client/src/pages/SystemUpdatesPage.tsx`.

- [ ] **Step 1: Write failing HTTP tests** using a temporary filesystem and ephemeral port for status/releases/packages/history, token rejection, upload bounds, verify, and helper dispatch serialization.
- [ ] **Step 2: Run focused tests** and verify expected RED.
- [ ] **Step 3: Implement minimal Agent** with injected filesystem roots through environment variables for tests, fixed production defaults, constant-time token comparison, bounded upload streaming, verification sidecars, and history records.
- [ ] **Step 4: Re-run focused and core tests**; confirm GREEN.
- [ ] **Step 5: Commit** `feat(update-agent): add loopback update service`.

### Task 3: Privileged helper and atomic release switch

**Files:**
- Create: `update-agent/update_helper.py`
- Create: `update-agent/tests/test_update_helper.py`

**Interfaces:**
- Consumes: `verify_package()` and safe extraction from Task 1.
- Produces: Unix-socket JSON operations `{operation:"install", filename}` and `{operation:"rollback", version}`.

- [ ] **Step 1: Write failing tests** for socket request validation, package re-verification, release destination confinement, atomic symlink switching, and automatic restoration after a simulated health failure.
- [ ] **Step 2: Run focused tests** and verify RED.
- [ ] **Step 3: Implement helper** with injectable restart/health hooks for tests; production hook invokes only `systemctl restart photoos.service` and local HTTP health polling.
- [ ] **Step 4: Run all Update Agent unit tests** and confirm GREEN.
- [ ] **Step 5: Commit** `feat(update-agent): add privileged release helper`.

### Task 4: Hardened systemd units and token provisioning

**Files:**
- Modify: `systemd/photoos-update-agent.service`
- Create: `systemd/photoos-update-helper.service`
- Create: `scripts/update-agent-contract-check.py`
- Modify: `installer/install.sh`
- Modify: `installer/install-to-disk.sh`

**Interfaces:**
- Produces: Agent as `photoos`, helper as root, shared `/etc/photoos/update-agent.env`, and enabled installed-target services.

- [ ] **Step 1: Write a failing source contract** asserting service users, loopback agent command, helper socket service, token path/permissions, and absence of legacy `/var/lib/photoos/runtime/token` provisioning.
- [ ] **Step 2: Run** `python3 scripts/update-agent-contract-check.py` and verify RED against the legacy root Agent unit/token path.
- [ ] **Step 3: Update units/installers minimally** to satisfy the contract; generate token only when missing/invalid and never echo its value.
- [ ] **Step 4: Run contract + `bash -n` on modified installer scripts** and all Python tests.
- [ ] **Step 5: Commit** `fix(installer): provision split update agent safely`.

### Task 5: Reproducible installer payload

**Files:**
- Modify: `installer/build-installer.sh`
- Extend: `scripts/update-agent-contract-check.py`

**Interfaces:**
- Produces: installer payload sourced from `$PROJECT_ROOT/update-agent`, with canonical systemd units included.

- [ ] **Step 1: Extend the contract so legacy `SOURCE_AGENT="/opt/photoos/update-agent"` fails and repository source is required.**
- [ ] **Step 2: Run contract and verify RED.**
- [ ] **Step 3: Change build script** to validate/copy `update_agent.py`, `update_helper.py`, `agent_core.py`, and both service units from repository source.
- [ ] **Step 4: Run contract, `bash -n`, and a synthetic installer-payload smoke build with a fake release tree.**
- [ ] **Step 5: Commit** `fix(installer): package update agent from canonical source`.

### Task 6: Cross-layer Update Center contract and delivery gate

**Files:**
- Create: `update-agent/tests/test_contract_shapes.py`
- Create: `docs/superpowers/reports/2026-09-18-update-agent-v4-evidence.md`

**Interfaces:**
- Validates Agent JSON against current frontend and Rust proxy expectations without changing either layer unless a real mismatch is found.

- [ ] **Step 1: Add failing shape tests** for status/releases/packages/history and mutating response bodies.
- [ ] **Step 2: Run and confirm RED for any missing fields.**
- [ ] **Step 3: Make only compatibility fixes required by the tests.**
- [ ] **Step 4: Run all Update Agent tests, source contract, all existing frontend contract scripts, Python syntax compilation, and installer Bash syntax checks.**
- [ ] **Step 5: Record exact commands/results and commit** `test(update-agent): lock update center v4 contract`.

### Task 7: Package handoff without live deployment

**Files:**
- Generate outside Git: v4 Git bundle, SHA256SUMS, apply-and-isolated-test script, README.

**Interfaces:**
- Consumes: exact source HEAD from Task 6.
- Produces: a fast-forwardable delivery from server base `4315ece5e4c2c7e25a9474bc3d083f0579784923` without changing `/opt/photoos/current` or enabling services.

- [ ] **Step 1: Verify branch ancestry, clean tree, and exact tests.**
- [ ] **Step 2: Generate and verify Git bundle.**
- [ ] **Step 3: Generate a server-side apply/test script that imports the branch, reruns tests/contracts, and performs only an unprivileged temporary Agent HTTP smoke test on a non-production port/socket.**
- [ ] **Step 4: Compute SHA256SUMS and inspect archive contents.**
- [ ] **Step 5: Deliver the archive; do not deploy live services.**
