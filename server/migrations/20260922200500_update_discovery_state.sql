CREATE TABLE IF NOT EXISTS photoos_update_preferences (
    singleton INTEGER PRIMARY KEY
        CHECK (singleton = 1),
    channel TEXT NOT NULL
        CHECK (
            channel IN (
                'stable',
                'beta',
                'disabled'
            )
        ),
    updated_at TEXT NOT NULL
        DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO photoos_update_preferences (
    singleton,
    channel
)
VALUES (
    1,
    'stable'
);

CREATE TABLE IF NOT EXISTS photoos_update_manifest_replay (
    channel TEXT PRIMARY KEY
        CHECK (
            channel IN (
                'stable',
                'beta'
            )
        ),
    version TEXT NOT NULL,
    published_at TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL
        CHECK (length(manifest_sha256) = 64),
    observed_at TEXT NOT NULL
        DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS photoos_update_operations (
    operation_id TEXT PRIMARY KEY,
    target_version TEXT NOT NULL,
    status TEXT NOT NULL
        CHECK (
            status IN (
                'downloading',
                'verifying',
                'verified',
                'installing',
                'succeeded',
                'failed'
            )
        ),
    updated_at TEXT NOT NULL
);
