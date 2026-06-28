use axum::{
    extract::Multipart,
    http::StatusCode,
    Json,
};

use serde::Serialize;

#[derive(Serialize)]
pub struct UploadResponse {
    pub success: bool,
    pub message: String,
    pub filename: Option<String>,
}

pub async fn upload_media(
    mut multipart: Multipart,
) -> (StatusCode, Json<UploadResponse>) {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let file_name = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let data = field.bytes().await.unwrap();

        println!("Uploaded file: {}", file_name);
        println!("Size: {} bytes", data.len());

        return (
            StatusCode::OK,
            Json(UploadResponse {
                success: true,
                message: "Dosya alındı.".to_string(),
                filename: Some(file_name),
            }),
        );
    }

    (
        StatusCode::BAD_REQUEST,
        Json(UploadResponse {
            success: false,
            message: "Dosya bulunamadı.".to_string(),
            filename: None,
        }),
    )
}
