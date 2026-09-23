CREATE TABLE IF NOT EXISTS photoos_update_verified_operations (
    operation_id TEXT PRIMARY KEY,
    release_id TEXT NOT NULL,
    package_filename TEXT NOT NULL,
    package_sha256 TEXT NOT NULL
        CHECK (length(package_sha256) = 64),
    package_size INTEGER NOT NULL
        CHECK (package_size > 0),
    FOREIGN KEY (operation_id)
        REFERENCES photoos_update_operations(operation_id)
        ON DELETE CASCADE
);
