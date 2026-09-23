# PhotoOS Final Source Reconstruction Design

**Date:** 2026-09-12  
**Project:** PhotoOS  
**Goal:** Reconstruct a canonical, buildable, reproducible final source tree that matches the currently working PhotoOS product closely enough to become the only source used for the final ISO.

## 1. Why reconstruction is required

The currently running PhotoOS is healthier and more complete than the available recovery source trees.

Confirmed gaps in the recovery source include:
- Missing modern frontend pages and route coverage.
- Broken/missing frontend API modules.
- Backend modules that are recovery stubs or intentionally incomplete.
- An old hard-coded JWT secret in recovered source.
- Installer/source metadata that does not match the active release.
- A running frontend bundle and backend binary whose behavior is not reproducible from the currently recovered source.

Therefore, the recovered tree must not be promoted directly to final source.

## 2. Source-of-truth hierarchy

Reconstruction will use the following priority order:

1. **Exact original source recovered from backups/archives**
   - Preferred whenever a complete, internally consistent version exists.

2. **Current live runtime behavior**
   - HTTP/API behavior.
   - Route contracts.
   - Authentication behavior.
   - Service/unit behavior.
   - Storage/backup/mirror/cloud/update semantics.

3. **Current live frontend bundle**
   - Used as a behavioral and UI reference.
   - Used to recover route names, component behavior, labels, API paths, and responsive behavior.
   - Not treated as ideal source code by itself.

4. **Current live configuration and installer scripts**
   - Used to reconstruct deterministic install/runtime behavior.

5. **Recovered/reimplementation source**
   - Used as a base only where it agrees with the live product and passes parity checks.

No inferred implementation may override an exact recovered source or verified live behavior without an explicit reason documented in the reconstruction report.

## 3. Canonical tree

The new canonical tree will be created as:

`/home/photoos/PhotoOS-final-source`

Initial structure:

```text
PhotoOS-final-source/
  Cargo.toml
  server/
  client/
  installer/
  systemd/
  nginx/
  scripts/
  docs/
    reconstruction/
    release/
  tests/
  reports/
```

The existing live release and recovery trees remain untouched until the new tree passes all gates.

## 4. Phase A — Deep recovery pass

Before recreating missing code, perform one last read-only pass across nested archives and historical recovery artifacts.

Search targets include:
- `.tar.gz`, `.tgz`, `.tar`, `.zip`
- copied source snapshots
- build/release work directories
- old reports containing embedded source
- frontend backup folders
- installer payloads containing source-like files

The scan must:
- never modify the live PhotoOS release;
- extract only to `/tmp` or a dedicated reconstruction workspace;
- hash every candidate;
- de-duplicate exact files;
- classify files as exact source / partial / generated / unknown;
- keep provenance for every accepted file.

If a complete original source file is found, it takes priority over decompiled/recreated code.

## 5. Phase B — Backend reconstruction

Base:
`reimplementation-v15`

Backend modules are classified into three groups:

### Group 1 — usable recovered modules
Keep and verify modules already substantially implemented and matching live behavior, such as:
- photos
- albums
- notifications
- cloud sync
- docker
- updates
- cloud auth
- storage integrations where applicable

### Group 2 — incomplete/stub modules
Reconstruct and test:
- mobile
- cloud accounts
- logs
- performance alerts
- PC backup missing operations
- any other `recovery_incomplete` or `pc_backup_recovery_unavailable` path

### Group 3 — security-critical core
Rework even if recovered:
- JWT secret loading
- auth middleware
- route protection
- user access
- setup preferences
- cloud-account metadata exposure

Rules:
- No hard-coded JWT secret.
- Secrets must be generated/loaded from runtime-owned secure files or configuration.
- Sensitive routes must require authentication in backend code, not only Nginx.
- Nginx guards added during repair remain defense-in-depth, not the only security boundary.

## 6. Phase C — Frontend reconstruction

The current live `dist` is the behavioral reference.

Reconstruction order:
1. App/router structure.
2. Shared API client.
3. Authentication/session flow.
4. Dashboard/layout/navigation.
5. Photos and Albums.
6. Storage / RAID.
7. Backup / PC Backup.
8. Mirror.
9. Cloud Sync.
10. Notifications.
11. Logs.
12. Docker.
13. Automation.
14. Devices / TV.
15. System / Updates / Settings.
16. Setup flows.
17. Responsive/mobile behavior.

For each reconstructed page:
- route must match live product;
- API calls must match verified server endpoints;
- visible labels and state behavior should match live UI;
- loading/empty/error states must exist;
- mobile/desktop layout must match the current working product as closely as practical.

No hard-coded LAN IPs are allowed. API addressing must derive from the browser origin or a controlled deployment configuration.

## 7. Phase D — Installer and runtime integration

Move the currently repaired installer/runtime logic into source-controlled files.

Required installer guarantees:
- fail closed if live media cannot be identified;
- enforce minimum target disk size;
- avoid copying mounted data disks;
- sanitize runtime/database/installer state;
- reset `machine-id`;
- regenerate SSH host keys;
- regenerate update/runtime tokens;
- enable all required services deterministically;
- enable time synchronization;
- verify required units exist before declaring success;
- use one canonical release/version identity.

The final installer must not rely on accidental state inherited from the live build machine.

## 8. Phase E — Version and release identity

Use one canonical product version.

Initial final target:
`1.2.1`

Build-specific release IDs may use:
`1.2.1-final-YYYYMMDD-HHMMSS`

The same version/release identity must appear consistently in:
- backend config
- release manifest
- installer manifest
- ISO metadata
- UI version display
- update package metadata

Development strings such as:
- `usbtest`
- `dev-fix`
- `rc2`

must not remain in final release metadata.

## 9. Verification gates

The new source tree is not allowed to replace the live release until all gates pass.

### Gate 1 — Static/build
Backend:
- `cargo check`
- `cargo test`
- `cargo clippy` reviewed
- no recovery stubs in production paths

Frontend:
- dependencies install/reproduce
- TypeScript build passes
- lint passes or remaining warnings are explicitly accepted
- production build passes
- no broken relative imports

### Gate 2 — Security
Unauthenticated requests to sensitive APIs must return 401/403.

Explicit checks:
- users
- photos listing/index APIs
- albums
- setup preferences
- cloud account metadata
- storage/admin endpoints
- logs
- notifications
- automation
- Docker
- update/admin operations

No hard-coded secrets.

### Gate 3 — API parity
Compare canonical build against live product for:
- status codes
- response schema
- key workflows
- error behavior

Differences must be documented and intentional.

### Gate 4 — UI parity
Test actual browser rendering at:
- 390 px
- 460 px
- tablet
- desktop

Check:
- navigation/sidebar
- photos timeline/year cards
- dialogs
- overflow
- empty/loading/error states
- major management pages
- authenticated transitions

### Gate 5 — clean installation
Install on a clean test machine/disk.

Verify:
- first boot
- setup wizard
- login
- storage detection
- storage assignment
- RAID flows
- backup
- mirror
- cloud sync
- notifications
- logs
- update agent
- reboot persistence

### Gate 6 — final ISO
Only after Gates 1–5 pass:
- build final ISO;
- compute SHA256;
- record source commit/release ID;
- archive build manifest and audit results.

## 10. Safety / rollback policy

Until canonical source passes all gates:
- do not overwrite `/opt/photoos/current`;
- do not delete recovery trees;
- do not delete previous repair backups;
- do not use the new source to build the production ISO;
- perform reconstruction in an isolated workspace;
- every live change must have a backup and a verification step.

## 11. Definition of done

PhotoOS is considered complete only when:

1. One canonical source tree exists.
2. That source builds backend and frontend from scratch.
3. Security fixes exist in source, not only in runtime patches.
4. Installer behavior is reproducible from source.
5. Clean install passes.
6. Real UI audit passes.
7. Final ISO is built from the canonical source.
8. ISO SHA256 and build manifest are recorded.
9. No unresolved ISO blocker remains.

## 12. Next implementation step

After approval of this written design:

1. Run the read-only nested backup/archive recovery pass.
2. Build the provenance inventory.
3. Create `/home/photoos/PhotoOS-final-source` from the best verified sources.
4. Produce a reconstruction gap report.
5. Reconstruct missing backend/frontend pieces in controlled batches.
6. Run the verification gates after each batch.
