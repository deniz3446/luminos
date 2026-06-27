use axum::Router;

use crate::routes::system;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(system::router())
        .with_state(state)
}
