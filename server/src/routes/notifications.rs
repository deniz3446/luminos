use axum::{Router, routing::get};

use crate::{
    handlers::notifications::{
        list_notifications, notification_health, notification_socket, notification_summary,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/notifications", get(list_notifications))
        .route("/api/v1/notifications/summary", get(notification_summary))
        .route("/api/v1/notifications/health", get(notification_health))
        .route("/api/v1/notifications/ws", get(notification_socket))
}
