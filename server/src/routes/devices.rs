use axum::{Router, routing::get};

use crate::{handlers::devices::list_devices, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/devices", get(list_devices))
}
