use axum::{Router, routing::get};

use crate::{
    handlers::logs::{list_logs, log_health, log_services, log_summary},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/logs", get(list_logs))
        .route("/api/v1/logs/services", get(log_services))
        .route("/api/v1/logs/summary", get(log_summary))
        .route("/api/v1/logs/health", get(log_health))
}
