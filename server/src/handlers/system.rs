use axum::{
    extract::State,
    Json,
};
use serde::Serialize;

use crate::{config::Settings, state::AppState};

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct InfoResponse {
    product: &'static str,
    version: String,
    status: &'static str,
    api: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
    })
}

fn info_response(settings: &Settings) -> InfoResponse {
    InfoResponse {
        product: "PhotoOS Server",
        version: settings.release.version.clone(),
        status: "running",
        api: "v1",
    }
}

pub async fn info(
    State(state): State<AppState>,
) -> Json<InfoResponse> {
    Json(info_response(&state.settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_info_uses_photoos_product_and_configured_release() {
        let mut settings = Settings::default();
        settings.release.version = "9.9.9-test".to_string();

        let response = info_response(&settings);

        assert_eq!(response.product, "PhotoOS Server");
        assert_eq!(response.version, "9.9.9-test");
        assert_eq!(response.status, "running");
        assert_eq!(response.api, "v1");
    }
}
