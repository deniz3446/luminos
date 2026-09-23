use crate::{
    handlers::mobile::{create_pairing, heartbeat, pair, unpair, upload},
    state::AppState,
};
use axum::{
    Router,
    routing::{delete, get, post},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/mobile/pairing", post(create_pairing))
        .route("/api/v1/mobile/pair", post(pair))
        .route("/api/v1/mobile/heartbeat", get(heartbeat))
        .route("/api/v1/mobile/device", delete(unpair))
        .route("/api/v1/mobile/upload", post(upload))
}
