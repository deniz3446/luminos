use axum::{routing::get, Router};

use crate::handlers::system::{health, info};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    println!("Loading system routes...");

    Router::new()
        .route("/api/v1/system/info", get(info))
        .route("/api/v1/system/health", get(health))
}
