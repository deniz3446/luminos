PRAGMA foreign_keys = ON;

-- ============================================================
-- PhotoOS Cloud Engine Core 1A
-- ============================================================

CREATE TABLE IF NOT EXISTS cloud_accounts (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    account_uuid        TEXT NOT NULL UNIQUE,

    provider            TEXT NOT NULL,
    display_name        TEXT NOT NULL,
    remote_name         TEXT NOT NULL UNIQUE,

    account_email       TEXT,
    account_user_id     TEXT,

    status              TEXT NOT NULL DEFAULT 'disconnected',
    enabled             INTEGER NOT NULL DEFAULT 1,

    config_json         TEXT NOT NULL DEFAULT '{}',
    capabilities_json   TEXT NOT NULL DEFAULT '{}',

    quota_total_bytes   INTEGER,
    quota_used_bytes    INTEGER,
    quota_free_bytes    INTEGER,

    last_checked_at     TEXT,
    last_connected_at   TEXT,
    last_error          TEXT,

    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CHECK (
        provider IN (
            'google_drive',
            'onedrive',
            'dropbox',
            'mega',
            's3',
            'backblaze_b2',
            'webdav',
            'ftp',
            'sftp',
            'nextcloud',
            'other'
        )
    ),

    CHECK (
        status IN (
            'connected',
            'disconnected',
            'degraded',
            'error',
            'disabled',
            'authorizing'
        )
    ),

    CHECK (enabled IN (0, 1))
);

CREATE INDEX IF NOT EXISTS idx_cloud_accounts_provider
    ON cloud_accounts(provider);

CREATE INDEX IF NOT EXISTS idx_cloud_accounts_status
    ON cloud_accounts(status);

CREATE INDEX IF NOT EXISTS idx_cloud_accounts_enabled
    ON cloud_accounts(enabled);


CREATE TABLE IF NOT EXISTS cloud_jobs (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    job_uuid                TEXT NOT NULL UNIQUE,

    account_id              INTEGER NOT NULL,

    name                    TEXT NOT NULL,
    description             TEXT,

    direction               TEXT NOT NULL DEFAULT 'upload',
    sync_mode               TEXT NOT NULL DEFAULT 'copy',

    local_path              TEXT NOT NULL,
    remote_path             TEXT NOT NULL DEFAULT '/',

    enabled                 INTEGER NOT NULL DEFAULT 1,
    status                  TEXT NOT NULL DEFAULT 'idle',

    schedule_type           TEXT NOT NULL DEFAULT 'manual',
    schedule_expression     TEXT,
    next_run_at             TEXT,

    conflict_policy         TEXT NOT NULL DEFAULT 'newer',
    delete_policy           TEXT NOT NULL DEFAULT 'keep',

    include_patterns_json   TEXT NOT NULL DEFAULT '[]',
    exclude_patterns_json   TEXT NOT NULL DEFAULT '[]',

    bandwidth_limit         TEXT,
    transfers               INTEGER NOT NULL DEFAULT 4,
    checkers                INTEGER NOT NULL DEFAULT 8,

    retry_count             INTEGER NOT NULL DEFAULT 3,
    retry_delay_seconds     INTEGER NOT NULL DEFAULT 30,

    last_run_at             TEXT,
    last_success_at         TEXT,
    last_error              TEXT,

    created_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY(account_id)
        REFERENCES cloud_accounts(id)
        ON DELETE CASCADE,

    CHECK (
        direction IN (
            'upload',
            'download',
            'bidirectional'
        )
    ),

    CHECK (
        sync_mode IN (
            'copy',
            'sync',
            'move',
            'bisync'
        )
    ),

    CHECK (
        status IN (
            'idle',
            'queued',
            'running',
            'paused',
            'completed',
            'failed',
            'cancelled',
            'disabled'
        )
    ),

    CHECK (
        schedule_type IN (
            'manual',
            'hourly',
            'daily',
            'weekly',
            'cron'
        )
    ),

    CHECK (
        conflict_policy IN (
            'newer',
            'local_wins',
            'remote_wins',
            'rename',
            'skip'
        )
    ),

    CHECK (
        delete_policy IN (
            'keep',
            'mirror',
            'trash'
        )
    ),

    CHECK (enabled IN (0, 1)),
    CHECK (transfers BETWEEN 1 AND 64),
    CHECK (checkers BETWEEN 1 AND 128),
    CHECK (retry_count BETWEEN 0 AND 100),
    CHECK (retry_delay_seconds BETWEEN 0 AND 86400)
);

CREATE INDEX IF NOT EXISTS idx_cloud_jobs_account_id
    ON cloud_jobs(account_id);

CREATE INDEX IF NOT EXISTS idx_cloud_jobs_status
    ON cloud_jobs(status);

CREATE INDEX IF NOT EXISTS idx_cloud_jobs_enabled
    ON cloud_jobs(enabled);

CREATE INDEX IF NOT EXISTS idx_cloud_jobs_next_run
    ON cloud_jobs(next_run_at);


CREATE TABLE IF NOT EXISTS cloud_job_runs (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    run_uuid                TEXT NOT NULL UNIQUE,

    job_id                  INTEGER NOT NULL,
    account_id              INTEGER NOT NULL,

    trigger_type            TEXT NOT NULL DEFAULT 'manual',
    status                  TEXT NOT NULL DEFAULT 'queued',

    process_id              INTEGER,
    rclone_command          TEXT,

    started_at              TEXT,
    finished_at             TEXT,

    duration_seconds        INTEGER,

    files_total             INTEGER NOT NULL DEFAULT 0,
    files_transferred       INTEGER NOT NULL DEFAULT 0,
    files_checked           INTEGER NOT NULL DEFAULT 0,
    files_failed            INTEGER NOT NULL DEFAULT 0,
    files_deleted           INTEGER NOT NULL DEFAULT 0,

    bytes_total             INTEGER NOT NULL DEFAULT 0,
    bytes_transferred       INTEGER NOT NULL DEFAULT 0,

    average_speed_bps       INTEGER NOT NULL DEFAULT 0,
    peak_speed_bps          INTEGER NOT NULL DEFAULT 0,

    retry_number            INTEGER NOT NULL DEFAULT 0,

    exit_code               INTEGER,
    error_code              TEXT,
    error_message           TEXT,

    stats_json              TEXT NOT NULL DEFAULT '{}',

    created_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY(job_id)
        REFERENCES cloud_jobs(id)
        ON DELETE CASCADE,

    FOREIGN KEY(account_id)
        REFERENCES cloud_accounts(id)
        ON DELETE CASCADE,

    CHECK (
        trigger_type IN (
            'manual',
            'schedule',
            'retry',
            'startup',
            'api'
        )
    ),

    CHECK (
        status IN (
            'queued',
            'starting',
            'running',
            'paused',
            'completed',
            'failed',
            'cancelled'
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_cloud_job_runs_job_id
    ON cloud_job_runs(job_id);

CREATE INDEX IF NOT EXISTS idx_cloud_job_runs_account_id
    ON cloud_job_runs(account_id);

CREATE INDEX IF NOT EXISTS idx_cloud_job_runs_status
    ON cloud_job_runs(status);

CREATE INDEX IF NOT EXISTS idx_cloud_job_runs_created_at
    ON cloud_job_runs(created_at DESC);


CREATE TABLE IF NOT EXISTS cloud_transfers (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    transfer_uuid           TEXT NOT NULL UNIQUE,

    run_id                  INTEGER NOT NULL,
    job_id                  INTEGER NOT NULL,
    account_id              INTEGER NOT NULL,

    operation               TEXT NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'queued',

    local_path              TEXT,
    remote_path             TEXT,

    file_name               TEXT,
    file_size_bytes         INTEGER NOT NULL DEFAULT 0,

    bytes_transferred       INTEGER NOT NULL DEFAULT 0,
    progress_percent        REAL NOT NULL DEFAULT 0,

    speed_bps               INTEGER NOT NULL DEFAULT 0,
    eta_seconds             INTEGER,

    checksum                TEXT,

    started_at              TEXT,
    finished_at             TEXT,

    retry_number            INTEGER NOT NULL DEFAULT 0,

    error_code              TEXT,
    error_message           TEXT,

    created_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY(run_id)
        REFERENCES cloud_job_runs(id)
        ON DELETE CASCADE,

    FOREIGN KEY(job_id)
        REFERENCES cloud_jobs(id)
        ON DELETE CASCADE,

    FOREIGN KEY(account_id)
        REFERENCES cloud_accounts(id)
        ON DELETE CASCADE,

    CHECK (
        operation IN (
            'upload',
            'download',
            'copy',
            'move',
            'delete',
            'check',
            'mkdir',
            'rename'
        )
    ),

    CHECK (
        status IN (
            'queued',
            'running',
            'completed',
            'failed',
            'skipped',
            'cancelled'
        )
    ),

    CHECK (
        progress_percent >= 0
        AND progress_percent <= 100
    )
);

CREATE INDEX IF NOT EXISTS idx_cloud_transfers_run_id
    ON cloud_transfers(run_id);

CREATE INDEX IF NOT EXISTS idx_cloud_transfers_job_id
    ON cloud_transfers(job_id);

CREATE INDEX IF NOT EXISTS idx_cloud_transfers_account_id
    ON cloud_transfers(account_id);

CREATE INDEX IF NOT EXISTS idx_cloud_transfers_status
    ON cloud_transfers(status);

CREATE INDEX IF NOT EXISTS idx_cloud_transfers_created_at
    ON cloud_transfers(created_at DESC);


CREATE TABLE IF NOT EXISTS cloud_engine_events (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,

    event_uuid          TEXT NOT NULL UNIQUE,

    account_id          INTEGER,
    job_id              INTEGER,
    run_id              INTEGER,

    level               TEXT NOT NULL DEFAULT 'info',
    event_type          TEXT NOT NULL,

    message             TEXT NOT NULL,
    details_json        TEXT NOT NULL DEFAULT '{}',

    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY(account_id)
        REFERENCES cloud_accounts(id)
        ON DELETE SET NULL,

    FOREIGN KEY(job_id)
        REFERENCES cloud_jobs(id)
        ON DELETE SET NULL,

    FOREIGN KEY(run_id)
        REFERENCES cloud_job_runs(id)
        ON DELETE SET NULL,

    CHECK (
        level IN (
            'debug',
            'info',
            'warning',
            'error',
            'critical'
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_cloud_engine_events_level
    ON cloud_engine_events(level);

CREATE INDEX IF NOT EXISTS idx_cloud_engine_events_type
    ON cloud_engine_events(event_type);

CREATE INDEX IF NOT EXISTS idx_cloud_engine_events_created_at
    ON cloud_engine_events(created_at DESC);


-- ============================================================
-- updated_at trigger'ları
-- ============================================================

CREATE TRIGGER IF NOT EXISTS trg_cloud_accounts_updated_at
AFTER UPDATE ON cloud_accounts
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE cloud_accounts
    SET updated_at = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;


CREATE TRIGGER IF NOT EXISTS trg_cloud_jobs_updated_at
AFTER UPDATE ON cloud_jobs
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE cloud_jobs
    SET updated_at = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;


CREATE TRIGGER IF NOT EXISTS trg_cloud_job_runs_updated_at
AFTER UPDATE ON cloud_job_runs
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE cloud_job_runs
    SET updated_at = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;


CREATE TRIGGER IF NOT EXISTS trg_cloud_transfers_updated_at
AFTER UPDATE ON cloud_transfers
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE cloud_transfers
    SET updated_at = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;
