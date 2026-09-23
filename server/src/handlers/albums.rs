use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{auth::decode_token, state::AppState};

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct AlbumItem {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub created_at: String,
    pub photo_count: i64,
    pub cover_url: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct AlbumPhotoItem {
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
}

#[derive(Debug, Serialize)]
pub struct AlbumDetailResponse {
    pub album: AlbumItem,
    pub photos: Vec<AlbumPhotoItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAlbumRequest {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddPhotosToAlbumRequest {
    pub photo_ids: Vec<i64>,
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

async fn get_user_role(state: &AppState, user_id: i64) -> String {
    sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| "user".to_string())
}

pub async fn list_albums(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AlbumItem>>, StatusCode> {
    let user_id = get_user_id(&headers);
    let role = get_user_role(&state, user_id).await;

    let albums = if role == "admin" {
        sqlx::query_as::<_, AlbumItem>(
            r#"
            SELECT
                a.id,
                a.user_id,
                a.title,
                a.description,
                a.created_at,
                COUNT(ai.id) AS photo_count,
                (
                    SELECT p.url
                    FROM album_items ai2
                    JOIN photos p ON p.id = ai2.photo_id
                    WHERE ai2.album_id = a.id
                    ORDER BY COALESCE(p.taken_at, p.uploaded_at) DESC
                    LIMIT 1
                ) AS cover_url
            FROM albums a
            LEFT JOIN album_items ai ON ai.album_id = a.id
            GROUP BY a.id
            ORDER BY a.created_at DESC
            "#,
        )
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        sqlx::query_as::<_, AlbumItem>(
            r#"
            SELECT
                a.id,
                a.user_id,
                a.title,
                a.description,
                a.created_at,
                COUNT(ai.id) AS photo_count,
                (
                    SELECT p.url
                    FROM album_items ai2
                    JOIN photos p ON p.id = ai2.photo_id
                    WHERE ai2.album_id = a.id
                    ORDER BY COALESCE(p.taken_at, p.uploaded_at) DESC
                    LIMIT 1
                ) AS cover_url
            FROM albums a
            LEFT JOIN album_items ai ON ai.album_id = a.id
            WHERE a.user_id = ?
            GROUP BY a.id
            ORDER BY a.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    Ok(Json(albums))
}

pub async fn create_album(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAlbumRequest>,
) -> Result<Json<AlbumItem>, StatusCode> {
    let user_id = get_user_id(&headers);

    let title = payload.title.trim();
    if title.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let result = sqlx::query(
        r#"
        INSERT INTO albums (user_id, title, description)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(title)
    .bind(payload.description.as_deref())
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let album_id = result.last_insert_rowid();

    let album = sqlx::query_as::<_, AlbumItem>(
        r#"
        SELECT
            a.id,
            a.user_id,
            a.title,
            a.description,
            a.created_at,
            0 AS photo_count,
            NULL AS cover_url
        FROM albums a
        WHERE a.id = ?
        "#,
    )
    .bind(album_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(album))
}

pub async fn get_album(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(album_id): Path<i64>,
) -> Result<Json<AlbumDetailResponse>, StatusCode> {
    let user_id = get_user_id(&headers);
    let role = get_user_role(&state, user_id).await;

    let album = if role == "admin" {
        sqlx::query_as::<_, AlbumItem>(
            r#"
            SELECT
                a.id,
                a.user_id,
                a.title,
                a.description,
                a.created_at,
                COUNT(ai.id) AS photo_count,
                (
                    SELECT p.url
                    FROM album_items ai2
                    JOIN photos p ON p.id = ai2.photo_id
                    WHERE ai2.album_id = a.id
                    ORDER BY COALESCE(p.taken_at, p.uploaded_at) DESC
                    LIMIT 1
                ) AS cover_url
            FROM albums a
            LEFT JOIN album_items ai ON ai.album_id = a.id
            WHERE a.id = ?
            GROUP BY a.id
            "#,
        )
        .bind(album_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        sqlx::query_as::<_, AlbumItem>(
            r#"
            SELECT
                a.id,
                a.user_id,
                a.title,
                a.description,
                a.created_at,
                COUNT(ai.id) AS photo_count,
                (
                    SELECT p.url
                    FROM album_items ai2
                    JOIN photos p ON p.id = ai2.photo_id
                    WHERE ai2.album_id = a.id
                    ORDER BY COALESCE(p.taken_at, p.uploaded_at) DESC
                    LIMIT 1
                ) AS cover_url
            FROM albums a
            LEFT JOIN album_items ai ON ai.album_id = a.id
            WHERE a.id = ? AND a.user_id = ?
            GROUP BY a.id
            "#,
        )
        .bind(album_id)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };


    let Some(album) = album else {
        return Err(StatusCode::NOT_FOUND);
    };

    let photos = sqlx::query_as::<_, AlbumPhotoItem>(
        r#"
        SELECT
            p.id,
            p.filename,
            p.original_name,
            p.url,
            p.size_bytes,
            p.mime_type,
            p.uploaded_at,
            p.taken_at,
            p.user_id,
            COALESCE(u.username, 'Bilinmeyen') AS owner_username
        FROM album_items ai
        JOIN photos p ON p.id = ai.photo_id
        LEFT JOIN users u ON u.id = p.user_id
        WHERE ai.album_id = ?
        ORDER BY COALESCE(p.taken_at, p.uploaded_at) DESC
        "#,
    )
    .bind(album_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AlbumDetailResponse { album, photos }))
}

pub async fn add_photos_to_album(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Path(album_id): Path<i64>,
    Json(payload): Json<AddPhotosToAlbumRequest>,
) -> Result<StatusCode, StatusCode> {
    let album_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if album_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    for photo_id in payload.photo_ids {
        let photo_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM photos WHERE id = ?")
            .bind(photo_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if photo_exists.is_none() {
            continue;
        }

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO album_items (album_id, photo_id)
            VALUES (?, ?)
            "#,
        )
        .bind(album_id)
        .bind(photo_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_photo_from_album(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Path((album_id, photo_id)): Path<(i64, i64)>,
) -> Result<StatusCode, StatusCode> {
    let album_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if album_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    sqlx::query("DELETE FROM album_items WHERE album_id = ? AND photo_id = ?")
        .bind(album_id)
        .bind(photo_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_album(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Path(album_id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    let album_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if album_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    sqlx::query("DELETE FROM albums WHERE id = ?")
        .bind(album_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}
