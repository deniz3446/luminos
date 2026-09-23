# PhotoOS UI Restoration Wave 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore PhotoOS's professional shared UI shell and make Health, Logs, Control, and System/Updates pages render reliably against the simplified backend without reintroducing Mirror, Docker Manager, Cloud Sync, or Automation.

**Architecture:** Keep the simplified backend and the current 13-route product surface as the authority. Use the old 18083 build only as a visual reference. Rebuild shared visual primitives in source, then repair each Wave 1 page with explicit API contracts, error states, and browser smoke coverage before any live deployment.

**Tech Stack:** React + TypeScript + Vite, Rust + Axum + SQLx, existing PhotoOS shell/CSS, Node contract scripts, Cargo tests, curl/browser smoke tests.

**Spec:** `docs/superpowers/specs/2026-09-18-photoos-ui-restoration-design.md`

## Global Constraints

- Live `/opt/photoos/current` and `photoos.service` are unchanged until the staged release passes all Wave 1 gates.
- Preserve the simplified feature set: Mirror, Docker Manager, Cloud Sync, and Automation stay absent from frontend navigation/routes/APIs and backend routes.
- Preserve existing SQLite data; test against isolated database snapshots.
- Use RED → minimal GREEN → regression verification for each functional change.
- Use the 18083 old build only as a visual reference; never copy its backend assumptions or retired feature routes.
- No live RAID, backup, storage-destructive, or disk operations during UI restoration.
- Browser-visible success is required; HTTP 200 alone is insufficient.

---

### Task 1: Establish Wave 1 audit contracts

**Files:**
- Create: `client/scripts/wave1-ui-contract-check.mjs`
- Create: `docs/superpowers/reports/2026-09-18-wave1-baseline.md`
- Inspect: `client/src/layouts/DashboardLayout.tsx`
- Inspect: `client/src/pages/HealthPage.tsx`
- Inspect: `client/src/pages/LogsPage.tsx`
- Inspect: `client/src/pages/ControlPage.tsx`
- Inspect: `client/src/pages/SystemUpdatesPage.tsx`
- Inspect: `server/src/routes/*.rs`

**Interfaces:**
- Consumes: current 13-route frontend contract and simplified backend route set.
- Produces: a machine-checkable Wave 1 contract listing allowed page routes, forbidden retired labels/routes, critical API endpoints, and required shared-shell selectors.

- [ ] **Step 1: Write the failing Wave 1 contract**

Create a Node contract that asserts:
`DashboardLayout` exposes the shared shell classes; the four Wave 1 pages do not reference retired features; every literal `/api/v1/...` endpoint used by those pages is present in backend route source; and each page exposes a stable page root class.

- [ ] **Step 2: Run it and record RED**

Run:
```bash
cd client
node scripts/wave1-ui-contract-check.mjs
```

Expected: FAIL on missing shared-shell/page-root/API contract details that are documented in the baseline report.

- [ ] **Step 3: Record baseline**

Capture current source head, existing 36/36 backend tests, frontend contract status, 18082 page behavior, and the known visual/runtime findings from the audit video.

- [ ] **Step 4: Commit only the contract/report**

```bash
git add client/scripts/wave1-ui-contract-check.mjs docs/superpowers/reports/2026-09-18-wave1-baseline.md
git commit -m "test(ui): define wave1 restoration contracts"
```

### Task 2: Restore the professional shared application shell

**Files:**
- Modify: `client/src/layouts/DashboardLayout.tsx`
- Modify: `client/src/layouts/DashboardLayout.css`
- Modify: `client/src/index.css`
- Create: `client/src/styles/photoos-theme.css`
- Test: `client/scripts/wave1-ui-contract-check.mjs`

**Interfaces:**
- Consumes: existing session contract `/api/v1/me`, existing admin/user menu grouping.
- Produces: shared CSS variables and layout primitives used by all Wave 1 pages.

- [ ] **Step 1: Extend RED assertions**

Assert presence of professional theme variables for background/surface/border/text/accent, consistent sidebar/topbar/content structure, selected navigation state, responsive mobile drawer behavior, and no retired nav labels.

- [ ] **Step 2: Run RED**

```bash
cd client
node scripts/wave1-ui-contract-check.mjs
```

Expected: FAIL on the new theme/shell assertions.

- [ ] **Step 3: Implement the minimum shared shell**

Recreate the professional dark navy/blue hierarchy from the 18083 reference using source CSS variables and reusable shell classes. Preserve current route groups and `/api/v1/me` behavior. Do not add new dependencies.

- [ ] **Step 4: Verify GREEN and build**

```bash
cd client
node scripts/login-session-contract-check.mjs
node scripts/not-found-contract-check.mjs
node scripts/removed-features-check.mjs
node scripts/wave1-ui-contract-check.mjs
npm run contract-check
npm run build
```

Expected: all PASS; Vite build succeeds.

- [ ] **Step 5: Commit**

```bash
git add client/src/layouts/DashboardLayout.tsx client/src/layouts/DashboardLayout.css client/src/index.css client/src/styles/photoos-theme.css client/scripts/wave1-ui-contract-check.mjs
git commit -m "refactor(ui): restore professional PhotoOS shell"
```

### Task 3: Repair Health page contracts and error states

**Files:**
- Modify: `client/src/pages/HealthPage.tsx`
- Modify: `client/src/pages/HealthPage.css`
- Create or modify: `client/scripts/health-page-contract-check.mjs`
- Modify backend only if a required retained endpoint is genuinely missing.

**Interfaces:**
- Consumes: retained health/system/storage/raid endpoints only.
- Produces: a Health page that distinguishes loading, healthy, warning, unavailable, and genuine server-error states without querying retired services.

- [ ] **Step 1: Write failing endpoint/render contract**

The test must enumerate every literal endpoint HealthPage calls and require it to exist in the simplified backend source. Assert zero references to Mirror, Docker, Cloud Sync, or Automation.

- [ ] **Step 2: Run RED**

```bash
cd client
node scripts/health-page-contract-check.mjs
```

Expected: FAIL for every stale or mismatched call found by the audit.

- [ ] **Step 3: Implement minimal repair**

Replace stale calls with retained equivalents, remove retired health cards, and render professional status cards with explicit unavailable/error copy instead of blank content.

- [ ] **Step 4: Verify**

```bash
cd client
node scripts/health-page-contract-check.mjs
node scripts/wave1-ui-contract-check.mjs
npm run build
cd ..
cargo test -p server
```

Expected: frontend contracts PASS; build PASS; server tests PASS.

- [ ] **Step 5: Commit**

```bash
git add client/src/pages/HealthPage.tsx client/src/pages/HealthPage.css client/scripts/health-page-contract-check.mjs
git commit -m "fix(ui): repair health dashboard contracts"
```

### Task 4: Repair Logs page runtime rendering

**Files:**
- Modify: `client/src/pages/LogsPage.tsx`
- Modify: `client/src/pages/LogsPage.css`
- Create: `client/scripts/logs-page-contract-check.mjs`

**Interfaces:**
- Consumes: `/api/v1/logs`, `/api/v1/logs/services`, `/api/v1/logs/summary`, `/api/v1/logs/health`.
- Produces: a non-blank Logs page with bounded filters, loading, empty, and API-error states.

- [ ] **Step 1: Write RED contract**

Assert the retained log endpoints, page root, table/list container, explicit empty state, and explicit API error state.

- [ ] **Step 2: Reproduce the blank-page failure**

Run the current preview and capture the console/runtime error or the failing data-shape assumption before editing.

- [ ] **Step 3: Implement minimal fix**

Normalize API envelopes safely and render the professional log summary/filter/list layout without assuming fields that the backend does not guarantee.

- [ ] **Step 4: Verify build and runtime**

Run Node contracts, `npm run build`, server tests, then browser smoke on the isolated preview. Logs must visibly render with no uncaught console exception.

- [ ] **Step 5: Commit**

```bash
git add client/src/pages/LogsPage.tsx client/src/pages/LogsPage.css client/scripts/logs-page-contract-check.mjs
git commit -m "fix(ui): restore logs page rendering"
```

### Task 5: Repair Control page API status

**Files:**
- Modify: `client/src/pages/ControlPage.tsx`
- Modify: `client/src/pages/ControlPage.css`
- Create: `client/scripts/control-page-contract-check.mjs`

**Interfaces:**
- Consumes: only retained system/storage/control-safe read endpoints.
- Produces: a professional control overview with no “API Durumu: Bilinmiyor” when retained health endpoints are reachable.

- [ ] **Step 1: Write RED contract**
- [ ] **Step 2: Confirm current unknown-status cause**
- [ ] **Step 3: Map status to retained health/system endpoints**
- [ ] **Step 4: Verify isolated preview and build**
- [ ] **Step 5: Commit `fix(ui): repair control status contracts`**

### Task 6: Repair System/Updates page

**Files:**
- Modify: `client/src/pages/SystemUpdatesPage.tsx`
- Modify: `client/src/pages/SystemUpdatesPage.css`
- Create: `client/scripts/system-updates-contract-check.mjs`

**Interfaces:**
- Consumes: `/api/v1/system/update/status`, `/releases`, `/packages`, `/history` and retained system info endpoints.
- Produces: a professional update center that shows real unavailable/empty states rather than misleading zeros/dashes.

- [ ] **Step 1: Write RED contract**
- [ ] **Step 2: Audit exact backend response shapes**
- [ ] **Step 3: Normalize frontend response handling and restore professional sections**
- [ ] **Step 4: Verify contracts, build, server tests, isolated browser**
- [ ] **Step 5: Commit `fix(ui): restore system update center`**

### Task 7: Wave 1 isolated browser gate

**Files:**
- Create: `client/scripts/wave1-browser-smoke.mjs` if a supported local browser is available; otherwise record manual-browser evidence in the report.
- Update: `docs/superpowers/reports/2026-09-18-wave1-baseline.md`

**Interfaces:**
- Consumes: Tasks 2–6.
- Produces: browser-visible evidence for login, dashboard shell, Health, Logs, Control, and System/Updates.

- [ ] **Step 1: Build fresh production frontend and release server**
- [ ] **Step 2: Start isolated preview on a new port with a copied SQLite DB**
- [ ] **Step 3: Verify login → `/api/v1/me` → page navigation**
- [ ] **Step 4: Verify each Wave 1 page renders and no retired nav/routes appear**
- [ ] **Step 5: Verify no uncaught console/runtime errors and no unexpected 404/500 API calls**
- [ ] **Step 6: Run full regression suite**

Required:
```bash
cargo test -p server
cd client
node scripts/login-session-contract-check.mjs
node scripts/not-found-contract-check.mjs
node scripts/removed-features-check.mjs
node scripts/wave1-ui-contract-check.mjs
npm run contract-check
npm run build
```

- [ ] **Step 7: Commit evidence-only updates**

### Task 8: Stage a new release, but do not switch live yet

**Files:**
- Create a new immutable release directory under `/opt/photoos/releases/` only after all prior tasks pass.
- Generate fresh `SHA256SUMS`, `release-manifest.json`, and deployment evidence.

**Interfaces:**
- Consumes: verified Wave 1 branch HEAD.
- Produces: a staged release suitable for isolated HTTP/browser verification.

- [ ] **Step 1: Build release payload from the verified HEAD**
- [ ] **Step 2: Verify checksums and file permissions**
- [ ] **Step 3: Run isolated HTTP/API/browser smoke from the staged release**
- [ ] **Step 4: Confirm live `/opt/photoos/current` remains unchanged**
- [ ] **Step 5: Stop here if any browser-visible regression remains**

Live deployment is a separate controlled step after explicit verification of the staged release.
