use axum::Router;

use crate::routes::system;

pub fn create_router() -> Router {
    Router::new()
        .merge(system::router())
}
