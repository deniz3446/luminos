use axum::{
    extract::State,
    Json,
};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
pub struct InfoResponse {
    product: &'static str,
    version: &'static str,
    status: &'static str,
    api: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
    })
}

pub async fn info(
    State(state): State<AppState>,
) -> Json<InfoResponse> {

    println!("==============================");
    println!("LuminOS AppState Test");
    println!("Storage Root : {}", state.settings.storage.root);
    println!("Database     : {}", state.settings.database.path);
    println!("==============================");

    Json(InfoResponse {
        product: "LuminOS Server",
        version: "0.1.0-dev",
        status: "running",
        api: "v1",
    })
}
