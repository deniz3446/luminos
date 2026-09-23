use axum::{Json, extract::State};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    handlers::mobile::{MOBILE_ONLINE_SECONDS, ensure_mobile_schema},
    state::AppState,
};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DeviceSummary {
    pub device_id: String,
    pub device_name: String,
    pub total_media_count: i64,
    pub photo_count: i64,
    pub video_count: i64,
    pub total_bytes: i64,
    pub ip_address: Option<String>,
    pub network_scope: Option<String>,
    pub last_filename: Option<String>,
    pub last_source_type: Option<String>,
    pub first_upload_at: Option<String>,
    pub last_upload_at: Option<String>,
    pub online: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct DeviceRow {
    device_id: String,
    device_name: String,
    total_media_count: i64,
    photo_count: i64,
    video_count: i64,
    total_bytes: i64,
    ip_address: Option<String>,
    network_scope: Option<String>,
    last_filename: Option<String>,
    last_source_type: Option<String>,
    first_upload_at: Option<String>,
    last_upload_at: Option<String>,
    online: i64,
}

async fn query_devices(db: &SqlitePool, now: i64) -> Result<Vec<DeviceSummary>, sqlx::Error> {
    let online_cutoff = now.saturating_sub(MOBILE_ONLINE_SECONDS);

    let rows = sqlx::query_as::<_, DeviceRow>(
        r#"
        SELECT
            d.id AS device_id,
            d.name AS device_name,
            COUNT(p.id) AS total_media_count,
            COALESCE(SUM(CASE WHEN p.mime_type LIKE 'image/%' THEN 1 ELSE 0 END), 0) AS photo_count,
            COALESCE(SUM(CASE WHEN p.mime_type LIKE 'video/%' THEN 1 ELSE 0 END), 0) AS video_count,
            COALESCE(SUM(p.size_bytes), 0) AS total_bytes,

            (
                SELECT p2.client_ip
                FROM photos p2
                WHERE p2.device_id = d.id
                ORDER BY COALESCE(p2.modified_at, p2.sort_timestamp, 0) DESC, p2.id DESC
                LIMIT 1
            ) AS ip_address,

            (
                SELECT p2.network_scope
                FROM photos p2
                WHERE p2.device_id = d.id
                ORDER BY COALESCE(p2.modified_at, p2.sort_timestamp, 0) DESC, p2.id DESC
                LIMIT 1
            ) AS network_scope,

            (
                SELECT COALESCE(p2.original_name, p2.filename)
                FROM photos p2
                WHERE p2.device_id = d.id
                ORDER BY COALESCE(p2.modified_at, p2.sort_timestamp, 0) DESC, p2.id DESC
                LIMIT 1
            ) AS last_filename,

            (
                SELECT p2.source_type
                FROM photos p2
                WHERE p2.device_id = d.id
                ORDER BY COALESCE(p2.modified_at, p2.sort_timestamp, 0) DESC, p2.id DESC
                LIMIT 1
            ) AS last_source_type,

            MIN(p.uploaded_at) AS first_upload_at,
            MAX(p.uploaded_at) AS last_upload_at,

            CASE WHEN d.last_seen >= ? THEN 1 ELSE 0 END AS online

        FROM mobile_devices d
        LEFT JOIN photos p ON p.device_id = d.id
        GROUP BY d.id, d.name, d.last_seen
        ORDER BY d.last_seen DESC, d.name ASC
        "#,
    )
    .bind(online_cutoff)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DeviceSummary {
            device_id: row.device_id,
            device_name: row.device_name,
            total_media_count: row.total_media_count,
            photo_count: row.photo_count,
            video_count: row.video_count,
            total_bytes: row.total_bytes,
            ip_address: row.ip_address,
            network_scope: row.network_scope,
            last_filename: row.last_filename,
            last_source_type: row.last_source_type,
            first_upload_at: row.first_upload_at,
            last_upload_at: row.last_upload_at,
            online: row.online == 1,
        })
        .collect())
}

pub async fn list_devices(State(state): State<AppState>) -> Json<Vec<DeviceSummary>> {
    if let Err(error) = ensure_mobile_schema(&state.db).await {
        eprintln!("Devices schema initialization failed: {error}");
        return Json(Vec::new());
    }

    match query_devices(&state.db, chrono::Utc::now().timestamp()).await {
        Ok(devices) => Json(devices),
        Err(error) => {
            eprintln!("Devices query failed: {error}");
            Json(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_db() -> SqlitePool {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE users(
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                email TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE photos(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                album_id INTEGER,
                filename TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&db)
        .await
        .unwrap();

        ensure_mobile_schema(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn devices_match_live_page_contract_and_two_minute_window() {
        let db = test_db().await;
        let now = 1_800_000_000_i64;

        sqlx::query(
            r#"
            INSERT INTO mobile_devices(
                id,user_id,name,token_hash,created_at,last_seen,status,files_uploaded,bytes_uploaded
            )
            VALUES
                ('dev-online',1,'Online Phone','hash-1',?,?,'online',2,300),
                ('dev-idle',1,'Idle Phone','hash-2',?,?,'online',0,0)
            "#,
        )
        .bind(now - 1000)
        .bind(now - 60)
        .bind(now - 1000)
        .bind(now - 121)
        .execute(&db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO photos(
                filename,created_at,original_name,path,url,size_bytes,mime_type,uploaded_at,
                sort_timestamp,user_id,content_hash,source_type,device_name,device_id,
                client_ip,network_scope,is_favorite
            )
            VALUES
                ('a.jpg','2026-01-01','Camera A.jpg','/tmp/a','/api/v1/photos/file/a.jpg',100,
                 'image/jpeg','2026-01-01T10:00:00Z',1000,1,'ha','camera','Online Phone',
                 'dev-online','192.168.1.10','local',0),
                ('b.mp4','2026-01-02','Video B.mp4','/tmp/b','/api/v1/photos/file/b.mp4',200,
                 'video/mp4','2026-01-02T10:00:00Z',2000,1,'hb','download','Online Phone',
                 'dev-online','8.8.8.8','external',0)
            "#,
        )
        .execute(&db)
        .await
        .unwrap();

        let devices = query_devices(&db, now).await.unwrap();
        assert_eq!(devices.len(), 2);

        let online = devices
            .iter()
            .find(|device| device.device_id == "dev-online")
            .unwrap();

        assert!(online.online);
        assert_eq!(online.device_name, "Online Phone");
        assert_eq!(online.total_media_count, 2);
        assert_eq!(online.photo_count, 1);
        assert_eq!(online.video_count, 1);
        assert_eq!(online.total_bytes, 300);
        assert_eq!(online.ip_address.as_deref(), Some("8.8.8.8"));
        assert_eq!(online.network_scope.as_deref(), Some("external"));
        assert_eq!(online.last_filename.as_deref(), Some("Video B.mp4"));
        assert_eq!(online.last_source_type.as_deref(), Some("download"));

        let idle = devices
            .iter()
            .find(|device| device.device_id == "dev-idle")
            .unwrap();

        assert!(!idle.online);
        assert_eq!(idle.total_media_count, 0);
        assert_eq!(idle.total_bytes, 0);
    }
}
