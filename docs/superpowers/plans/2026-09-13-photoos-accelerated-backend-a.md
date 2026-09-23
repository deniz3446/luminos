# PhotoOS Accelerated Backend Batch A — Mobile + Devices + Performance Alerts

**Goal:** Collapse the next three backend work items into one isolated, fail-closed batch: reconstruct the complete Mobile server API, restore Devices-page parity, and restore the Performance Alert Engine.

**Approved workflow:** Accelerated mode. One branch, one user command, one result archive. No live deploy. No intermediate user stop unless a hard gate fails.

## Accepted evidence

- Canonical master: `e8bbcb6a6c5684ea63a871cef8e49ed171d1e4d6`
- Mobile contract capture: `PHASE2B4_MOBILE_CONTRACT=PASS`
- Live Devices page:
  - `POST /api/v1/mobile/pairing`
  - response uses `code`
  - pairing UI says 10 minutes
  - online UI window is 2 minutes
  - expected fields: `device_id`, `device_name`, `total_media_count`, `photo_count`, `video_count`, `total_bytes`, `ip_address`, `network_scope`, `last_filename`, `last_source_type`, `first_upload_at`, `last_upload_at`, `online`
- Live binary:
  - `mobile_pairings` and `mobile_devices` schema
  - token-hash lookup
  - pairing consume, heartbeat, unpair, upload counters
  - Mobile upload uses multipart
- Live SQLite:
  - `mobile_pairings(code, expires_at, created_by, used_at)`
  - `mobile_devices(id, user_id, name, token_hash, created_at, last_seen, status, files_uploaded, bytes_uploaded)`
  - extended `photos` metadata/device/source fields
- Performance live binary:
  - `performance-alerts.json`
  - producer name `performance-alert-engine`
  - `PHOTOOS_ALERT_TEST_MODE`
  - metric families CPU, memory, load average, root disk

## Scope

### Mobile

Implement:

- deterministic schema initialization for Mobile tables and missing extended `photos` columns;
- authenticated pairing-code creation using the logged-in PhotoOS JWT;
- one-time pairing-code consumption;
- random device token returned once and only SHA-256 hash stored;
- heartbeat by device token;
- unpair by device token;
- multipart upload authenticated by device token;
- server-side SHA-256 content hash;
- duplicate detection by user/content hash;
- StorageEngine allocation to Photo/Video storage;
- photo DB metadata insert;
- upload byte/file counters;
- source type normalization;
- last transfer IP/network scope capture from reverse-proxy headers.

Pairing code TTL remains 10 minutes.

### Devices parity

Replace the recovered photo-name-only summary with paired-device-driven output:

- include paired devices even before their first upload;
- online = `last_seen` within 120 seconds;
- aggregate photo/video/count/bytes from `photos.device_id`;
- latest file/source/IP/network from the latest uploaded media.

Response remains a direct JSON array because the live Devices page calls `await response.json()` and maps the array directly.

### Performance Alerts

Restore the background producer without self-HTTP calls:

- read CPU from `/proc/stat`;
- memory from `/proc/meminfo`;
- load from `/proc/loadavg`;
- root disk usage from `/usr/bin/df`;
- write an atomic notification producer file at `notification-producers/performance-alerts.json`;
- environment-overridable thresholds;
- conservative defaults;
- 15-second loop;
- unit tests for parsers and severity evaluation.

This avoids a future source-auth regression caused by self-calling protected `/api/v1/system/performance`.

## Security boundaries

- `/api/v1/mobile/pairing` stays middleware-public but requires a valid PhotoOS JWT inside the handler.
- `/api/v1/mobile/pair` stays public because possession of a valid one-time pairing code is the bootstrap credential.
- heartbeat, unpair, upload require a valid device bearer token.
- plaintext device token is never stored.
- upload never trusts a client-provided content hash.
- upload filename is regenerated as a UUID and never uses client path components.
- only image/video multipart files are accepted.
- SQL parameters are bound.
- public API errors do not expose raw SQLx/storage internals.
- no live database writes are made by tests.

## TDD / verification

The script must capture RED evidence against the recovered stubs before replacing them.

Required final gates:

```text
cargo test -p server handlers::mobile::tests --offline
cargo test -p server handlers::devices::tests --offline
cargo test -p server handlers::performance_alerts::tests --offline
cargo test -p server --offline
cargo check --workspace --offline
```

Static gates:

- no `recovery_incomplete` in Mobile;
- no disabled/reimplementation marker in Performance Alerts;
- 600-second pairing TTL;
- 120-second Devices online window;
- no plaintext token DB storage;
- StorageEngine allocation is used by Mobile upload;
- canonical master remains unchanged;
- live release/binary remains unchanged;
- no service restart/deploy.

## Result

Branch:

`phase2b-accelerated-mobile-performance`

Acceptance marker:

`ACCEL_BACKEND_A=PASS`

The branch is not merged in this batch. The next accelerated batch will merge it after review and continue directly into PC Backup / shared ingestion hardening, avoiding a separate merge-only user round trip.
