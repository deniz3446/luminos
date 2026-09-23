# PhotoOS Professional UI Restoration Design

**Date:** 2026-09-18
**Scope:** PhotoOS 1.2.1 simplified branch UI/UX restoration and page/API stabilization
**Source branch:** `final-simplification-20260917`
**Known good source head before UI restoration:** `2ca52f3b5de8cf2bddc11e2113f061fb58def81c`
**Reference UI:** old professional build served only as a visual/UX reference on port 18083
**Development preview:** reconstructed source preview on port 18082

## 1. Objective

Restore PhotoOS from the current reconstructed, visually regressed interface to a professional dark navy/blue product UI while preserving the simplified product scope and the already-removed Mirror, Docker Manager, Cloud Sync, and Automation features.

The restoration must fix both appearance and behavior. A page is not considered restored merely because it renders; its expected API calls, loading states, empty states, error states, responsive behavior, and navigation must also be correct.

## 2. Non-Negotiable Product Scope

The following features remain removed from frontend navigation, frontend routes, backend routes, background jobs, configuration, health checks, and new schema writes:

- Mirror
- Docker Manager UI/API management layer
- Cloud Sync
- Automation

The Docker engine itself remains installed where required by PhotoOS infrastructure.

Historical database migrations are preserved for compatibility. No destructive database cleanup is part of this project.

## 3. Visual Direction

The professional UI reference is the older PhotoOS dark navy/blue interface, not the current red-accent reconstructed shell.

The restored design will use:

- dark navy layered backgrounds rather than large flat black/red areas;
- blue/cyan accents for active navigation, status, focus, and primary actions;
- stronger card hierarchy and section grouping;
- readable typography and spacing at desktop resolutions;
- deliberate empty states rather than visually empty screens;
- consistent badges, status indicators, tables, cards, and error panels;
- a responsive sidebar and content layout suitable for desktop and mobile;
- consistent loading, success, warning, error, and unavailable states.

The old compiled build is a reference only. Old bundles will not be copied wholesale into the new release because they include stale routes, retired features, and outdated API assumptions.

## 4. Application Shell

`DashboardLayout` becomes the single professional application shell for all authenticated pages.

It owns:

- product brand area;
- grouped navigation;
- active route styling;
- mobile menu behavior;
- authenticated user display;
- logout;
- top-level page header area;
- common content width and spacing;
- common status/error presentation primitives.

The shell must never reference Mirror, Docker Manager, Cloud Sync, or Automation.

## 5. Page Restoration Order

### Wave 1 — Highest-Risk Pages

1. Health / System Center
2. Logs
3. Control
4. System / Updates

These pages are first because the video audit showed functional errors, blank output, unknown API state, or severe visual regression.

### Wave 2 — Infrastructure Pages

1. Storage
2. RAID
3. Backup
4. PC Backup
5. Notifications

These pages must display real backend state safely and distinguish unavailable hardware, empty data, and API failure.

### Wave 3 — Core Product Pages

1. Dashboard / Overview
2. Photos
3. Albums
4. Devices
5. Users
6. Settings / Preferences

These pages complete the product-wide visual system and remaining behavior checks.

## 6. API Contract Policy

Every page will have an explicit list of backend endpoints it is allowed to call.

For each endpoint:

- the backend route must exist unless the frontend deliberately handles absence;
- authentication behavior must be defined;
- successful response shape must match frontend expectations;
- 401/403/404/500 behavior must render a meaningful state instead of blank UI;
- retired feature endpoints must remain 404;
- browser console errors and unhandled promise rejections are release blockers.

No frontend page may silently assume an endpoint exists.

## 7. Health Page Requirements

Health must be rebuilt around endpoints that actually exist in the simplified backend.

It must show only supported PhotoOS health domains. Retired Mirror, Docker Manager, Cloud Sync, and Automation health probes must not return through another path or label.

Unavailable metrics must render a neutral "not available" state rather than repeated red errors. Genuine service/API failures must remain visible and actionable.

## 8. Logs Page Requirements

The Logs page must never render as a blank black screen.

It must have explicit loading, empty, success, filter, service-selection, and error states. API parse failures must be shown inside the page and logged to the browser console only when diagnostically useful.

## 9. Control and System/Updates Requirements

Control must use only endpoints supported by the current backend and must not display "API status unknown" simply because a historical endpoint was removed.

System/Updates must distinguish between real zero values, unavailable values, and failed requests. Update actions must keep their existing authorization and safety behavior.

## 10. Shared UI Components

Where current source structure allows without unnecessary refactoring, repeated UI should move into focused components such as:

- `PageHeader`
- `StatusCard`
- `MetricCard`
- `EmptyState`
- `ErrorState`
- `LoadingState`
- `SectionCard`
- status badges / pills

This project will not perform a broad component rewrite purely for aesthetics. Components are extracted only when they reduce duplication across pages being actively restored.

## 11. Testing Strategy

All behavior changes use RED -> minimal GREEN -> refactor.

Required gates include:

- source/contract tests for retired-feature absence;
- page-specific endpoint contract tests;
- backend unit/security tests where routes or response shapes change;
- frontend TypeScript production build;
- real preview server on an isolated port and copied SQLite database;
- authenticated HTTP smoke tests;
- real-browser visual/runtime smoke checks;
- desktop and mobile layout checks;
- no browser console runtime errors;
- no unexpected API 404/500 from supported pages;
- live `/opt/photoos/current` remains untouched until preview acceptance.

## 12. Deployment Policy

No page-level work deploys directly to live 8081.

Each wave is first built and verified from source in an isolated preview. Only after the complete restored UI is accepted will a new immutable release directory be built, checksummed, switched atomically, restarted, and verified with rollback evidence.

The final Installer4 ISO work remains paused until the restored live UI passes the same browser/API verification.

## 13. Safety and Data Rules

- Never modify the active SQLite database during preview testing; use a consistent copy.
- Never format or alter PhotoOS data disks as part of UI restoration.
- Never overwrite the active release directory in place.
- Never make the old professional bundle the production payload.
- Never reintroduce retired features to achieve visual parity.
- Preserve current authentication and authorization protections.

## 14. Completion Criteria

The restoration is complete only when:

- all retained pages render meaningful content or intentional empty states;
- Health, Logs, Control, and System/Updates have no known functional regressions;
- supported page API calls no longer produce unexplained 404/500 responses;
- browser runtime is free of uncaught errors;
- desktop and mobile layouts are usable without clipping or uncontrolled overflow;
- professional navy/blue visual language is consistent across the product;
- Mirror, Docker Manager, Cloud Sync, and Automation remain absent;
- full frontend contracts/build and backend test suite pass;
- an isolated release passes authenticated browser smoke before any live switch;
- the live release passes the same verification after atomic deployment.

## 15. Out of Scope

This project does not add new product features, redesign backend storage architecture, remove historical migrations, change disk topology, redesign PhotoOS authentication, or build the final ISO before UI stabilization is complete.
