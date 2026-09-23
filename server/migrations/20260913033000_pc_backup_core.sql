CREATE TABLE IF NOT EXISTS pc_backup_pairings (
    code TEXT PRIMARY KEY,
    expires_at INTEGER NOT NULL,
    created_by INTEGER NOT NULL,
    used_at INTEGER
);

CREATE TABLE IF NOT EXISTS pc_backup_devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    token_hash TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    status TEXT NOT NULL,
    files_uploaded INTEGER NOT NULL DEFAULT 0,
    bytes_uploaded INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_pc_backup_devices_last_seen
ON pc_backup_devices(last_seen DESC);
