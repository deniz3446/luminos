use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    handlers::setup::{
        get_setup_preferences, initialize_setup, setup_status, update_setup_preferences,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/setup/status", get(setup_status))
        .route(
            "/api/v1/setup/preferences",
            get(get_setup_preferences).put(update_setup_preferences),
        )
        .route("/api/v1/setup/initialize", post(initialize_setup))
}
