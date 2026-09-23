# PhotoOS Frontend Reconstruction Design

**Date:** 2026-09-13
**Canonical base:** `66fbc95de23b17c82f9fe4b9fa165a657dd36ab3`

## Goal

Reconstruct the maintainable PhotoOS web client from the canonical source tree, verified backend API contracts, and read-only evidence from the currently deployed frontend bundle. The result must restore the missing management pages and routes without copying minified production assets back into source.

## Architecture

The canonical React/TypeScript client remains the source of truth. The deployed bundle is used only as read-only parity evidence (route strings, API strings, asset inventory, and source-map availability). Backend route modules and handlers are the authority for API contracts.

A shared dynamic API client uses `VITE_API_URL` when explicitly provided and otherwise uses `window.location.origin`, eliminating fixed LAN addresses. Authentication remains bearer-token based and is compatible with the token keys used by historical PhotoOS clients.

The reconstructed UI uses one responsive `DashboardLayout`, a small set of shared feature-page primitives, and focused pages for Health, Devices, PC Backup, TV Media, Mirror, Notifications, Logs, Docker, Cloud Sync, Automation, Settings, and Setup Preferences. Existing working pages (Photos, Albums, Storage, RAID, Backup, Dashboard, Users, Control, Updates) are retained and routed through the same shell.

## Safety and compatibility

- Work only in an isolated git worktree until review.
- Do not deploy or restart the live PhotoOS service in this batch.
- Capture the current live bundle before source changes.
- Preserve existing API-client exports (`API_URL` and `api`) so current pages continue compiling.
- Move historical `.bak`, `.backup`, and `.bozuk` frontend snapshots out of active `client/src` into `recovery/frontend-legacy/`, preserving relative paths.
- Do not delete recovered evidence.
- Do not introduce hard-coded RFC1918/LAN IP addresses in active frontend source.
- Keep the current backend and live release unchanged.

## Validation

The batch must run a clean baseline production build first, then after reconstruction:

1. production `npm run build` must pass;
2. a contract test must verify the required route set and dynamic API client;
3. ESLint must pass on all newly reconstructed frontend files;
4. global ESLint output is recorded for regression comparison but pre-existing unrelated lint debt is not silently reclassified as a blocker;
5. built assets must contain the expected route strings;
6. active frontend source must have zero hard-coded private LAN URLs;
7. live release path and live server binary SHA256 must remain unchanged.

Actual visual browser validation remains a separate release gate before final deployment/ISO because this batch does not alter the live frontend.
