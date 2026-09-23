use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Mutex as StdMutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json,
    extract::{ConnectInfo, Multipart, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use rand::{RngCore, rngs::OsRng};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    auth::decode_token,
    state::AppState,
    storage::MediaKind,
};

const PAIRING_TTL_SECONDS: i64 = 600;
pub const MOBILE_ONLINE_SECONDS: i64 = 120;
const MAX_MOBILE_UPLOAD_BYTES: u64 = 2_000_000_000;
const MOBILE_TMP_DIR: &str = "/var/lib/photoos/runtime/tmp/mobile";
const PAIR_RATE_LIMIT_MAX: usize = 10;
const PAIR_RATE_LIMIT_WINDOW_SECONDS: i64 = 60;
static PAIR_ATTEMPTS: OnceLock<StdMutex<HashMap<IpAddr, VecDeque<i64>>>> = OnceLock::new();

#[derive(Debug)]
enum MobileError {
    Unauthorized,
    RateLimited,
    BadRequest(String),
    UnsupportedMedia,
    NotFound(String),
    Conflict(String),
    Database,
    Storage,
    Io,
}

impl IntoResponse for MobileError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Cihaz veya PhotoOS oturumu doğrulanamadı.".to_string(),
            ),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Çok fazla eşleştirme denemesi yapıldı. Kısa süre sonra tekrar deneyin.".to_string(),
            ),
            Self::UnsupportedMedia => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "Dosya içeriği desteklenen bir fotoğraf veya video değil.".to_string(),
            ),
            Self::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                message,
            ),
            Self::NotFound(message) => (
                StatusCode::NOT_FOUND,
                "not_found",
                message,
            ),
            Self::Conflict(message) => (
                StatusCode::CONFLICT,
                "conflict",
                message,
            ),
            Self::Database => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                "Mobile veritabanı işlemi başarısız.".to_string(),
            ),
            Self::Storage => (
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                "PhotoOS depolama alanı kullanılamıyor.".to_string(),
            ),
            Self::Io => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                "Mobile dosya işlemi başarısız.".to_string(),
            ),
        };

        (
            status,
            Json(json!({
                "ok": false,
                "error": code,
                "message": message,
            })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct PairRequest {
    pub code: String,
    #[serde(alias = "name", alias = "deviceName")]
    pub device_name: String,
}

#[derive(Debug, Clone)]
struct PairingCode {
    code: String,
    expires_at: i64,
}

#[derive(Debug, Clone)]
struct PairedDevice {
    id: String,
    user_id: i64,
    name: String,
}

#[derive(Debug, Clone)]
struct PairResult {
    device: PairedDevice,
    token: String,
}

#[derive(Debug, Default)]
struct UploadMetadata {
    source_type: Option<String>,
    source_path: Option<String>,
    media_store_id: Option<i64>,
    modified_at: Option<i64>,
    taken_at: Option<String>,
}

#[derive(Debug)]
struct UploadedTemp {
    path: PathBuf,
    original_name: String,
    mime_type: String,
    extension: String,
    size_bytes: u64,
    content_hash: String,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn random_device_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex_bytes(&bytes)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex_bytes(&hasher.finalize())
}

fn generate_pairing_code() -> String {
    let mut rng = OsRng;
    format!("{:06}", rng.next_u32() % 1_000_000)
}

fn bearer_token(headers: &HeaderMap) -> Result<String, MobileError> {
    let raw = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(MobileError::Unauthorized)?;

    let token = raw.strip_prefix("Bearer ").unwrap_or(raw).trim();

    if token.is_empty() {
        return Err(MobileError::Unauthorized);
    }

    Ok(token.to_string())
}

async fn photoos_user_id(db: &SqlitePool, headers: &HeaderMap) -> Result<i64, MobileError> {
    let token = bearer_token(headers)?;
    let claims = decode_token(&token).map_err(|_| MobileError::Unauthorized)?;

    let user_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE id = ? LIMIT 1")
            .bind(claims.sub)
            .fetch_optional(db)
            .await
            .map_err(|_| MobileError::Database)?;

    user_id.ok_or(MobileError::Unauthorized)
}

async fn authenticate_device(
    db: &SqlitePool,
    headers: &HeaderMap,
) -> Result<PairedDevice, MobileError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_token(&token);

    let row: Option<(String, i64, String)> = sqlx::query_as(
        "SELECT id, user_id, name FROM mobile_devices WHERE token_hash = ? LIMIT 1",
    )
    .bind(token_hash)
    .fetch_optional(db)
    .await
    .map_err(|_| MobileError::Database)?;

    row.map(|(id, user_id, name)| PairedDevice { id, user_id, name })
        .ok_or(MobileError::Unauthorized)
}

async fn photo_columns(db: &SqlitePool) -> Result<HashSet<String>, sqlx::Error> {
    let rows = sqlx::query("PRAGMA table_info(photos)").fetch_all(db).await?;
    let mut names = HashSet::new();

    for row in rows {
        names.insert(row.try_get::<String, _>("name")?);
    }

    Ok(names)
}

pub async fn ensure_mobile_schema(db: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS mobile_pairings (
            code TEXT PRIMARY KEY,
            expires_at INTEGER NOT NULL,
            created_by INTEGER NOT NULL,
            used_at INTEGER
        )
        "#,
    )
    .execute(db)
    .await
    .map_err(|_| "mobile_pairings oluşturulamadı".to_string())?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS mobile_devices (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            token_hash TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL,
            last_seen INTEGER NOT NULL,
            status TEXT NOT NULL,
            files_uploaded INTEGER NOT NULL DEFAULT 0,
            bytes_uploaded INTEGER NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(db)
    .await
    .map_err(|_| "mobile_devices oluşturulamadı".to_string())?;

    let existing = photo_columns(db)
        .await
        .map_err(|_| "photos şeması okunamadı".to_string())?;

    const COLUMNS: &[(&str, &str)] = &[
        ("original_name", "ALTER TABLE photos ADD COLUMN original_name TEXT"),
        ("path", "ALTER TABLE photos ADD COLUMN path TEXT"),
        ("url", "ALTER TABLE photos ADD COLUMN url TEXT"),
        (
            "size_bytes",
            "ALTER TABLE photos ADD COLUMN size_bytes INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "mime_type",
            "ALTER TABLE photos ADD COLUMN mime_type TEXT NOT NULL DEFAULT 'application/octet-stream'",
        ),
        ("uploaded_at", "ALTER TABLE photos ADD COLUMN uploaded_at TEXT"),
        ("taken_at", "ALTER TABLE photos ADD COLUMN taken_at TEXT"),
        (
            "sort_timestamp",
            "ALTER TABLE photos ADD COLUMN sort_timestamp INTEGER NOT NULL DEFAULT 0",
        ),
        ("user_id", "ALTER TABLE photos ADD COLUMN user_id INTEGER"),
        ("content_hash", "ALTER TABLE photos ADD COLUMN content_hash TEXT"),
        (
            "source_type",
            "ALTER TABLE photos ADD COLUMN source_type TEXT NOT NULL DEFAULT 'other'",
        ),
        ("source_path", "ALTER TABLE photos ADD COLUMN source_path TEXT"),
        ("device_name", "ALTER TABLE photos ADD COLUMN device_name TEXT"),
        ("device_id", "ALTER TABLE photos ADD COLUMN device_id TEXT"),
        ("client_ip", "ALTER TABLE photos ADD COLUMN client_ip TEXT"),
        ("network_scope", "ALTER TABLE photos ADD COLUMN network_scope TEXT"),
        ("media_store_id", "ALTER TABLE photos ADD COLUMN media_store_id INTEGER"),
        ("modified_at", "ALTER TABLE photos ADD COLUMN modified_at INTEGER"),
        (
            "is_favorite",
            "ALTER TABLE photos ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0",
        ),
    ];

    for (name, statement) in COLUMNS {
        if !existing.contains(*name) {
            sqlx::query(statement)
                .execute(db)
                .await
                .map_err(|_| format!("photos.{name} sütunu eklenemedi"))?;
        }
    }

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_photos_content_hash ON photos(content_hash)",
    )
    .execute(db)
    .await
    .map_err(|_| "content_hash index oluşturulamadı".to_string())?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_photos_sort_timestamp ON photos(sort_timestamp DESC, id DESC)",
    )
    .execute(db)
    .await
    .map_err(|_| "sort_timestamp index oluşturulamadı".to_string())?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_photos_user_device ON photos(user_id, device_id)",
    )
    .execute(db)
    .await
    .map_err(|_| "user_device index oluşturulamadı".to_string())?;

    Ok(())
}

pub async fn initialize_mobile(state: &AppState) -> Result<(), String> {
    ensure_mobile_schema(&state.db).await
}

async fn create_pairing_for_user(
    db: &SqlitePool,
    user_id: i64,
) -> Result<PairingCode, MobileError> {
    let now = unix_now();
    let expires_at = now + PAIRING_TTL_SECONDS;

    for _ in 0..16 {
        let code = generate_pairing_code();
        let result = sqlx::query(
            "INSERT INTO mobile_pairings(code, expires_at, created_by, used_at) VALUES (?, ?, ?, NULL)",
        )
        .bind(&code)
        .bind(expires_at)
        .bind(user_id)
        .execute(db)
        .await;

        match result {
            Ok(_) => return Ok(PairingCode { code, expires_at }),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => continue,
            Err(_) => return Err(MobileError::Database),
        }
    }

    Err(MobileError::Conflict(
        "Yeni eşleştirme kodu üretilemedi. Tekrar deneyin.".to_string(),
    ))
}

fn validate_pair_request(request: &PairRequest) -> Result<(String, String), MobileError> {
    let code = request.code.trim();

    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(MobileError::BadRequest(
            "Eşleştirme kodu 6 rakam olmalıdır.".to_string(),
        ));
    }

    let name = request.device_name.trim();

    if name.is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
        return Err(MobileError::BadRequest(
            "Geçerli bir cihaz adı gereklidir.".to_string(),
        ));
    }

    Ok((code.to_string(), name.to_string()))
}

async fn pair_device(db: &SqlitePool, request: PairRequest) -> Result<PairResult, MobileError> {
    let (code, name) = validate_pair_request(&request)?;
    let now = unix_now();
    let mut tx = db.begin().await.map_err(|_| MobileError::Database)?;

    let user_id: Option<i64> = sqlx::query_scalar(
        "SELECT created_by FROM mobile_pairings WHERE code = ? AND used_at IS NULL AND expires_at >= ?",
    )
    .bind(&code)
    .bind(now)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| MobileError::Database)?;

    let user_id = user_id.ok_or_else(|| {
        MobileError::BadRequest("Eşleştirme kodu geçersiz veya süresi dolmuş.".to_string())
    })?;

    let device_id = Uuid::new_v4().to_string();
    let token = random_device_token();
    let token_hash = hash_token(&token);

    sqlx::query(
        r#"
        INSERT INTO mobile_devices(
            id, user_id, name, token_hash, created_at, last_seen, status
        )
        VALUES (?, ?, ?, ?, ?, ?, 'online')
        "#,
    )
    .bind(&device_id)
    .bind(user_id)
    .bind(&name)
    .bind(token_hash)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|_| MobileError::Database)?;

    let consumed = sqlx::query(
        "UPDATE mobile_pairings SET used_at = ? WHERE code = ? AND used_at IS NULL",
    )
    .bind(now)
    .bind(&code)
    .execute(&mut *tx)
    .await
    .map_err(|_| MobileError::Database)?;

    if consumed.rows_affected() != 1 {
        return Err(MobileError::Conflict(
            "Eşleştirme kodu başka bir cihaz tarafından kullanıldı.".to_string(),
        ));
    }

    tx.commit().await.map_err(|_| MobileError::Database)?;

    Ok(PairResult {
        device: PairedDevice {
            id: device_id,
            user_id,
            name,
        },
        token,
    })
}

async fn heartbeat_device(db: &SqlitePool, device_id: &str) -> Result<i64, MobileError> {
    let now = unix_now();

    let result = sqlx::query(
        "UPDATE mobile_devices SET last_seen = ?, status = 'online' WHERE id = ?",
    )
    .bind(now)
    .bind(device_id)
    .execute(db)
    .await
    .map_err(|_| MobileError::Database)?;

    if result.rows_affected() != 1 {
        return Err(MobileError::NotFound("Cihaz bulunamadı.".to_string()));
    }

    Ok(now)
}

async fn delete_device(db: &SqlitePool, device_id: &str) -> Result<(), MobileError> {
    let result = sqlx::query("DELETE FROM mobile_devices WHERE id = ?")
        .bind(device_id)
        .execute(db)
        .await
        .map_err(|_| MobileError::Database)?;

    if result.rows_affected() != 1 {
        return Err(MobileError::NotFound("Cihaz bulunamadı.".to_string()));
    }

    Ok(())
}

fn safe_original_name(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("mobile-upload")
        .chars()
        .take(255)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DetectedMedia {
    mime_type: &'static str,
    extension: &'static str,
    video: bool,
}

fn detect_media_signature(bytes: &[u8]) -> Option<DetectedMedia> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(DetectedMedia { mime_type: "image/jpeg", extension: "jpg", video: false });
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(DetectedMedia { mime_type: "image/png", extension: "png", video: false });
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(DetectedMedia { mime_type: "image/webp", extension: "webp", video: false });
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(DetectedMedia { mime_type: "image/gif", extension: "gif", video: false });
    }
    if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Some(DetectedMedia { mime_type: "video/webm", extension: "webm", video: true });
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if matches!(
            brand,
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1"
        ) {
            return Some(DetectedMedia { mime_type: "image/heic", extension: "heic", video: false });
        }
        if matches!(brand, b"avif" | b"avis") {
            return Some(DetectedMedia { mime_type: "image/avif", extension: "avif", video: false });
        }
        if brand == b"qt  " {
            return Some(DetectedMedia { mime_type: "video/quicktime", extension: "mov", video: true });
        }
        if matches!(
            brand,
            b"isom" | b"iso2" | b"mp41" | b"mp42" | b"avc1" | b"M4V " | b"3gp4" | b"3gp5"
        ) {
            return Some(DetectedMedia { mime_type: "video/mp4", extension: "mp4", video: true });
        }
    }
    None
}

fn normalize_source_type(explicit: Option<&str>, source_path: Option<&str>) -> String {
    const ALLOWED: &[&str] = &[
        "camera",
        "whatsapp_received",
        "whatsapp_sent",
        "screenshot",
        "download",
        "telegram",
        "other",
    ];

    if let Some(value) = explicit {
        let value = value.trim().to_ascii_lowercase();
        if ALLOWED.contains(&value.as_str()) {
            return value;
        }
    }

    let path = source_path.unwrap_or_default().to_ascii_lowercase();

    if path.contains("whatsapp") && path.contains("sent") {
        "whatsapp_sent"
    } else if path.contains("whatsapp") {
        "whatsapp_received"
    } else if path.contains("screenshot") {
        "screenshot"
    } else if path.contains("telegram") {
        "telegram"
    } else if path.contains("download") {
        "download"
    } else if path.contains("dcim") || path.contains("camera") {
        "camera"
    } else {
        "other"
    }
    .to_string()
}

fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
        .or_else(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .and_then(|value| value.trim().parse().ok())
        })
}

fn request_ip(peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
    if peer.ip().is_loopback() {
        forwarded_ip(headers).unwrap_or_else(|| peer.ip())
    } else {
        peer.ip()
    }
}

fn pair_attempt_allowed(ip: IpAddr, now: i64) -> bool {
    let map = PAIR_ATTEMPTS.get_or_init(|| StdMutex::new(HashMap::new()));
    let Ok(mut map) = map.lock() else {
        return false;
    };
    let attempts = map.entry(ip).or_default();
    while attempts
        .front()
        .is_some_and(|timestamp| now.saturating_sub(*timestamp) >= PAIR_RATE_LIMIT_WINDOW_SECONDS)
    {
        attempts.pop_front();
    }
    if attempts.len() >= PAIR_RATE_LIMIT_MAX {
        return false;
    }
    attempts.push_back(now);
    true
}

fn clear_pair_attempts(ip: IpAddr) {
    if let Some(map) = PAIR_ATTEMPTS.get() {
        if let Ok(mut map) = map.lock() {
            map.remove(&ip);
        }
    }
}

fn network_scope(ip: Option<&str>) -> Option<String> {
    let ip: IpAddr = ip?.parse().ok()?;

    let local = match ip {
        IpAddr::V4(value) => {
            value.is_private() || value.is_loopback() || value.is_link_local()
        }
        IpAddr::V6(value) => {
            value.is_loopback() || value.is_unique_local() || value.is_unicast_link_local()
        }
    };

    Some(if local { "local" } else { "external" }.to_string())
}

async fn remove_temp(path: &Path) {
    let _ = tokio::fs::remove_file(path).await;
}

async fn save_multipart_file(
    mut field: axum::extract::multipart::Field<'_>,
    original_name: String,
) -> Result<UploadedTemp, MobileError> {
    tokio::fs::create_dir_all(MOBILE_TMP_DIR)
        .await
        .map_err(|_| MobileError::Io)?;

    let temp_path = PathBuf::from(MOBILE_TMP_DIR)
        .join(format!("{}.part", Uuid::new_v4()));

    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|_| MobileError::Io)?;

    let mut size_bytes = 0_u64;
    let mut hasher = Sha256::new();
    let mut probe = Vec::with_capacity(32);

    loop {
        let chunk = match field.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(_) => {
                drop(file);
                remove_temp(&temp_path).await;
                return Err(MobileError::BadRequest("Multipart dosyası okunamadı.".to_string()));
            }
        };

        size_bytes = size_bytes.saturating_add(chunk.len() as u64);
        if size_bytes > MAX_MOBILE_UPLOAD_BYTES {
            drop(file);
            remove_temp(&temp_path).await;
            return Err(MobileError::BadRequest(
                "Mobil yükleme boyut sınırını aşıyor.".to_string(),
            ));
        }

        if probe.len() < 32 {
            let take = (32 - probe.len()).min(chunk.len());
            probe.extend_from_slice(&chunk[..take]);
        }

        hasher.update(&chunk);
        if file.write_all(&chunk).await.is_err() {
            drop(file);
            remove_temp(&temp_path).await;
            return Err(MobileError::Io);
        }
    }

    if file.flush().await.is_err() {
        drop(file);
        remove_temp(&temp_path).await;
        return Err(MobileError::Io);
    }
    drop(file);

    let Some(media) = detect_media_signature(&probe) else {
        remove_temp(&temp_path).await;
        return Err(MobileError::UnsupportedMedia);
    };

    Ok(UploadedTemp {
        path: temp_path,
        original_name,
        mime_type: media.mime_type.to_string(),
        extension: media.extension.to_string(),
        size_bytes,
        content_hash: hex_bytes(&hasher.finalize()),
    })
}

async fn move_temp_to_storage(temp: &Path, destination: &Path) -> Result<(), MobileError> {
    let Some(parent) = destination.parent() else {
        return Err(MobileError::Storage);
    };

    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_| MobileError::Storage)?;

    tokio::fs::copy(temp, destination)
        .await
        .map_err(|_| MobileError::Storage)?;

    tokio::fs::remove_file(temp)
        .await
        .map_err(|_| MobileError::Io)?;

    Ok(())
}

pub async fn create_pairing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(_) = ensure_mobile_schema(&state.db).await {
        return MobileError::Database.into_response();
    }

    let user_id = match photoos_user_id(&state.db, &headers).await {
        Ok(user_id) => user_id,
        Err(error) => return error.into_response(),
    };

    match create_pairing_for_user(&state.db, user_id).await {
        Ok(pairing) => (
            StatusCode::CREATED,
            Json(json!({
                "ok": true,
                "code": pairing.code,
                "expires_at": pairing.expires_at,
                "expires_in_seconds": PAIRING_TTL_SECONDS,
            })),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn pair(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<PairRequest>,
) -> Response {
    if let Err(_) = ensure_mobile_schema(&state.db).await {
        return MobileError::Database.into_response();
    }

    let client_ip = request_ip(peer, &headers);
    if !pair_attempt_allowed(client_ip, unix_now()) {
        return MobileError::RateLimited.into_response();
    }

    match pair_device(&state.db, request).await {
        Ok(result) => {
            clear_pair_attempts(client_ip);
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "device_id": result.device.id,
                    "device_name": result.device.name,
                    "token": result.token,
                })),
            )
                .into_response()
        }
        Err(error) => error.into_response(),
    }
}

pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(_) = ensure_mobile_schema(&state.db).await {
        return MobileError::Database.into_response();
    }

    let device = match authenticate_device(&state.db, &headers).await {
        Ok(device) => device,
        Err(error) => return error.into_response(),
    };

    match heartbeat_device(&state.db, &device.id).await {
        Ok(last_seen) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "device_id": device.id,
                "status": "online",
                "last_seen": last_seen,
            })),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn unpair(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(_) = ensure_mobile_schema(&state.db).await {
        return MobileError::Database.into_response();
    }

    let device = match authenticate_device(&state.db, &headers).await {
        Ok(device) => device,
        Err(error) => return error.into_response(),
    };

    match delete_device(&state.db, &device.id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "device_id": device.id,
                "deleted": true,
            })),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn upload(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(_) = ensure_mobile_schema(&state.db).await {
        return MobileError::Database.into_response();
    }

    let device = match authenticate_device(&state.db, &headers).await {
        Ok(device) => device,
        Err(error) => return error.into_response(),
    };

    let client_ip = Some(request_ip(peer, &headers).to_string());
    let mut metadata = UploadMetadata::default();
    let mut uploaded: Option<UploadedTemp> = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => {
                if let Some(file) = uploaded.as_ref() {
                    let _ = tokio::fs::remove_file(&file.path).await;
                }
                return MobileError::BadRequest("Multipart isteği geçersiz.".to_string())
                    .into_response();
            }
        };

        let field_name = field.name().unwrap_or_default().to_string();
        let file_name = field.file_name().map(str::to_string);

        if let Some(file_name) = file_name {
            if uploaded.is_some() {
                if let Some(file) = uploaded.as_ref() {
                    remove_temp(&file.path).await;
                }
                return MobileError::BadRequest(
                    "Her istekte yalnızca bir medya dosyası gönderilebilir.".to_string(),
                )
                .into_response();
            }

            let original_name = safe_original_name(&file_name);
            match save_multipart_file(field, original_name).await {
                Ok(file) => uploaded = Some(file),
                Err(error) => return error.into_response(),
            }

            continue;
        }

        let text = match field.text().await {
            Ok(text) => text,
            Err(_) => {
                if let Some(file) = uploaded.as_ref() {
                    remove_temp(&file.path).await;
                }
                return MobileError::BadRequest(
                    "Multipart metadata alanı okunamadı.".to_string(),
                )
                .into_response();
            }
        };

        match field_name.as_str() {
            "source_type" => metadata.source_type = Some(text),
            "source_path" => metadata.source_path = Some(text),
            "media_store_id" => metadata.media_store_id = text.trim().parse().ok(),
            "modified_at" => metadata.modified_at = text.trim().parse().ok(),
            "taken_at" => {
                let value = text.trim();
                if !value.is_empty() {
                    metadata.taken_at = Some(value.to_string());
                }
            }
            _ => {}
        }
    }

    let Some(file) = uploaded else {
        return MobileError::BadRequest("Yüklenecek medya dosyası bulunamadı.".to_string())
            .into_response();
    };

    let duplicate: Result<Option<(String, Option<String>)>, sqlx::Error> = sqlx::query_as(
        "SELECT filename, url FROM photos WHERE user_id = ? AND content_hash = ? LIMIT 1",
    )
    .bind(device.user_id)
    .bind(&file.content_hash)
    .fetch_optional(&state.db)
    .await;

    match duplicate {
        Ok(Some((filename, url))) => {
            let _ = tokio::fs::remove_file(&file.path).await;
            let _ = heartbeat_device(&state.db, &device.id).await;

            return (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "duplicate": true,
                    "filename": filename,
                    "url": url,
                    "content_hash": file.content_hash,
                })),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(_) => {
            let _ = tokio::fs::remove_file(&file.path).await;
            return MobileError::Database.into_response();
        }
    }

    let new_filename = format!("{}.{}", Uuid::new_v4(), file.extension);
    let media_kind = if file.mime_type.starts_with("video/") {
        MediaKind::Video
    } else {
        MediaKind::Photo
    };

    let target = match state
        .storage
        .allocate(media_kind, &new_filename, file.size_bytes)
        .await
    {
        Ok(target) => target,
        Err(_) => {
            let _ = tokio::fs::remove_file(&file.path).await;
            return MobileError::Storage.into_response();
        }
    };

    if let Err(error) = move_temp_to_storage(&file.path, &target.absolute_path).await {
        let _ = tokio::fs::remove_file(&file.path).await;
        return error.into_response();
    }

    let now = Utc::now();
    let created_at = now.to_rfc3339();
    let uploaded_at = created_at.clone();
    let sort_timestamp = metadata.modified_at.unwrap_or_else(|| now.timestamp_millis());
    let source_type =
        normalize_source_type(metadata.source_type.as_deref(), metadata.source_path.as_deref());
    let ip_scope = network_scope(client_ip.as_deref());
    let url = format!("/api/v1/photos/file/{new_filename}");
    let path = target.absolute_path.display().to_string();

    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            let _ = tokio::fs::remove_file(&target.absolute_path).await;
            return MobileError::Database.into_response();
        }
    };

    let insert = sqlx::query(
        r#"
        INSERT INTO photos (
            filename, created_at, original_name, path, url,
            size_bytes, mime_type, uploaded_at, taken_at, sort_timestamp,
            user_id, content_hash, source_type, source_path,
            device_name, device_id, client_ip, network_scope,
            media_store_id, modified_at, is_favorite
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
        "#,
    )
    .bind(&new_filename)
    .bind(&created_at)
    .bind(&file.original_name)
    .bind(&path)
    .bind(&url)
    .bind(file.size_bytes as i64)
    .bind(&file.mime_type)
    .bind(&uploaded_at)
    .bind(metadata.taken_at)
    .bind(sort_timestamp)
    .bind(device.user_id)
    .bind(&file.content_hash)
    .bind(&source_type)
    .bind(metadata.source_path)
    .bind(&device.name)
    .bind(&device.id)
    .bind(client_ip.clone())
    .bind(ip_scope.clone())
    .bind(metadata.media_store_id)
    .bind(metadata.modified_at)
    .execute(&mut *tx)
    .await;

    if insert.is_err() {
        let _ = tx.rollback().await;
        let _ = tokio::fs::remove_file(&target.absolute_path).await;
        return MobileError::Database.into_response();
    }

    let now_unix = unix_now();
    let counter = sqlx::query(
        r#"
        UPDATE mobile_devices
        SET
            last_seen = ?,
            status = 'online',
            files_uploaded = files_uploaded + 1,
            bytes_uploaded = bytes_uploaded + ?
        WHERE id = ?
        "#,
    )
    .bind(now_unix)
    .bind(file.size_bytes as i64)
    .bind(&device.id)
    .execute(&mut *tx)
    .await;

    if !matches!(counter, Ok(ref value) if value.rows_affected() == 1) {
        let _ = tx.rollback().await;
        let _ = tokio::fs::remove_file(&target.absolute_path).await;
        return MobileError::Database.into_response();
    }

    if tx.commit().await.is_err() {
        let _ = tokio::fs::remove_file(&target.absolute_path).await;
        return MobileError::Database.into_response();
    }

    (
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "duplicate": false,
            "filename": new_filename,
            "original_name": file.original_name,
            "url": url,
            "path": path,
            "size_bytes": file.size_bytes,
            "mime_type": file.mime_type,
            "content_hash": file.content_hash,
            "source_type": source_type,
            "device_id": device.id,
            "device_name": device.name,
            "client_ip": client_ip,
            "network_scope": ip_scope,
        })),
    )
        .into_response()
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

        sqlx::query(
            "INSERT INTO users(id, username, email, password_hash, created_at) VALUES (1, 'u', 'u@example.invalid', 'x', 'now')",
        )
        .execute(&db)
        .await
        .unwrap();

        ensure_mobile_schema(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn schema_and_pairing_round_trip() {
        let db = test_db().await;

        let columns = photo_columns(&db).await.unwrap();
        for column in [
            "size_bytes",
            "mime_type",
            "uploaded_at",
            "content_hash",
            "source_type",
            "device_name",
            "device_id",
            "client_ip",
            "network_scope",
        ] {
            assert!(columns.contains(column), "missing photos.{column}");
        }

        let before = unix_now();
        let pairing = create_pairing_for_user(&db, 1).await.unwrap();
        assert_eq!(pairing.code.len(), 6);
        assert!(pairing.code.bytes().all(|byte| byte.is_ascii_digit()));
        assert!(pairing.expires_at >= before + PAIRING_TTL_SECONDS - 1);

        let result = pair_device(
            &db,
            PairRequest {
                code: pairing.code.clone(),
                device_name: "Deniz Phone".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.device.user_id, 1);
        assert_eq!(result.device.name, "Deniz Phone");
        assert_eq!(result.token.len(), 64);

        let stored_hash: String =
            sqlx::query_scalar("SELECT token_hash FROM mobile_devices WHERE id = ?")
                .bind(&result.device.id)
                .fetch_one(&db)
                .await
                .unwrap();

        assert_eq!(stored_hash, hash_token(&result.token));
        assert_ne!(stored_hash, result.token);

        let second = pair_device(
            &db,
            PairRequest {
                code: pairing.code,
                device_name: "Second".to_string(),
            },
        )
        .await;

        assert!(second.is_err());

        let now = heartbeat_device(&db, &result.device.id).await.unwrap();
        let last_seen: i64 =
            sqlx::query_scalar("SELECT last_seen FROM mobile_devices WHERE id = ?")
                .bind(&result.device.id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(last_seen, now);

        delete_device(&db, &result.device.id).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mobile_devices")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn token_source_and_network_helpers_are_safe() {
        let token = random_device_token();
        assert_eq!(token.len(), 64);
        assert_eq!(hash_token("same"), hash_token("same"));
        assert_ne!(hash_token("same"), hash_token("different"));

        assert_eq!(
            normalize_source_type(None, Some("/storage/emulated/0/DCIM/Camera/a.jpg")),
            "camera"
        );
        assert_eq!(
            normalize_source_type(None, Some("/WhatsApp/Media/WhatsApp Images/Sent/a.jpg")),
            "whatsapp_sent"
        );
        assert_eq!(
            normalize_source_type(Some("screenshot"), None),
            "screenshot"
        );

        assert_eq!(network_scope(Some("192.168.1.20")).as_deref(), Some("local"));
        assert_eq!(network_scope(Some("8.8.8.8")).as_deref(), Some("external"));
        assert_eq!(network_scope(None), None);
    }

    #[test]
    fn online_window_contract_is_two_minutes() {
        assert_eq!(MOBILE_ONLINE_SECONDS, 120);
        assert_eq!(PAIRING_TTL_SECONDS, 600);
    }

    #[test]
    fn peer_ip_trusts_proxy_headers_only_from_loopback() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "8.8.8.8".parse().unwrap());

        let direct: SocketAddr = "192.168.1.10:5000".parse().unwrap();
        assert_eq!(request_ip(direct, &headers).to_string(), "192.168.1.10");

        let proxy: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        assert_eq!(request_ip(proxy, &headers).to_string(), "8.8.8.8");
    }

    #[test]
    fn media_signature_controls_mime_and_extension() {
        let jpeg = detect_media_signature(&[0xff, 0xd8, 0xff, 0x00]).unwrap();
        assert_eq!(jpeg.mime_type, "image/jpeg");
        assert_eq!(jpeg.extension, "jpg");
        assert!(!jpeg.video);

        let mp4 = detect_media_signature(b"\0\0\0\x18ftypisom\0\0\0\0").unwrap();
        assert_eq!(mp4.mime_type, "video/mp4");
        assert_eq!(mp4.extension, "mp4");
        assert!(mp4.video);

        assert!(detect_media_signature(b"not-media").is_none());
    }

    #[test]
    fn pairing_attempts_are_bounded() {
        let ip: IpAddr = "203.0.113.77".parse().unwrap();
        clear_pair_attempts(ip);
        for _ in 0..PAIR_RATE_LIMIT_MAX {
            assert!(pair_attempt_allowed(ip, 1000));
        }
        assert!(!pair_attempt_allowed(ip, 1000));
        assert!(pair_attempt_allowed(ip, 1000 + PAIR_RATE_LIMIT_WINDOW_SECONDS));
        clear_pair_attempts(ip);
    }

}
