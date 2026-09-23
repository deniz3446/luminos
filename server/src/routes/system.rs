use axum::{routing::get, Router};

use crate::handlers::{system::{health, info}, system_metrics::system_metrics};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    println!("Loading system routes...");

    Router::new()
        .route("/api/v1/system/info", get(info))
        .route("/api/v1/system/health", get(health))
        .route("/api/v1/system/metrics", get(system_metrics))
}
