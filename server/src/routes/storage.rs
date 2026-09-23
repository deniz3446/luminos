use axum::{Router, routing::get};

use crate::{
    storage_monitor::get_storage_metrics,
    handlers::storage::{
        get_smart_test_status, get_storage_disks, get_storage_health, get_storage_info,
        start_smart_test,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/storage", get(get_storage_info))
        .route("/api/v1/storage/disks", get(get_storage_disks))
        .route("/api/v1/storage/health", get(get_storage_health))
        .route("/api/v1/storage/metrics", get(get_storage_metrics))
        .route(
            "/api/v1/storage/disks/{device}/smart-test",
            get(get_smart_test_status).post(start_smart_test),
        )
}
