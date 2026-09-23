# PhotoOS Update Agent v4 Evidence

Date: 2026-09-18
Branch: `update-agent-v4-20260918`
Base Wave 1 source: `4315ece5e4c2c7e25a9474bc3d083f0579784923`

## TDD evidence

The cross-layer contract test was added before the final token parser hardening. The first run failed only because the Agent accepted a non-canonical short token:

```text
test_token_file_must_contain_exactly_64_lowercase_hex_characters ... FAIL
AssertionError: RuntimeError not raised
TASK6_RED_EXIT=1
```

Minimal GREEN change: `_read_token()` now accepts only exactly 64 lowercase hexadecimal characters, matching installer provisioning and the v4 design.

## Update Agent tests

Command:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s update-agent/tests -p 'test_*.py' -v
```

Result:

```text
Ran 26 tests in 6.623s
OK
```

Coverage includes strict `.popkg` validation, path traversal/link/hash rejection, loopback Agent HTTP contracts, mutating-token enforcement, package upload/verify/list flow, helper Unix socket protocol, helper re-verification, atomic release switching, health-failure recovery, rollback validation, and the frontend-facing Update Center JSON shapes.

## Source/security contract

Command:

```bash
python3 scripts/update-agent-contract-check.py
```

Result:

```text
UPDATE_AGENT_SOURCE_CONTRACT=PASS
```

The contract locks these architecture properties:

- Agent source is canonical repository source, not `/opt/photoos/update-agent` build-host state.
- Network Agent runs as `photoos:photoos`, binds loopback, and uses `/etc/photoos/update-agent.env`.
- Root helper is a separate Unix-socket service and does not use shell command execution.
- Installer token provisioning uses `secrets.token_hex(32)`, `root:photoos`, mode `0640`.
- Both Agent and helper service units are shipped in installer payload.
- Legacy `/var/lib/photoos/runtime/token` is rejected.

## Frontend compatibility contracts

Commands were run from `client/` as required by the scripts:

```bash
for script in scripts/*.mjs; do node "$script"; done
```

Results:

```text
CONTROL_PAGE_CONTRACT=PASS
FRONTEND_CONTRACT=PASS routes=13
HEALTH_PAGE_CONTRACT=PASS
LOGIN_SESSION_FRONTEND_CONTRACT=PASS
LOGS_PAGE_CONTRACT=PASS
NOT_FOUND_CONTRACT=PASS
POST_AUDIT_CONTRACT=PASS
REMOVED_FEATURES_FRONTEND_CONTRACT=PASS
SYSTEM_UPDATES_CONTRACT=PASS
WAVE1_UI_CONTRACT=PASS
```

Removed Mirror, Docker Manager, Cloud Sync, and Automation UI/API assumptions remain absent from the frontend contracts.

## Syntax gates

Commands:

```bash
PYTHONPYCACHEPREFIX=/tmp/photoos-v4-pycache \
  python3 -m py_compile \
  update-agent/agent_core.py \
  update-agent/update_agent.py \
  update-agent/update_helper.py

bash -n \
  installer/build-installer.sh \
  installer/install.sh \
  installer/install-to-disk.sh

git diff --check
```

Results:

```text
PYTHON_COMPILE=PASS
INSTALLER_BASH_SYNTAX=PASS
GIT_DIFF_CHECK=PASS
```

## Reproducible installer payload smoke

A synthetic release tree was built with `PHOTOOS_SOURCE_RELEASE` pointing at an isolated temporary directory. The generated self-extracting installer was inspected without executing its privileged install path.

Required archive members were present:

```text
payload/update-agent/update_agent.py
payload/update-agent/update_helper.py
payload/update-agent/agent_core.py
payload/systemd/photoos-update-agent.service
payload/systemd/photoos-update-helper.service
payload/release/server
payload/release/client/dist/index.html
```

`payload/update-agent/tests/` was absent.

Result:

```text
SYNTHETIC_INSTALLER_PAYLOAD_SMOKE=PASS
```

## Environment limitation / required target verification

The local artifact workspace does not contain the Rust toolchain or the real Debian systemd/runtime environment. Therefore final target verification is intentionally deferred to the PhotoOS server handoff script. That gate must rerun:

- `cargo check`
- full server tests
- release server build
- Python tests under target Python 3.11
- Agent/helper isolated smoke using temporary roots and a non-production port/socket
- systemd unit installation/activation only after isolated gates pass

No live `/opt/photoos/current`, production database, production Update Agent service, or data disk was changed during this v4 source-development evidence run.
