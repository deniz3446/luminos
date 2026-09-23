CREATE TABLE IF NOT EXISTS mobile_pairings (
    code TEXT PRIMARY KEY,
    expires_at INTEGER NOT NULL,
    created_by INTEGER NOT NULL,
    used_at INTEGER
);

CREATE TABLE IF NOT EXISTS mobile_devices (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    status TEXT NOT NULL,
    files_uploaded INTEGER NOT NULL DEFAULT 0,
    bytes_uploaded INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_mobile_devices_user
ON mobile_devices(user_id);

CREATE INDEX IF NOT EXISTS idx_mobile_devices_last_seen
ON mobile_devices(last_seen DESC);
