CREATE TABLE IF NOT EXISTS setup_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),

    server_name TEXT NOT NULL DEFAULT 'PhotoOS',
    hostname TEXT NOT NULL DEFAULT 'photoos',
    timezone TEXT NOT NULL DEFAULT 'Europe/Istanbul',
    language TEXT NOT NULL DEFAULT 'tr-TR',

    selected_disks_json TEXT NOT NULL DEFAULT '[]',
    raid_type TEXT NOT NULL DEFAULT 'basic',

    google_drive_enabled INTEGER NOT NULL DEFAULT 0,

    web_notifications_enabled INTEGER NOT NULL DEFAULT 1,
    email_notifications_enabled INTEGER NOT NULL DEFAULT 1,
    critical_only INTEGER NOT NULL DEFAULT 0,

    setup_version TEXT NOT NULL DEFAULT '2.1',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
