use crate::{
    handlers::raid::{create_raid_plan, get_raid_candidates, get_raid_manager_status},
    state::AppState,
};
use axum::{
    Router,
    routing::{get, post},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/raid/status", get(get_raid_manager_status))
        .route("/api/v1/raid/candidates", get(get_raid_candidates))
        .route("/api/v1/raid/plan", post(create_raid_plan))
}
