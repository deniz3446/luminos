# PhotoOS Update Agent v4 Design

## Goal

Restore the missing PhotoOS Update Agent from canonical source without recreating the old root-network-daemon design. The rebuilt subsystem must preserve the existing Update Center HTTP contract while separating network-facing work from privileged release switching.

## Confirmed problem

The canonical source and installed system contain `photoos-update-agent.service`, and the Rust server proxies Update Center requests to `127.0.0.1:8091`, but `update_agent.py` is absent. Historical archives do not contain a recoverable copy. The installer currently expects `/opt/photoos/update-agent/update_agent.py` to pre-exist on the build host, which makes the build non-reproducible.

## Architecture

### Unprivileged Update Agent

`/opt/photoos/update-agent/update_agent.py` runs as `photoos:photoos` and binds only `127.0.0.1:8091`.

It owns these operations:
- `GET /status`
- `GET /releases`
- `GET /packages`
- `GET /history`
- `POST /upload-package`
- `POST /verify-package`
- authenticated dispatch for `POST /install-package`
- authenticated dispatch for `POST /rollback`

GET endpoints are loopback-only and do not require the token because the Rust server's existing `agent_get()` contract does not send one. Every mutating POST requires `X-PhotoOS-Update-Token` and compares it with `PHOTOOS_UPDATE_TOKEN` using constant-time comparison.

### Privileged helper

`/opt/photoos/update-agent/update_helper.py` runs as root in `photoos-update-helper.service`. It has no TCP listener. It accepts a narrow JSON protocol on `/run/photoos/update-helper.sock`, owned `root:photoos` with mode `0660`.

Allowed operations are only:
- install a named verified `.popkg` already under `/var/lib/photoos/update/packages`
- switch to an existing named release under `/opt/photoos/releases`

The helper never runs scripts from a package, never shells out with user-controlled command text, never writes outside the configured PhotoOS release/update roots, and re-verifies the package itself before installation.

### Token

Canonical token storage is `/etc/photoos/update-agent.env`:

```text
PHOTOOS_UPDATE_TOKEN=<64 lowercase hex characters>
```

Installer provisioning creates it with cryptographically secure randomness, owner `root:photoos`, mode `0640`, and never prints the token. Existing non-empty valid token files are preserved.

### Package format

A `.popkg` is a gzip-compressed tar archive containing:

```text
manifest.json
release/
  server
  VERSION
  client/dist/index.html
  ...
```

`manifest.json` format version 1 contains at minimum:

```json
{
  "product": "PhotoOS",
  "format_version": 1,
  "version": "1.2.1",
  "release_id": "1.2.1-example-20260918-120000",
  "channel": "stable",
  "release_notes": "...",
  "files": [
    {"path": "server", "size": 123, "sha256": "..."}
  ]
}
```

Verification rejects:
- absolute paths, `..`, empty path segments, NUL bytes;
- symlinks, hard links, devices, FIFOs, sockets, or non-regular payload members;
- duplicate members;
- unsupported product/format;
- unsafe release/version identifiers;
- missing critical release files;
- file-list mismatch, size mismatch, or SHA-256 mismatch;
- packages above configured compressed/expanded limits.

Verification is performed both by the unprivileged Agent and again by the privileged helper before installation.

### Installation and rollback

Install extracts only verified regular files into a fresh staging directory. It creates a new immutable release directory named by `release_id`, writes files with controlled modes (server executable, ordinary files non-executable), and atomically switches `/opt/photoos/current` using a temporary symlink plus `os.replace()`.

After switching, the helper restarts only `photoos.service` and waits for the configured local health endpoint. If health fails, it restores the previous symlink and restarts the server again. Old releases are never deleted automatically.

Rollback uses the same atomic switch and health/automatic-recovery flow. The release argument must resolve to exactly one direct child of `/opt/photoos/releases`.

### Reproducible installer source

`installer/build-installer.sh` must copy Update Agent files from the repository's `update-agent/` directory, never from `/opt/photoos/update-agent`. Missing canonical agent files fail the build.

Installer logic installs:
- Agent source under `/opt/photoos/update-agent` as root-owned files;
- both systemd units;
- token configuration;
- writable update directories owned by `photoos`;
- helper and agent services enabled for the installed target.

The live Installer4 environment may continue to mask the Update Agent; the installed target must not.

## Compatibility contract

The existing Rust Update Center proxy remains authoritative. Response fields must remain compatible with `SystemUpdatesPage.tsx`:
- status: `product`, `agent_version`, `mode`, `timestamp`, `healthy`, `health`
- releases: `active_release`, `releases`, `timestamp`
- packages: `packages`, `timestamp`; items include `filename`, `size_bytes`, `sha256`, `verified`, `verification`
- history: `history`, `timestamp`; entries include `filename`, `payload`

The server continues to read `/etc/photoos/update-agent.env` and send the token on mutating calls.

## Safety constraints

- No live `/opt/photoos/current` changes during v4 source development.
- No access to data disks during development or preview.
- Agent binds loopback only.
- Root helper has no network-facing command endpoint.
- No package hook execution.
- No destructive cleanup of previous releases.
- No update installation before source tests and isolated contract tests pass.
- Installer4 final ISO remains blocked until browser/runtime and clean-install gates pass.
