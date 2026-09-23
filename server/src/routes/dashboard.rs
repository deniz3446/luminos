use axum::{routing::get, Router};

use crate::{
    handlers::dashboard::dashboard_stats,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/dashboard/stats", get(dashboard_stats))
}
