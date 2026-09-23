# PhotoOS Phase 2B2 — Log Center Backend Reconstruction Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the recovered Log Center stub with a tested backend implementation that matches the live frontend contract without touching the live release.

**Architecture:** Work happens in an isolated Git worktree branched from canonical master. The implementation reads the systemd journal through a fixed executable and a whitelist of PhotoOS service units, parses journal JSON into the live frontend schema, applies safe in-process filtering, and returns structured JSON for list/services/summary/health endpoints. No shell is used and no user-controlled argument is passed without normalization/whitelisting.

**Tech Stack:** Rust, Axum, Tokio process API, serde/serde_json, systemd journal, Git worktrees.

**Spec:** `docs/superpowers/specs/2026-09-12-photoos-final-source-design.md`

## Global Constraints

- Start from canonical HEAD `f7d1f594237dcbeaf33af81837265123018ab2ea`.
- Never modify or deploy `/opt/photoos/current`.
- Never restart `photoos.service` or Nginx.
- Use isolated branch `phase2b-logs`.
- Worktree path: `/home/photoos/PhotoOS-worktrees/phase2b-logs`.
- Verify `photoos` can read at least its own systemd journal before source edits.
- Tests must be written and observed failing before production implementation.
- No shell command interpolation.
- Service selector is a fixed whitelist.
- `limit` clamps to `1..=2000`.
- `since_minutes` clamps to `1..=43200` to match the live UI 30-day option.
- Priority filter accepts only `all`, `emergency`, `alert`, `critical`, `error`, `warning`, `notice`, `info`, `debug`.
- Search is performed in-process against message/unit/identifier.
- Existing Nginx/backend auth policy remains unchanged.
- No live API behavior is changed during this phase.

---

### Task 1: Create Isolated Worktree

- Verify canonical master clean at expected HEAD.
- Create branch/worktree.
- Add this plan to `docs/superpowers/plans/`.
- Commit the plan.

### Task 2: Environment Readability Gate

Run as `photoos`:

```bash
/usr/bin/journalctl -u photoos.service -n 1 --no-pager -o json
```

Acceptance:
- executable exists;
- command exits `0`;
- output is either a valid JSON journal record or empty;
- if journal access is denied, stop before implementation.

### Task 3: RED — Journal Parser Contract

Add unit tests to `server/src/handlers/logs.rs` requiring:

- `__REALTIME_TIMESTAMP` microseconds → Unix seconds;
- numeric `PRIORITY` maps:
  - 0 emergency
  - 1 alert
  - 2 critical
  - 3 error
  - 4 warning
  - 5 notice
  - 6 info
  - 7 debug
- output record fields:
  - `id`
  - `timestamp`
  - `priority`
  - `level`
  - `unit`
  - `identifier`
  - `pid`
  - `boot_id`
  - `message`
- malformed journal JSON is ignored, not panicked.

Run parser test and observe RED because parser helper does not exist.

### Task 4: RED — Query Normalization and Whitelist

Add tests requiring:

- default service `photoos`;
- unknown service is rejected;
- `all` is accepted;
- limit 0 becomes 1;
- limit above 2000 becomes 2000;
- since 0 becomes 1;
- since `43200` is accepted and values above `43200` clamp to `43200`;
- unknown priority is rejected.

Observe RED.

### Task 5: GREEN — Implement Log Reader

Implement:

```text
LogEntry
LogService
NormalizedLogsQuery
parse_journal_line()
normalize_query()
service_units()
read_logs()
```

Use:

```text
/usr/bin/journalctl
--no-pager
--output=json
--reverse
--since=-<N>min
-n <fetch_limit>
-u <whitelisted unit>
```

For `service=all`, query the known PhotoOS units only, not the entire system journal.

Known service keys:

- `photoos`
- `storage`
- `notifications`
- `update`
- `installer`
- `ssh`

Apply priority and text-search filters after parsing.

### Task 6: GREEN — Live Frontend Response Contract

`GET /api/v1/logs/services`

```json
{
  "ok": true,
  "services": [
    {"key":"photoos","title":"PhotoOS Ana Servis","units":["photoos.service"]}
  ]
}
```

`GET /api/v1/logs?...`

```json
{
  "ok": true,
  "logs": [],
  "generated_at": 0
}
```

`GET /api/v1/logs/summary?...`

Must include:

```json
{
  "ok": true,
  "total": 0,
  "emergency": 0,
  "alert": 0,
  "critical": 0,
  "error": 0,
  "warning": 0,
  "notice": 0,
  "info": 0,
  "debug": 0,
  "unknown": 0,
  "generated_at": 0
}
```

`GET /api/v1/logs/health`

Returns `healthy` only when `/usr/bin/journalctl` is available.

### Task 7: Tests and Static Safety Gate

Required commands:

```bash
cargo test -p server logs --offline
cargo test -p server --offline
cargo check --workspace --offline
```

Static checks:

- no `recovery_incomplete` in `server/src/handlers/logs.rs`;
- no `Command::new("sh")`;
- no `Command::new("bash")`;
- canonical master unchanged;
- live binary hash unchanged.

### Task 8: Commit but Do Not Merge

Commit message:

```text
fix(logs): reconstruct log center backend
```

Produce result archive containing:
- RED proof;
- GREEN test output;
- cargo test/check reports;
- branch diff;
- source snapshot;
- acceptance report.

Acceptance marker:

```text
PHASE2B2_LOGS=PASS
```

Do not merge until diff/review is complete.
