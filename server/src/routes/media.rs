use axum::{
    routing::post,
    Router,
};

use crate::{
    handlers::media::upload_media,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/media/upload", post(upload_media))
}
