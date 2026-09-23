# PhotoOS Wave 1 UI Baseline

- Source baseline: `2ca52f3b5de8cf2bddc11e2113f061fb58def81c` (`final-simplification-20260917`).
- Existing frontend contract scripts pass before Wave 1 changes.
- Backend baseline from server run: 36/36 tests passed.
- Browser audit: reconstructed 18082 UI renders but is visually regressed from the professional reference; Logs can appear blank; Control shows unknown API status; Health reports multiple unavailable/error cards; System/Updates uses an older inline red layout.
- Professional reference: `/opt/photoos/releases/1.2.1-final-20260916-093315/client/dist`, used only for visual structure and styling.
- Retired features must remain absent: Mirror, Docker Manager, Cloud Sync, Automation.
- Live `/opt/photoos/current` is not modified by this work package.

## Wave 1 contract intent

The Wave 1 source must expose a stable professional shared shell, use only retained backend endpoints on Health/Logs/Control/System-Updates, render explicit loading/empty/error states, and keep removed features absent.

## Local restoration package result

- Shared shell restored to a navy/cyan aurora design while preserving the simplified navigation.
- Health now probes only retained System, Storage, RAID, Backup, Notifications, and Logs endpoints and renders unavailable states explicitly.
- Logs blank-screen root cause was corrected: `/api/v1/logs/services` returns `{key,title,units}` objects, not strings; the page now models that response and uses summary/health endpoints safely.
- Control no longer calls the removed `backfill-taken-at`, `rescan-media`, `generate-thumbnails`, or `clear-thumbs` APIs. It uses independent read probes and retained quick links instead.
- Update Center now uses only the retained update status/releases/packages/history/upload/verify/install/rollback routes, handles partial agent outages, and no longer exposes stale power-manager routes.
- Static contracts all pass locally: login/session, NotFound, removed features, 13-route frontend contract, Wave 1, Health, Logs, Control, and System/Updates.
- TypeScript syntax transpilation passed for all changed TSX files and `git diff --check` is clean.
- Full `npm run build`, Rust `cargo test -p server`, and real browser smoke remain mandatory on the PhotoOS server because this handoff environment does not contain a complete installed frontend dependency tree or Rust toolchain.
- No live release, database, service, disk, RAID, or ISO was modified by this local work.
