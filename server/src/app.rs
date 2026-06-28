use axum::Router;

use tower_http::cors::{
    Any,
    CorsLayer,
};

use crate::routes::{
    media,
    storage,
    system,
    user,
};

use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(system::router())
        .merge(user::router())
        .merge(media::router())
        .merge(storage::router())
        .layer(cors)
        .with_state(state)
}
