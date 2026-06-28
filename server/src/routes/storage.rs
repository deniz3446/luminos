use axum::{
    routing::get,
    Router,
};

use crate::{
    handlers::storage::get_storage_info,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/storage", get(get_storage_info))
}
