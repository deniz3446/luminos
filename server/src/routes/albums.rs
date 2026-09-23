use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::{
    handlers::albums::{
        add_photos_to_album,
        create_album,
        delete_album,
        get_album,
        list_albums,
        remove_photo_from_album,
    },
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/albums", get(list_albums))
        .route("/api/v1/albums", post(create_album))
        .route("/api/v1/albums/{album_id}", get(get_album))
        .route("/api/v1/albums/{album_id}", delete(delete_album))
        .route("/api/v1/albums/{album_id}/photos", post(add_photos_to_album))
        .route(
            "/api/v1/albums/{album_id}/photos/{photo_id}",
            delete(remove_photo_from_album),
        )
}
