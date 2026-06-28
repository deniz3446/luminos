CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    user_id INTEGER NOT NULL,

    media_type TEXT NOT NULL,
    filename TEXT NOT NULL UNIQUE,
    original_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    extension TEXT NOT NULL,

    sha256 TEXT NOT NULL UNIQUE,
    size INTEGER NOT NULL,

    width INTEGER,
    height INTEGER,
    duration REAL,

    taken_at TEXT,
    uploaded_at TEXT NOT NULL,
    file_created_at TEXT,
    created_at TEXT NOT NULL,

    latitude REAL,
    longitude REAL,

    camera_make TEXT,
    camera_model TEXT,
    lens TEXT,
    iso INTEGER,
    aperture REAL,
    shutter_speed TEXT,
    focal_length REAL,

    favorite INTEGER NOT NULL DEFAULT 0,
    deleted INTEGER NOT NULL DEFAULT 0,

    FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_media_user
ON media(user_id);

CREATE INDEX IF NOT EXISTS idx_media_type
ON media(media_type);

CREATE INDEX IF NOT EXISTS idx_media_taken_at
ON media(taken_at);

CREATE INDEX IF NOT EXISTS idx_media_uploaded_at
ON media(uploaded_at);

CREATE INDEX IF NOT EXISTS idx_media_sha256
ON media(sha256);

CREATE INDEX IF NOT EXISTS idx_media_deleted
ON media(deleted);
