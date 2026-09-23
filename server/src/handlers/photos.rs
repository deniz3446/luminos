use axum::{
    extract::{Multipart, Path as AxumPath, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{NaiveDateTime, Utc};
use exif::{In, Reader, Tag};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Write},
    path::Path,
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{auth::decode_token, state::AppState};

#[derive(Serialize)]
pub struct UploadResponse {
    pub success: bool,
    pub filename: String,
    pub path: String,
}

#[derive(Serialize, sqlx::FromRow, Clone)]
pub struct PhotoItem {
    pub id: i64,
    pub filename: String,
    pub original_name: String,
    pub url: String,
    pub size_bytes: i64,
    pub mime_type: String,
    pub uploaded_at: String,
    pub taken_at: Option<String>,
    pub user_id: i64,
    pub owner_username: String,
    pub source_type: String,
    pub source_path: Option<String>,
    pub device_name: Option<String>,
}

#[derive(Deserialize)]
pub struct DownloadZipRequest {
    pub filenames: Vec<String>,
}

#[derive(Deserialize)]
pub struct PhotoListParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}


fn get_user_id(headers: &HeaderMap) -> i64 {
    let Some(value) = headers.get(header::AUTHORIZATION) else {
        return 1;
    };

    let Ok(auth) = value.to_str() else {
        return 1;
    };

    let token = auth.strip_prefix("Bearer ").unwrap_or(auth);

    match decode_token(token) {
        Ok(claims) => claims.sub,
        Err(_) => 1,
    }
}

fn read_taken_at_from_exif(data: &[u8]) -> Option<String> {
    let mut cursor = Cursor::new(data);

    let exif = Reader::new().read_from_container(&mut cursor).ok()?;

    let field = exif
        .get_field(Tag::DateTimeOriginal, In::PRIMARY)
        .or_else(|| exif.get_field(Tag::DateTime, In::PRIMARY))?;

    let value = field.display_value().with_unit(&exif).to_string();

    let naive = NaiveDateTime::parse_from_str(value.trim(), "%Y:%m:%d %H:%M:%S").ok()?;

    Some(naive.and_utc().to_rfc3339())
}

pub async fn upload_photo(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, StatusCode> {
    let user_id = get_user_id(&headers);

    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let original_name = field.file_name().unwrap_or("photo.jpg").to_string();

        let extension = Path::new(&original_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");

        let mime_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
        let size_bytes = data.len() as i64;

        let taken_at =
            read_taken_at_from_exif(&data).unwrap_or_else(|| Utc::now().to_rfc3339());

        let now = Utc::now();
        let year = now.format("%Y").to_string();
        let month = now.format("%m").to_string();

        let new_filename = format!("{}.{}", Uuid::new_v4(), extension);

        let dir = format!("storage/users/{}/photos/{}/{}", username, year, month);

        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let save_path = format!("{}/{}", dir, new_filename);
        let url = format!("/api/v1/photos/file/{}", new_filename);

        let mut file = tokio::fs::File::create(&save_path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        file.write_all(&data)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        sqlx::query(
            r#"
            INSERT INTO photos (
                user_id,
                filename,
                original_name,
                path,
                url,
                size_bytes,
                mime_type,
                taken_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(&new_filename)
        .bind(&original_name)
        .bind(&save_path)
        .bind(&url)
        .bind(size_bytes)
        .bind(&mime_type)
        .bind(&taken_at)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return Ok(Json(UploadResponse {
            success: true,
            filename: new_filename,
            path: save_path,
        }));
    }

    Err(StatusCode::BAD_REQUEST)
}

pub async fn list_photos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PhotoListParams>,
) -> Json<Vec<PhotoItem>> {
    let user_id = get_user_id(&headers);

    let limit = params.limit.unwrap_or(100000).clamp(1, 100000);
    let offset = params.offset.unwrap_or(0).max(0);

    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| "user".to_string());

    let photos = if role == "admin" {
        sqlx::query_as::<_, PhotoItem>(
            r#"
            SELECT
                photos.id,
                photos.filename,
                photos.original_name,
                photos.url,
                photos.size_bytes,
                photos.mime_type,
                photos.uploaded_at,
                photos.taken_at,
                photos.user_id,
                COALESCE(users.username, 'Bilinmeyen') AS owner_username,
                photos.source_type,
                photos.source_path,
                photos.device_name
            FROM photos
            LEFT JOIN users ON users.id = photos.user_id
            ORDER BY COALESCE(photos.taken_at, photos.uploaded_at) DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, PhotoItem>(
            r#"
            SELECT
                photos.id,
                photos.filename,
                photos.original_name,
                photos.url,
                photos.size_bytes,
                photos.mime_type,
                photos.uploaded_at,
                photos.taken_at,
                photos.user_id,
                COALESCE(users.username, 'Bilinmeyen') AS owner_username,
                photos.source_type,
                photos.source_path,
                photos.device_name
            FROM photos
            LEFT JOIN users ON users.id = photos.user_id
            WHERE photos.user_id = ?
            ORDER BY COALESCE(photos.taken_at, photos.uploaded_at) DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    Json(photos)
}

pub async fn get_photo(
    State(state): State<AppState>,
    AxumPath(filename): AxumPath<String>,
) -> impl IntoResponse {
    let path: Option<String> =
        sqlx::query_scalar("SELECT path FROM photos WHERE filename = ?")
            .bind(&filename)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    let path = path.unwrap_or_else(|| format!("storage/photos/{}", filename));

    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let content_type = if filename.ends_with(".png") {
                "image/png"
            } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
                "image/jpeg"
            } else if filename.ends_with(".webp") {
                "image/webp"
            } else if filename.ends_with(".mp4") {
                "video/mp4"
            } else if filename.ends_with(".mov") {
                "video/quicktime"
            } else {
                "application/octet-stream"
            };

            ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn get_photo_thumb(
    State(state): State<AppState>,
    AxumPath(filename): AxumPath<String>,
) -> impl IntoResponse {
    let path: Option<String> =
        sqlx::query_scalar("SELECT path FROM photos WHERE filename = ?")
            .bind(&filename)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    let Some(path) = path else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let thumb_dir = "storage/thumbs";
    let thumb_path = format!("{}/{}.jpg", thumb_dir, filename);

    if let Ok(bytes) = tokio::fs::read(&thumb_path).await {
        return (
            [(header::CONTENT_TYPE, "image/jpeg")],
            bytes,
        )
            .into_response();
    }

    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let image = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(_) => {
            return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
        }
    };

    let thumb = image.resize(420, 420, FilterType::Triangle);

    let mut out = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut out, 72);

    if encoder.encode_image(&thumb).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let _ = tokio::fs::create_dir_all(thumb_dir).await;
    let _ = tokio::fs::write(&thumb_path, &out).await;

    (
        [(header::CONTENT_TYPE, "image/jpeg")],
        out,
    )
        .into_response()
}

pub async fn delete_photo(
    State(state): State<AppState>,
    AxumPath(filename): AxumPath<String>,
) -> impl IntoResponse {
    let path: Option<String> =
        sqlx::query_scalar("SELECT path FROM photos WHERE filename = ?")
            .bind(&filename)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    if let Some(path) = path {
        let _ = tokio::fs::remove_file(path).await;
    }

    match sqlx::query("DELETE FROM photos WHERE filename = ?")
        .bind(&filename)
        .execute(&state.db)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}


pub async fn photo_count(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let user_id = get_user_id(&headers);

    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| "user".to_string());

    let count: i64 = if role == "admin" {
        sqlx::query_scalar("SELECT COUNT(*) FROM photos")
            .fetch_one(&state.db)
            .await
            .unwrap_or(0)
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM photos WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0)
    };

    Json(serde_json::json!({
        "count": count
    }))
}

pub async fn download_photos_zip(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DownloadZipRequest>,
) -> impl IntoResponse {
    let user_id = get_user_id(&headers);

    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| "user".to_string());

    if payload.filenames.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let mut files: Vec<(String, String)> = Vec::new();

    for filename in &payload.filenames {
        let row = if role == "admin" {
            sqlx::query_as::<_, (String, String)>(
                "SELECT original_name, path FROM photos WHERE filename = ?",
            )
            .bind(filename)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
        } else {
            sqlx::query_as::<_, (String, String)>(
                "SELECT original_name, path FROM photos WHERE filename = ? AND user_id = ?",
            )
            .bind(filename)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
        };

        if let Some((original_name, path)) = row {
            files.push((original_name, path));
        }
    }

    if files.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let mut cursor = Cursor::new(Vec::<u8>::new());

    {
        let mut zip = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated);

        for (original_name, path) in files {
            if let Ok(bytes) = tokio::fs::read(&path).await {
                let safe_name = if original_name.trim().is_empty() {
                    "dosya".to_string()
                } else {
                    original_name
                };

                if zip.start_file(safe_name, options).is_ok() {
                    let _ = zip.write_all(&bytes);
                }
            }
        }

        let _ = zip.finish();
    }

    let bytes = cursor.into_inner();

    let headers = [
        (header::CONTENT_TYPE, HeaderValue::from_static("application/zip")),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"photoos-download.zip\""),
        ),
    ];

    (headers, bytes).into_response()
}

#[cfg(test)]
mod photo_source_contract_tests {
    use super::PhotoItem;

    #[test]
    fn photo_item_serializes_source_metadata() {
        let item = PhotoItem {
            id: 1,
            filename: "camera.jpg".to_string(),
            original_name: "camera.jpg".to_string(),
            url: "/api/v1/photos/file/camera.jpg".to_string(),
            size_bytes: 123,
            mime_type: "image/jpeg".to_string(),
            uploaded_at: "2026-09-20T00:00:00Z".to_string(),
            taken_at: None,
            user_id: 1,
            owner_username: "DenizBerk".to_string(),
            source_type: "camera".to_string(),
            source_path: Some("DCIM/Camera".to_string()),
            device_name: Some("Telefon".to_string()),
        };

        let json = serde_json::to_value(item).expect("PhotoItem JSON olmal?");
        assert_eq!(json["source_type"], "camera");
        assert_eq!(json["source_path"], "DCIM/Camera");
        assert_eq!(json["device_name"], "Telefon");
    }
}
