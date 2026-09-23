# PhotoOS Phase 2B3 — Cloud Accounts Backend Reconstruction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the recovered Cloud Accounts `recovery_incomplete` stub with a tested SQLite-backed CRUD and health API matching the recovered schema and live binary field contract.

**Architecture:** Work occurs in an isolated `phase2b-cloud-accounts` Git worktree branched from the accepted canonical master. The handler maps the existing `cloud_accounts` table into a stable public API model, validates provider/status/JSON inputs before SQL execution, and exposes list/get/create/update/delete/health through the already-existing routes. Tests use an in-memory SQLite database with one connection so behavior is verified without touching the live runtime database.

**Tech Stack:** Rust 2024, Axum 0.8, SQLx 0.8 SQLite, serde/serde_json, UUID v4, Tokio tests, Git worktrees.

**Spec:** `docs/superpowers/specs/2026-09-12-photoos-final-source-design.md`

## Global Constraints

- Start from canonical master `8b366cc1f8c80f500a2e9846f0b53fe7d319e5b6`.
- Worktree: `/home/photoos/PhotoOS-worktrees/phase2b-cloud-accounts`.
- Branch: `phase2b-cloud-accounts`.
- Never modify `/opt/photoos/current`.
- Never restart PhotoOS/Nginx.
- Never write to `/var/lib/photoos/runtime/luminos.db`.
- Use the existing `cloud_accounts` schema from `server/migrations/20260723021550_cloud_engine_core.sql`.
- Preserve the existing routes:
  - `GET /api/v1/cloud/accounts/health`
  - `GET /api/v1/cloud/accounts`
  - `POST /api/v1/cloud/accounts`
  - `GET /api/v1/cloud/accounts/{id}`
  - `PUT /api/v1/cloud/accounts/{id}`
  - `DELETE /api/v1/cloud/accounts/{id}`
- Existing application middleware remains the auth/admin boundary.
- Public API must never expose secrets outside `config_json`; callers are responsible for not storing OAuth secrets there. This reconstruction does not migrate or inspect live account rows.
- Provider values must match the database CHECK constraint exactly:
  `google_drive`, `onedrive`, `dropbox`, `mega`, `s3`, `backblaze_b2`, `webdav`, `ftp`, `sftp`, `nextcloud`, `other`.
- Status values must match the database CHECK constraint exactly:
  `connected`, `disconnected`, `degraded`, `error`, `disabled`, `authorizing`.
- `display_name` and `remote_name` must be non-empty after trimming.
- `config_json` and `capabilities_json` must be JSON objects.
- Duplicate `remote_name` or `account_uuid` must return HTTP 409.
- Missing account ID must return HTTP 404.
- No production code change without an observed RED test first.

---

### Task 1: Create Isolated Worktree and Record Plan

**Files:**
- Create worktree: `/home/photoos/PhotoOS-worktrees/phase2b-cloud-accounts`
- Create branch: `phase2b-cloud-accounts`
- Create: `docs/superpowers/plans/2026-09-13-photoos-phase2b3-cloud-accounts-plan.md`

- [ ] Verify canonical master is clean and at the required HEAD.
- [ ] Create branch/worktree.
- [ ] Copy this plan into the worktree.
- [ ] Commit: `docs: add phase2b3 cloud accounts plan`.

### Task 2: RED — Validation and Health Classification

**Files:**
- Modify: `server/src/handlers/cloud_accounts.rs`
- Test: inline `#[cfg(test)]`

Add tests that require these production functions:

```rust
fn validate_provider(value: &str) -> Result<String, String>;
fn validate_status(value: &str) -> Result<String, String>;
fn validate_json_object(value: &serde_json::Value, field: &str) -> Result<(), String>;
fn classify_health(status: &str) -> &'static str;
```

Test requirements:

```rust
assert_eq!(validate_provider(" google_drive ").unwrap(), "google_drive");
assert!(validate_provider("unknown").is_err());

assert_eq!(validate_status(" connected ").unwrap(), "connected");
assert!(validate_status("broken").is_err());

assert!(validate_json_object(&json!({"root":"PhotoOS"}), "config_json").is_ok());
assert!(validate_json_object(&json!(["not-object"]), "config_json").is_err());

assert_eq!(classify_health("connected"), "healthy");
assert_eq!(classify_health("degraded"), "warning");
assert_eq!(classify_health("error"), "failed");
assert_eq!(classify_health("disconnected"), "unavailable");
assert_eq!(classify_health("disabled"), "unavailable");
assert_eq!(classify_health("authorizing"), "unavailable");
```

- [ ] Add tests.
- [ ] Run targeted test.
- [ ] Verify it fails because helpers do not exist.

### Task 3: GREEN — API Models and Validation

**Files:**
- Modify: `server/src/handlers/cloud_accounts.rs`

Create API model:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CloudAccount {
    pub id: i64,
    pub account_uuid: String,
    pub provider: String,
    pub display_name: String,
    pub remote_name: String,
    pub account_email: Option<String>,
    pub account_user_id: Option<String>,
    pub status: String,
    pub enabled: bool,
    pub config_json: Value,
    pub capabilities_json: Value,
    pub quota_total_bytes: Option<i64>,
    pub quota_used_bytes: Option<i64>,
    pub quota_free_bytes: Option<i64>,
    pub last_checked_at: Option<String>,
    pub last_connected_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

Create request types:

```rust
#[derive(Debug, Deserialize)]
pub struct CreateCloudAccountRequest {
    pub provider: String,
    pub display_name: String,
    pub remote_name: String,
    pub account_email: Option<String>,
    pub account_user_id: Option<String>,
    pub status: Option<String>,
    pub enabled: Option<bool>,
    #[serde(default = "empty_object")]
    pub config_json: Value,
    #[serde(default = "empty_object")]
    pub capabilities_json: Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCloudAccountRequest {
    pub provider: Option<String>,
    pub display_name: Option<String>,
    pub remote_name: Option<String>,
    pub account_email: Option<Option<String>>,
    pub account_user_id: Option<Option<String>>,
    pub status: Option<String>,
    pub enabled: Option<bool>,
    pub config_json: Option<Value>,
    pub capabilities_json: Option<Value>,
    pub quota_total_bytes: Option<Option<i64>>,
    pub quota_used_bytes: Option<Option<i64>>,
    pub quota_free_bytes: Option<Option<i64>>,
    pub last_checked_at: Option<Option<String>>,
    pub last_connected_at: Option<Option<String>>,
    pub last_error: Option<Option<String>>,
}
```

- [ ] Implement validation helpers.
- [ ] Run validation tests.
- [ ] Verify PASS.

### Task 4: RED — SQLite Repository Behavior

**Files:**
- Modify: `server/src/handlers/cloud_accounts.rs`
- Test: inline async tests

Create a single-connection in-memory SQLite database:

```rust
let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect("sqlite::memory:")
    .await
    .unwrap();
```

Create the exact `cloud_accounts` table structure required by the migration.

Write tests requiring:

```text
create_account()
list_accounts()
get_account()
update_account()
delete_account()
health_counts()
```

Behavior:
- create produces UUID and defaults status=`disconnected`, enabled=true;
- list returns inserted account;
- get returns inserted account;
- update changes display name/status/enabled;
- delete removes row;
- duplicate remote name is identified as conflict;
- health counts classify statuses into healthy/warning/failed/unavailable.

- [ ] Write async repository tests.
- [ ] Run targeted repository test.
- [ ] Verify RED due to missing repository functions.

### Task 5: GREEN — SQLite Repository

**Files:**
- Modify: `server/src/handlers/cloud_accounts.rs`

Implement:

```rust
async fn create_account(db: &SqlitePool, request: CreateCloudAccountRequest)
    -> Result<CloudAccount, CloudAccountError>;

async fn list_accounts(db: &SqlitePool)
    -> Result<Vec<CloudAccount>, CloudAccountError>;

async fn get_account(db: &SqlitePool, id: i64)
    -> Result<Option<CloudAccount>, CloudAccountError>;

async fn update_account(db: &SqlitePool, id: i64, request: UpdateCloudAccountRequest)
    -> Result<Option<CloudAccount>, CloudAccountError>;

async fn delete_account(db: &SqlitePool, id: i64)
    -> Result<bool, CloudAccountError>;

async fn health_counts(db: &SqlitePool)
    -> Result<CloudAccountHealth, CloudAccountError>;
```

`CloudAccountHealth` fields:

```rust
pub struct CloudAccountHealth {
    pub total: usize,
    pub healthy: usize,
    pub warning: usize,
    pub failed: usize,
    pub unavailable: usize,
}
```

Use parameterized SQL only.

- [ ] Implement repository.
- [ ] Run repository tests.
- [ ] Verify PASS.

### Task 6: GREEN — Axum Handler Contract

**Files:**
- Modify: `server/src/handlers/cloud_accounts.rs`
- Existing routes: `server/src/routes/cloud_accounts.rs`

Responses:

`GET /health`

```json
{
  "ok": true,
  "count": 1,
  "total": 1,
  "healthy": 0,
  "warning": 0,
  "failed": 0,
  "unavailable": 1
}
```

`GET /cloud/accounts`

```json
{
  "ok": true,
  "count": 1,
  "accounts": []
}
```

`GET /cloud/accounts/{id}`

```json
{
  "ok": true,
  "account": {}
}
```

`POST` returns HTTP 201.

`PUT` returns HTTP 200.

`DELETE` returns:

```json
{
  "ok": true,
  "id": 1,
  "deleted": true
}
```

Error response:

```json
{
  "ok": false,
  "error": "validation_error",
  "message": "..."
}
```

Status mapping:
- validation → 400;
- missing account → 404;
- unique conflict → 409;
- database failure → 500.

- [ ] Wire handlers to `State<AppState>`, `Path<i64>`, `Json<...>`.
- [ ] Verify existing route file compiles unchanged.

### Task 7: Full Verification and Static Safety Gate

Run:

```bash
cargo test -p server cloud_accounts --offline
cargo test -p server --offline
cargo check --workspace --offline
```

Static gates:
- no `recovery_incomplete` in `cloud_accounts.rs`;
- no raw SQL built from request string concatenation;
- migration contains `cloud_accounts`;
- canonical master unchanged;
- live release hash unchanged.

### Task 8: Commit, Review Artifact, No Merge

Commit:

```text
fix(cloud): reconstruct cloud accounts backend
```

Result archive must include:
- RED proof;
- targeted GREEN output;
- full cargo test;
- workspace cargo check;
- source snapshot;
- branch diff;
- acceptance report.

Acceptance marker:

```text
PHASE2B3_CLOUD_ACCOUNTS=PASS
```

Do not merge until the result diff has been reviewed.
