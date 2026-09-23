use axum::{Router, routing::get};
use crate::{handlers::system::health, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/health", get(health))
}
