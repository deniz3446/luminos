# PhotoOS Reconstruction Gaps

This document closes Phase 1 source recovery. Every item below is an explicit
reconstruction target for Phase 2 or Phase 3; none is silently treated as complete.

## Backend — Phase 2 blockers

- Recovered JWT source candidates with hard-coded secret: **3**.
  - Canonical backend must remove the recovered `photoos-super-secret-key` value.
  - Final JWT signing material must come from a per-install runtime secret/config source.

- Backend-native authentication coverage is incomplete in the recovered baseline.
  - `users`
  - photo list/index APIs
  - albums
  - setup preferences
  - cloud account metadata
  must be protected in Rust source; current Nginx guards remain defense-in-depth only.

- Recovery-incomplete handler modules detected:
  - `cloud_accounts.rs`
  - `logs.rs`
  - `mobile.rs`

- No `pc_backups.rs` backend source candidate was recovered.
  - PC Backup behavior must be reconstructed from live API/runtime evidence
    or intentionally removed only after product-level review.

## Frontend — Phase 3 blockers

The following modules exist in the live production bundle but have no usable
recovered TS/TSX source candidate:

- `HealthPage.tsx`
- `DevicesPage.tsx`
- `PcBackupsPage.tsx`
- `TvMediaPage.tsx`
- `MirrorPage.tsx`
- `MirrorCard.tsx`
- `NotificationsPage.tsx`
- `LogsPage.tsx`
- `DockerManagerPage.tsx`
- `CloudSyncPage.tsx`
- `SettingsPage.tsx`
- `SetupPreferencesPage.tsx`

Additional frontend source gaps:

- `client/src/api/photos.ts` in the recovered baseline is zero-length/incomplete.
- `client/src/api/albums.ts` is missing.
- The canonical router/layout must be reconciled with the live route set.
- Current mobile sidebar and responsive Photos behavior must be recreated from live bundle/UI evidence.
- No hard-coded LAN IP may remain in final frontend source.

## Runtime / Installer — imported, finalization remains Phase 4

- Repaired installer, Nginx gateway, notification aggregator and systemd units are source-controlled.
- Final release identity still needs unification to product version `1.2.1`.
- Development strings such as `storage-usbtest`, `dev-fix`, and `rc2` must not remain in final release metadata.
- Clean-install validation and final ISO build are not part of Phase 1.

## Media authorization

- Sensitive metadata/listing APIs are currently protected at the public gateway.
- Current thumbnail/file delivery remains compatible with the live frontend.
- Canonical frontend/backend media authorization must be designed and implemented before final release.

## Phase 1 decision

The source recovery phase is complete enough to start reconstruction, but the
canonical source is **not production-final**. Phase 2 must resolve backend
security/stubs before any production replacement is considered.
