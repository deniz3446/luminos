use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    handlers::updates::{
        check_for_updates, download_update, get_update_channel, install_verified_update,
        set_update_channel, verify_staged_update, install_package, rollback_release,
        update_history, update_packages, update_releases,
        update_status, upload_package, verify_package,
    },
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/system/update/status", get(update_status))
        .route(
            "/api/v1/system/update/channel",
            get(get_update_channel).post(set_update_channel),
        )
        .route(
            "/api/v1/system/update/check",
            post(check_for_updates),
        )
        .route(
            "/api/v1/system/update/download",
            post(download_update),
        )
        .route(
            "/api/v1/system/update/verify-staged",
            post(verify_staged_update),
        )
        .route(
            "/api/v1/system/update/install-verified",
            post(install_verified_update),
        )
        .route("/api/v1/system/update/releases", get(update_releases))
        .route("/api/v1/system/update/packages", get(update_packages))
        .route("/api/v1/system/update/history", get(update_history))
        .route("/api/v1/system/update/upload", post(upload_package))
        .route("/api/v1/system/update/verify", post(verify_package))
        .route("/api/v1/system/update/install", post(install_package))
        .route("/api/v1/system/update/rollback", post(rollback_release))
}
