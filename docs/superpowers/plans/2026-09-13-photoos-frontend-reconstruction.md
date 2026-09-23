# PhotoOS Frontend Reconstruction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct the missing PhotoOS management frontend into maintainable React/TypeScript source while preserving existing working pages and live-system isolation.

**Architecture:** Use the canonical source as the editable base, the backend router/handler tree as the API contract authority, and the deployed bundle as read-only parity evidence. Introduce one dynamic API client, one responsive dashboard shell, shared feature-page primitives, and focused pages for the missing management areas.

**Tech Stack:** React 19, TypeScript 6, React Router 7, Vite 8, ESLint 10, PhotoOS Rust/Axum backend.

**Spec:** `docs/superpowers/specs/2026-09-13-photoos-frontend-reconstruction-design.md`

## Global Constraints

- Canonical base must be `66fbc95de23b17c82f9fe4b9fa165a657dd36ab3`.
- Work in isolated branch `phase2c-accelerated-frontend` under `/home/photoos/PhotoOS-worktrees/`.
- No live deployment or service restart.
- Preserve existing `API_URL` and `api()` frontend interfaces.
- No active-source hard-coded RFC1918/LAN URL.
- Historical frontend snapshots are moved, not deleted.
- Production build is a hard gate.
- Newly reconstructed files must pass ESLint.

---

### Task 1: Isolated baseline and evidence capture

**Files:**
- Create worktree: `/home/photoos/PhotoOS-worktrees/phase2c-accelerated-frontend`
- Capture: deployed `client/dist/index.html`, asset names, hashes, route/API strings, source-map inventory.

**Interfaces:**
- Consumes: canonical git base and current live release.
- Produces: immutable baseline/evidence directory used by later review.

- [ ] **Step 1: Verify canonical and live identities**

Run:
```bash
git -C /home/photoos/PhotoOS-final-source rev-parse HEAD
readlink -f /opt/photoos/current
sha256sum /opt/photoos/current/server
```
Expected: canonical `66fbc95...`, unchanged live release and server SHA.

- [ ] **Step 2: Create isolated branch/worktree**

Run:
```bash
git -C /home/photoos/PhotoOS-final-source worktree add \
  /home/photoos/PhotoOS-worktrees/phase2c-accelerated-frontend \
  -b phase2c-accelerated-frontend
```
Expected: worktree at canonical base.

- [ ] **Step 3: Run baseline production build**

Run:
```bash
cd /home/photoos/PhotoOS-worktrees/phase2c-accelerated-frontend/client
npm run build
```
Expected: PASS before any source reconstruction.

### Task 2: Dynamic API client and contract test

**Files:**
- Modify: `client/src/api/client.ts`
- Modify: `client/src/api/photos.ts`
- Create: `client/src/api/albums.ts`
- Create: `client/scripts/frontend-contract-check.mjs`

**Interfaces:**
- Produces: `API_URL`, `api(path, init)`, `apiJson<T>(path, init)`, token helpers.

- [ ] **Step 1: Add contract check that rejects fixed LAN URLs and requires the full route set**
- [ ] **Step 2: Run it before reconstruction and record RED**
- [ ] **Step 3: Implement dynamic API URL and compatibility exports**
- [ ] **Step 4: Run contract test after router work and require GREEN**

### Task 3: Responsive application shell and router

**Files:**
- Modify: `client/src/App.tsx`
- Modify: `client/src/layouts/DashboardLayout.tsx`
- Create: `client/src/layouts/DashboardLayout.css`
- Create: `client/src/components/FeatureShell.tsx`
- Create: `client/src/pages/FeaturePages.css`

**Interfaces:**
- Produces: full route map and mobile sidebar behavior.

- [ ] **Step 1: Preserve existing routes and add RAID, Mirror, Backup, PC Backup, Cloud Sync, Notifications, Logs, Docker, Automation, Devices, TV, Health, Settings, and Preferences routes**
- [ ] **Step 2: Add mobile hamburger, Escape-to-close, route-change close, and accessible nav state**
- [ ] **Step 3: Keep login/logout compatible with historical token keys**

### Task 4: Reconstruct observability and device pages

**Files:**
- Create: `HealthPage.tsx`, `DevicesPage.tsx`, `NotificationsPage.tsx`, `LogsPage.tsx`

**Interfaces:**
- Consumes: `apiJson` and backend APIs `/system/health`, `/storage/health`, `/devices`, `/notifications`, `/logs`.
- Produces: read/refresh UI and mobile pairing-code action.

- [ ] **Step 1: Implement typed health cards with independent endpoint failure isolation**
- [ ] **Step 2: Implement device summary table and pairing-code generation**
- [ ] **Step 3: Implement notification counts/list and log filters/list**

### Task 5: Reconstruct backup, cloud, Docker, automation, and media pages

**Files:**
- Create: `PcBackupsPage.tsx`, `MirrorPage.tsx`, `CloudSyncPage.tsx`, `DockerManagerPage.tsx`, `AutomationPage.tsx`, `TvMediaPage.tsx`

**Interfaces:**
- Consumes: management APIs under `/pc-backups`, `/mirror`, `/cloud-sync`, `/docker`, `/automation`, `/photos`.
- Produces: safe management/read-only surfaces; mutating actions are limited to already-established endpoints.

- [ ] **Step 1: Implement PC Backup library and pairing code**
- [ ] **Step 2: Implement Mirror status/sync control with explicit confirmation**
- [ ] **Step 3: Implement Cloud/Docker/Automation status views**
- [ ] **Step 4: Implement TV media gallery using public thumbnail endpoint and authenticated photo listing**

### Task 6: Settings and recovery cleanup

**Files:**
- Create: `SettingsPage.tsx`, `SetupPreferencesPage.tsx`
- Move: historical `client/src/**/*.bak*`, `*.backup*`, `*.bozuk*` to `recovery/frontend-legacy/`.

**Interfaces:**
- Settings page links to operational sections.
- Preferences page reads `/api/v1/setup/preferences` without inventing unsupported mutation semantics.

- [ ] **Step 1: Build settings navigation and read-only preference inspector**
- [ ] **Step 2: Move legacy snapshots preserving relative paths**
- [ ] **Step 3: Confirm active `client/src` contains no legacy snapshot names**

### Task 7: Verification and review package

**Files:**
- Create: `reports/accelerated-frontend-a-acceptance.txt`

- [ ] **Step 1: Run frontend contract check**
- [ ] **Step 2: Run ESLint on reconstructed files**
- [ ] **Step 3: Run full production build**
- [ ] **Step 4: Scan active source for hard-coded private LAN URLs**
- [ ] **Step 5: Verify built assets contain required route strings**
- [ ] **Step 6: Commit branch but do not merge**
- [ ] **Step 7: Re-check live release/binary identity and package evidence for code review**
