use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MediaByUser {
    username: String,
    count: i64,
}

#[derive(Debug, Serialize)]
pub struct SourceStats {
    camera: i64,
    whatsapp_received: i64,
    whatsapp_sent: i64,
    screenshot: i64,
    download: i64,
    telegram: i64,
    other: i64,
}

#[derive(Debug, Serialize)]
pub struct UploadPeriodStats {
    today: i64,
    this_week: i64,
    this_month: i64,
    this_year: i64,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    user_count: i64,
    total_media_count: i64,
    photo_count: i64,
    video_count: i64,
    source_stats: SourceStats,
    upload_periods: UploadPeriodStats,
    media_by_user: Vec<MediaByUser>,
    version: String,
}

async fn count_source(
    state: &AppState,
    source_type: &str,
) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM photos WHERE source_type = ?",
    )
    .bind(source_type)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0)
}

pub async fn dashboard_stats(
    State(state): State<AppState>,
) -> Json<DashboardStats> {
    let user_count: i64 =
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM users",
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let total_media_count: i64 =
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM photos",
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let video_count: i64 =
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM photos
            WHERE mime_type LIKE 'video/%'
            "#,
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let photo_count =
        total_media_count - video_count;

    let source_stats = SourceStats {
        camera: count_source(
            &state,
            "camera",
        )
        .await,

        whatsapp_received: count_source(
            &state,
            "whatsapp_received",
        )
        .await,

        whatsapp_sent: count_source(
            &state,
            "whatsapp_sent",
        )
        .await,

        screenshot: count_source(
            &state,
            "screenshot",
        )
        .await,

        download: count_source(
            &state,
            "download",
        )
        .await,

        telegram: count_source(
            &state,
            "telegram",
        )
        .await,

        other: count_source(
            &state,
            "other",
        )
        .await,
    };

    let today: i64 =
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM photos
            WHERE date(uploaded_at) = date('now', 'localtime')
            "#,
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let this_week: i64 =
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM photos
            WHERE datetime(uploaded_at) >=
                  datetime('now', '-7 days')
            "#,
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let this_month: i64 =
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM photos
            WHERE strftime('%Y-%m', uploaded_at) =
                  strftime('%Y-%m', 'now', 'localtime')
            "#,
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let this_year: i64 =
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM photos
            WHERE strftime('%Y', uploaded_at) =
                  strftime('%Y', 'now', 'localtime')
            "#,
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let media_by_user =
        sqlx::query_as::<_, MediaByUser>(
            r#"
            SELECT
                COALESCE(
                    users.username,
                    'Bilinmeyen'
                ) AS username,
                COUNT(photos.id) AS count
            FROM photos
            LEFT JOIN users
                ON users.id = photos.user_id
            GROUP BY users.username
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(DashboardStats {
        user_count,
        total_media_count,
        photo_count,
        video_count,

        source_stats,

        upload_periods: UploadPeriodStats {
            today,
            this_week,
            this_month,
            this_year,
        },

        media_by_user,

        version: "1.0.0".to_string(),
    })
}
