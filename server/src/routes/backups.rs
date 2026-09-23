use crate::{
    handlers::{
        backups::{
            get_backup_history, get_backup_status, get_backup_targets, start_backup, stop_backup,
        },
        pc_backups::{
            browse_pc_backup, create_pc_pairing, get_pc_backup_library, pair_pc_device,
            pc_device_heartbeat, upload_pc_chunk, upload_pc_file,
        },
    },
    state::AppState,
};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post, put},
};

const PC_TRANSFER_BODY_LIMIT: usize = 64 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/backups/status", get(get_backup_status))
        .route("/api/v1/backups/history", get(get_backup_history))
        .route("/api/v1/backups/targets", get(get_backup_targets))
        .route("/api/v1/pc-backups", get(get_pc_backup_library))
        .route("/api/v1/pc-client/pairing", post(create_pc_pairing))
        .route("/api/v1/pc-client/pair", post(pair_pc_device))
        .route("/api/v1/pc-client/heartbeat", post(pc_device_heartbeat))
        .route("/api/v1/pc-client/file", put(upload_pc_file))
        .route("/api/v1/pc-client/chunk", put(upload_pc_chunk))
        .route("/api/v1/pc-backups/browse", get(browse_pc_backup))
        .route("/api/v1/backups/start", post(start_backup))
        .route("/api/v1/backups/stop", post(stop_backup))
        .layer(DefaultBodyLimit::max(PC_TRANSFER_BODY_LIMIT))
}
