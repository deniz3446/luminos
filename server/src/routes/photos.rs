use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};

use crate::handlers::photos::{
    delete_photo,
    download_photos_zip,
    get_photo,
    get_photo_thumb,
    list_photos,
    photo_count,
    upload_photo,
};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/photos/upload", post(upload_photo))
        .route("/api/v1/photos", get(list_photos))
        .route("/api/v1/photos/count", get(photo_count))
        .route("/api/v1/photos/download-zip", post(download_photos_zip))
        .route("/api/v1/photos/thumb/{filename}", get(get_photo_thumb))
        .route("/api/v1/photos/file/{filename}", get(get_photo))
        .route("/api/v1/photos/file/{filename}", delete(delete_photo))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}
