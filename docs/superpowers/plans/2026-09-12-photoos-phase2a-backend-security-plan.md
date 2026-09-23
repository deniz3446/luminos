# PhotoOS Phase 2A — Backend Security Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the recovered hard-coded JWT secret, move JWT signing to a per-install runtime secret, enforce backend-native authentication for sensitive API families, and make localhost-only binding the secure source default.

**Architecture:** Work occurs in an isolated Git worktree branched from the Phase 1 accepted canonical source. Security behavior is implemented test-first. Nginx Repair 2/4 guards remain defense-in-depth, while the Rust router becomes the authoritative application security boundary. Media GET routes remain temporarily public to preserve the current frontend contract until Phase 3 adds authenticated media delivery.

**Tech Stack:** Rust 2024, Axum 0.8, jsonwebtoken 9, rand 0.8, SQLite/sqlx, Cargo, Git worktrees.

**Spec:** `docs/superpowers/specs/2026-09-12-photoos-final-source-design.md`

## Global Constraints

- Start from accepted canonical HEAD `2ccc828e6eecc5708a09ea65872cd819e5e8d13c`.
- Never modify `/opt/photoos/current` in Phase 2A.
- Never restart or deploy the live PhotoOS service in Phase 2A.
- Work on branch `phase2-backend-security` in `/home/photoos/PhotoOS-worktrees/phase2-backend-security`.
- Every production behavior change must have a failing test first.
- Nginx auth guards remain in place as defense-in-depth.
- `/api/v1/login`, `/api/v1/setup/status`, and `/api/v1/setup/initialize` remain public.
- Current GET/HEAD photo thumbnail/file delivery remains public until Phase 3 media-auth work.
- Sensitive list/metadata/admin APIs require backend authentication.
- Mutating protected APIs require an admin role.
- No hard-coded JWT signing secret may remain under `server/src`.
- JWT secret is generated per installation under the runtime directory and stored mode `0600`.
- Secure source default binding is `127.0.0.1:8081`.
- Do not merge the Phase 2A branch to canonical `master` until all Phase 2A tests and review pass.

---

### Task 1: Create the Isolated Phase 2A Worktree

**Files:**
- Create worktree: `/home/photoos/PhotoOS-worktrees/phase2-backend-security`
- Create branch: `phase2-backend-security`
- Create: `docs/superpowers/plans/2026-09-12-photoos-phase2a-backend-security-plan.md`

**Interfaces:**
- Consumes canonical Phase 1 repository at accepted HEAD.
- Produces isolated branch for all Phase 2A changes.

- [ ] Verify canonical repository is clean and HEAD is the expected Phase 1 acceptance commit.
- [ ] Create an external Git worktree and `phase2-backend-security` branch.
- [ ] Copy this plan into the worktree and commit it.
- [ ] Verify `/opt/photoos/current` is not the worktree path.

### Task 2: Backend Route Auth Policy — RED/GREEN

**Files:**
- Modify: `server/src/app.rs`
- Test: inline `#[cfg(test)]` module in `server/src/app.rs`

**Required behavior:**
- Authentication required:
  - `/api/v1/users`
  - `/api/v1/photos`
  - `/api/v1/photos/count`
  - `/api/v1/photos/timeline`
  - `/api/v1/photos/upload`
  - `/api/v1/photos/download-zip`
  - `/api/v1/albums` and children
  - `/api/v1/setup/preferences`
  - `/api/v1/cloud/accounts` and children
  - all already protected system/storage/raid/mirror/backups/automation/logs/notifications/dashboard/devices/docker/cloud-sync/update routes
- Temporary public media compatibility:
  - GET/HEAD `/api/v1/photos/thumb/{filename}`
  - GET/HEAD `/api/v1/photos/file/{filename}`
- Public bootstrap:
  - `/api/v1/login`
  - `/api/v1/setup/status`
  - `/api/v1/setup/initialize`
- Mutating protected requests require admin.
- OPTIONS remains available for CORS preflight.

**TDD cycle:**
- [ ] Add tests asserting sensitive routes are protected; run and observe failure.
- [ ] Add tests asserting bootstrap/media GET routes remain public.
- [ ] Implement the minimal route-policy changes.
- [ ] Run route-policy tests and observe PASS.
- [ ] Run all server tests.

### Task 3: Per-Install JWT Secret — RED/GREEN

**Files:**
- Modify: `server/src/auth/jwt.rs`
- Modify: `server/src/auth/mod.rs`
- Modify: `server/src/main.rs`
- Test: inline unit tests in `server/src/auth/jwt.rs`

**Interfaces:**
- Produces: `auth::initialize_secret(runtime_dir: &Path) -> Result<(), String>`
- Existing `auth::create_token(user_id: i64) -> String` remains compatible.
- Existing `auth::decode_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error>` remains compatible.

**Required behavior:**
- On first startup, create `<runtime.directory>/jwt-secret`.
- Secret contains at least 32 bytes of cryptographic randomness.
- Created file mode is `0600`.
- Existing secret is reused rather than replaced.
- Tokens signed with one secret fail validation under another secret.
- Server initializes JWT secret before login/router use.
- No `photoos-super-secret-key` remains in production Rust source.

**TDD cycle:**
- [ ] Add tests for create/reuse, permissions, and different-secret rejection; verify RED.
- [ ] Implement secret loading/generation and token helpers.
- [ ] Initialize secret in `main.rs`.
- [ ] Run JWT tests and observe PASS.
- [ ] Run all server tests.

### Task 4: Secure Bind Defaults — RED/GREEN

**Files:**
- Modify: `server/src/config/mod.rs`
- Modify: `config/config.toml`
- Test: inline config unit test

**Required behavior:**
- `ServerConfig::default().host == "127.0.0.1"`
- `ServerConfig::default().port == 8081`
- repository `config/config.toml` matches localhost `8081`

**TDD cycle:**
- [ ] Add failing default-bind test and observe RED.
- [ ] Change Rust defaults.
- [ ] Update repository config.
- [ ] Run config test and observe PASS.

### Task 5: Security Static Gate

**Files:**
- Create: `reports/phase2a-security-scan.txt`

**Required checks:**
- `server/src` contains no `photoos-super-secret-key`.
- route policy tests pass.
- JWT secret tests pass.
- secure bind default test passes.
- Nginx defense-in-depth guards still exist in source-controlled gateway.
- installer sanitization remains present.
- live `/opt/photoos/current` remains unchanged.

### Task 6: Build/Test Gate and Branch Commit

**Files:**
- Create: `reports/phase2a-cargo-test.txt`
- Create: `reports/phase2a-cargo-check.txt`
- Create: `reports/phase2a-acceptance.txt`

**Commands:**
- `cargo test -p server --offline`
- `cargo check --workspace --offline`

**Acceptance:**
- both commands return 0;
- no hard-coded JWT secret in production source;
- branch working tree clean after commits;
- live release path/hash unchanged;
- no merge/deploy performed.

**Final commit message:**
- `fix(security): harden backend auth and jwt secrets`

---

## Phase 2A Definition of Done

Phase 2A is complete when:
1. backend route auth protects the sensitive API families;
2. temporary media GET compatibility is explicitly tested;
3. JWT secret is per-install and file-backed with mode 0600;
4. recovered hard-coded JWT secret is gone from production source;
5. source defaults bind only to `127.0.0.1:8081`;
6. `cargo test -p server --offline` passes;
7. `cargo check --workspace --offline` passes;
8. the branch is clean and isolated;
9. the live release has not changed.

After review, the branch may be merged into canonical master. Then Phase 2B begins: live API parity capture and reconstruction of Logs, Cloud Accounts, Mobile and PC Backup stubs.
