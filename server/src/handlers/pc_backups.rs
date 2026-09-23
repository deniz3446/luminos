use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, SocketAddr},
    path::{Component, Path, PathBuf},
    sync::{Mutex as StdMutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json,
    body::Bytes,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tokio::{
    fs,
    io::AsyncWriteExt,
    process::Command,
};
use uuid::Uuid;

use crate::{auth::decode_token, state::AppState};

const PC_PAIRING_TTL_SECONDS: i64 = 600;
const PC_ONLINE_SECONDS: i64 = 120;
const MAX_PC_CHUNK_BYTES: usize = 64 * 1024 * 1024;
const MAX_PC_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024 * 1024;
const SHARE_NAME: &str = "PhotoOS-PC-Backup";
const DEFAULT_ASSIGNMENTS_PATH: &str = "/var/lib/photoos/storage/assignments.json";
const PAIR_RATE_LIMIT_MAX: usize = 10;
const PAIR_RATE_LIMIT_WINDOW_SECONDS: i64 = 60;

static PAIR_ATTEMPTS: OnceLock<StdMutex<HashMap<IpAddr, VecDeque<i64>>>> = OnceLock::new();

#[derive(Debug)]
enum PcError {
    Unauthorized,
    Forbidden,
    RateLimited,
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    NotConfigured,
    Database,
    Io,
}

impl IntoResponse for PcError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "PC istemcisi veya PhotoOS oturumu doğrulanamadı.".to_string(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "Bu PC Backup işlemi yalnızca yöneticiler tarafından yapılabilir.".to_string(),
            ),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Çok fazla eşleştirme denemesi yapıldı. Kısa süre sonra tekrar deneyin.".to_string(),
            ),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "invalid_request", message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::NotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "pc_backup_not_configured",
                "PC Ana Yedek diski atanmamış.".to_string(),
            ),
            Self::Database => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                "PC Backup veritabanı işlemi başarısız.".to_string(),
            ),
            Self::Io => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                "PC Backup dosya işlemi başarısız.".to_string(),
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
pub struct PairDeviceRequest {
    pub code: String,
    #[serde(alias = "name", alias = "deviceName")]
    pub device_name: String,
}

#[derive(Debug, Deserialize)]
pub struct PcUploadQuery {
    pub path: String,
    pub modified_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PcChunkQuery {
    pub path: String,
    pub upload_id: String,
    pub offset: u64,
    pub complete: bool,
    pub modified_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PcBrowseQuery {
    pub disk: u32,
    pub computer: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone)]
struct PcDevice {
    id: String,
    name: String,
}

#[derive(Debug, Clone)]
struct PairingCode {
    code: String,
    expires_at: i64,
}

#[derive(Debug, Clone)]
struct PairResult {
    device_id: String,
    device_name: String,
    token: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AssignmentRecord {
    slot: u32,
    #[serde(default)]
    slot_name: String,
    role: String,
    mountpoint: String,
}

#[derive(Debug, Clone)]
struct PcDisk {
    index: u32,
    name: String,
    root: PathBuf,
    primary: bool,
}

#[derive(Debug, Serialize)]
struct PcClientSummary {
    id: String,
    name: String,
    status: String,
    last_seen: i64,
    files_uploaded: i64,
    bytes_uploaded: i64,
}

#[derive(Debug, Serialize)]
struct PcComputerSummary {
    disk_index: u32,
    name: String,
    modified_at: i64,
    size_bytes: u64,
}

#[derive(Debug, Serialize)]
struct PcDiskSummary {
    index: u32,
    name: String,
    root: String,
    computers: Vec<PcComputerSummary>,
}

#[derive(Debug, Serialize)]
struct PcLibraryResponse {
    ok: bool,
    configured: bool,
    share_name: String,
    computer_count: usize,
    disks: Vec<PcDiskSummary>,
    clients: Vec<PcClientSummary>,
}

#[derive(Debug, Serialize)]
struct PcBrowseEntry {
    path: String,
    name: String,
    directory: bool,
    size_bytes: u64,
}

#[derive(Debug)]
struct ChunkOutcome {
    complete: bool,
    received_bytes: u64,
    final_path: Option<PathBuf>,
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

fn random_token() -> String {
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

fn bearer_token(headers: &HeaderMap) -> Result<String, PcError> {
    let raw = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(PcError::Unauthorized)?;

    let token = raw.strip_prefix("Bearer ").unwrap_or(raw).trim();
    if token.is_empty() {
        return Err(PcError::Unauthorized);
    }
    Ok(token.to_string())
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

pub async fn initialize_pc_backup(db: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pc_backup_pairings(
            code TEXT PRIMARY KEY,
            expires_at INTEGER NOT NULL,
            created_by INTEGER NOT NULL,
            used_at INTEGER
        )
        "#,
    )
    .execute(db)
    .await
    .map_err(|_| "pc_backup_pairings oluşturulamadı".to_string())?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pc_backup_devices(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
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
    .map_err(|_| "pc_backup_devices oluşturulamadı".to_string())?;

    Ok(())
}

async fn photoos_admin_user_id(db: &SqlitePool, headers: &HeaderMap) -> Result<i64, PcError> {
    let token = bearer_token(headers)?;
    let claims = decode_token(&token).map_err(|_| PcError::Unauthorized)?;

    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT id, role FROM users WHERE id = ? LIMIT 1")
            .bind(claims.sub)
            .fetch_optional(db)
            .await
            .map_err(|_| PcError::Database)?;

    match row {
        Some((id, role)) if role == "admin" => Ok(id),
        Some(_) => Err(PcError::Forbidden),
        None => Err(PcError::Unauthorized),
    }
}

async fn authenticate_device(db: &SqlitePool, headers: &HeaderMap) -> Result<PcDevice, PcError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_token(&token);

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM pc_backup_devices WHERE token_hash = ? LIMIT 1",
    )
    .bind(token_hash)
    .fetch_optional(db)
    .await
    .map_err(|_| PcError::Database)?;

    row.map(|(id, name)| PcDevice { id, name })
        .ok_or(PcError::Unauthorized)
}

async fn create_pairing_for_user(db: &SqlitePool, user_id: i64) -> Result<PairingCode, PcError> {
    let expires_at = unix_now() + PC_PAIRING_TTL_SECONDS;

    for _ in 0..16 {
        let code = generate_pairing_code();
        let result = sqlx::query(
            "INSERT INTO pc_backup_pairings(code,expires_at,created_by,used_at) VALUES(?,?,?,NULL)",
        )
        .bind(&code)
        .bind(expires_at)
        .bind(user_id)
        .execute(db)
        .await;

        match result {
            Ok(_) => return Ok(PairingCode { code, expires_at }),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => continue,
            Err(_) => return Err(PcError::Database),
        }
    }

    Err(PcError::Conflict(
        "Yeni PC eşleştirme kodu üretilemedi. Tekrar deneyin.".to_string(),
    ))
}

fn validate_device_name(name: &str) -> Result<String, PcError> {
    let name = name.trim();
    if name.is_empty()
        || name.len() > 120
        || name.chars().any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':' | '\0'))
        || name == "."
        || name == ".."
        || name.eq_ignore_ascii_case(".photoos-parts")
    {
        return Err(PcError::BadRequest("Geçerli bir bilgisayar adı gereklidir.".to_string()));
    }
    Ok(name.to_string())
}

async fn pair_device(db: &SqlitePool, request: PairDeviceRequest) -> Result<PairResult, PcError> {
    let code = request.code.trim();
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PcError::BadRequest(
            "Eşleştirme kodu 6 rakam olmalıdır.".to_string(),
        ));
    }
    let device_name = validate_device_name(&request.device_name)?;
    let now = unix_now();

    let mut tx = db.begin().await.map_err(|_| PcError::Database)?;
    let created_by: Option<i64> = sqlx::query_scalar(
        "SELECT created_by FROM pc_backup_pairings WHERE code=? AND used_at IS NULL AND expires_at>=?",
    )
    .bind(code)
    .bind(now)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| PcError::Database)?;

    if created_by.is_none() {
        return Err(PcError::BadRequest(
            "Eşleştirme kodu geçersiz veya süresi dolmuş.".to_string(),
        ));
    }

    let device_id = Uuid::new_v4().to_string();
    let token = random_token();
    let token_hash = hash_token(&token);

    sqlx::query(
        r#"
        INSERT INTO pc_backup_devices(
            id,name,token_hash,created_at,last_seen,status,files_uploaded,bytes_uploaded
        )
        VALUES(?,?,?,?,?,'online',0,0)
        ON CONFLICT(name) DO UPDATE SET
            id=excluded.id,
            token_hash=excluded.token_hash,
            last_seen=excluded.last_seen,
            status='online'
        "#,
    )
    .bind(&device_id)
    .bind(&device_name)
    .bind(token_hash)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|_| PcError::Database)?;

    let consumed = sqlx::query(
        "UPDATE pc_backup_pairings SET used_at=? WHERE code=? AND used_at IS NULL",
    )
    .bind(now)
    .bind(code)
    .execute(&mut *tx)
    .await
    .map_err(|_| PcError::Database)?;

    if consumed.rows_affected() != 1 {
        return Err(PcError::Conflict(
            "Eşleştirme kodu başka bir istemci tarafından kullanıldı.".to_string(),
        ));
    }

    tx.commit().await.map_err(|_| PcError::Database)?;
    Ok(PairResult {
        device_id,
        device_name,
        token,
    })
}

fn assignments_path() -> PathBuf {
    std::env::var("PHOTOOS_STORAGE_ASSIGNMENTS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_ASSIGNMENTS_PATH))
}

fn parse_pc_disks(value: &Value) -> Result<Vec<PcDisk>, PcError> {
    let object = value.as_object().ok_or_else(|| {
        PcError::BadRequest("Storage assignment belgesi geçersiz.".to_string())
    })?;

    let mut disks = Vec::new();
    for record in object.values() {
        let assignment: AssignmentRecord = serde_json::from_value(record.clone())
            .map_err(|_| PcError::BadRequest("Storage assignment kaydı geçersiz.".to_string()))?;

        if !matches!(assignment.role.as_str(), "pc_primary" | "pc_backup") {
            continue;
        }

        let root = PathBuf::from(assignment.mountpoint.trim());
        if !root.is_absolute()
            || root.components().any(|part| matches!(part, Component::ParentDir))
        {
            continue;
        }

        let label = if assignment.role == "pc_primary" {
            format!("PC Ana Yedek {}", assignment.slot)
        } else {
            format!("PC İkincil Yedek {}", assignment.slot)
        };

        disks.push(PcDisk {
            index: assignment.slot,
            name: if assignment.slot_name.trim().is_empty() {
                label
            } else {
                format!("{label} ({})", assignment.slot_name)
            },
            root,
            primary: assignment.role == "pc_primary",
        });
    }

    disks.sort_by_key(|disk| (!disk.primary, disk.index));
    Ok(disks)
}

async fn load_pc_disks() -> Result<Vec<PcDisk>, PcError> {
    let raw = match fs::read_to_string(assignments_path()).await {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(PcError::Io),
    };
    let value: Value = serde_json::from_str(&raw).map_err(|_| PcError::Io)?;
    parse_pc_disks(&value)
}

fn validate_root_under_allowed(root: &Path, allowed: &Path) -> Result<(), PcError> {
    if !root.is_absolute()
        || !allowed.is_absolute()
        || !root.starts_with(allowed)
        || root.components().any(|part| matches!(part, Component::ParentDir))
    {
        return Err(PcError::NotConfigured);
    }

    let root_meta = std::fs::symlink_metadata(root).map_err(|_| PcError::NotConfigured)?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(PcError::NotConfigured);
    }

    let canonical_allowed = std::fs::canonicalize(allowed).map_err(|_| PcError::NotConfigured)?;
    let canonical_root = std::fs::canonicalize(root).map_err(|_| PcError::NotConfigured)?;
    if !canonical_root.starts_with(&canonical_allowed) {
        return Err(PcError::NotConfigured);
    }

    Ok(())
}

fn validate_production_root(root: &Path) -> Result<(), PcError> {
    validate_root_under_allowed(root, Path::new("/srv/photoos/disks"))
}

async fn selected_primary_disk() -> Result<PcDisk, PcError> {
    let disks = load_pc_disks().await?;
    let disk = disks
        .into_iter()
        .find(|disk| disk.primary)
        .ok_or(PcError::NotConfigured)?;
    validate_production_root(&disk.root)?;
    if !fs::metadata(&disk.root).await.map(|value| value.is_dir()).unwrap_or(false) {
        return Err(PcError::NotConfigured);
    }
    Ok(disk)
}

fn safe_relative_path(raw: &str) -> Result<PathBuf, PcError> {
    if raw.trim().is_empty() || raw.contains('\0') {
        return Err(PcError::BadRequest("Geçersiz dosya yolu.".to_string()));
    }
    let normalized = raw.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() {
        return Err(PcError::BadRequest("Geçersiz dosya yolu.".to_string()));
    }

    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                if part.is_empty()
                    || part == "."
                    || part == ".."
                    || part.chars().any(|ch| ch.is_control() || ch == ':')
                {
                    return Err(PcError::BadRequest("Geçersiz dosya yolu.".to_string()));
                }
                result.push(part.as_ref());
            }
            _ => return Err(PcError::BadRequest("Geçersiz dosya yolu.".to_string())),
        }
    }

    if result.as_os_str().is_empty() {
        return Err(PcError::BadRequest("Geçersiz dosya yolu.".to_string()));
    }
    Ok(result)
}

fn safe_browse_path(raw: &str) -> Result<PathBuf, PcError> {
    if raw.trim().is_empty() {
        return Ok(PathBuf::new());
    }
    safe_relative_path(raw)
}

fn validate_upload_id(value: &str) -> Result<String, PcError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(PcError::BadRequest("Geçersiz upload_id.".to_string()));
    }
    Ok(value.to_string())
}

fn ensure_no_symlink_ancestors(root: &Path, relative: &Path) -> Result<PathBuf, PcError> {
    let root_meta = std::fs::symlink_metadata(root).map_err(|_| PcError::Io)?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(PcError::Io);
    }

    let mut current = root.to_path_buf();
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    for component in parent.components() {
        let Component::Normal(part) = component else {
            return Err(PcError::BadRequest("Geçersiz dosya yolu.".to_string()));
        };
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(PcError::BadRequest("Symlink hedeflerine yazılamaz.".to_string()));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(PcError::BadRequest("Dosya yolu bir klasörle çakışıyor.".to_string()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err(PcError::Io),
        }
    }
    Ok(root.join(relative))
}

fn device_root(root: &Path, device_name: &str) -> Result<PathBuf, PcError> {
    let name = validate_device_name(device_name)?;
    let path = root.join(name);
    if let Ok(meta) = std::fs::symlink_metadata(&path) {
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(PcError::BadRequest("Bilgisayar klasörü güvenli değil.".to_string()));
        }
    }
    Ok(path)
}

async fn write_pc_chunk_to_root(
    root: &Path,
    device: &PcDevice,
    query: &PcChunkQuery,
    body: &[u8],
) -> Result<ChunkOutcome, PcError> {
    if body.len() > MAX_PC_CHUNK_BYTES {
        return Err(PcError::BadRequest("PC Backup chunk boyut sınırını aşıyor.".to_string()));
    }
    let end = query
        .offset
        .checked_add(body.len() as u64)
        .ok_or_else(|| PcError::BadRequest("Chunk offset taşması.".to_string()))?;
    if end > MAX_PC_FILE_BYTES {
        return Err(PcError::BadRequest("PC Backup dosya boyut sınırını aşıyor.".to_string()));
    }

    let relative = safe_relative_path(&query.path)?;
    let upload_id = validate_upload_id(&query.upload_id)?;
    let device_root = device_root(root, &device.name)?;

    fs::create_dir_all(&device_root).await.map_err(|_| PcError::Io)?;

    let parts_relative = PathBuf::from(".photoos-parts")
        .join(&device.id)
        .join("_probe");
    let _ = ensure_no_symlink_ancestors(root, &parts_relative)?;

    let parts_dir = root
        .join(".photoos-parts")
        .join(&device.id);
    fs::create_dir_all(&parts_dir).await.map_err(|_| PcError::Io)?;

    let parts_meta = fs::symlink_metadata(&parts_dir).await.map_err(|_| PcError::Io)?;
    if parts_meta.file_type().is_symlink() || !parts_meta.is_dir() {
        return Err(PcError::BadRequest(
            "Geçici upload klasörü güvenli değil.".to_string(),
        ));
    }

    let _ = ensure_no_symlink_ancestors(root, &parts_relative)?;
    let part = parts_dir.join(format!("{upload_id}.part"));

    if let Ok(meta) = fs::symlink_metadata(&part).await {
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(PcError::BadRequest("Geçici upload dosyası güvenli değil.".to_string()));
        }
    }

    if query.offset == 0 {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&part)
            .await
            .map_err(|_| PcError::Io)?;
        file.write_all(body).await.map_err(|_| PcError::Io)?;
        file.flush().await.map_err(|_| PcError::Io)?;
        file.sync_data().await.map_err(|_| PcError::Io)?;
    } else {
        let meta = fs::metadata(&part)
            .await
            .map_err(|_| PcError::Conflict("Chunk sırası eşleşmiyor.".to_string()))?;
        if meta.len() != query.offset {
            return Err(PcError::Conflict("Chunk offset değeri mevcut dosya boyutuyla eşleşmiyor.".to_string()));
        }
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&part)
            .await
            .map_err(|_| PcError::Io)?;
        file.write_all(body).await.map_err(|_| PcError::Io)?;
        file.flush().await.map_err(|_| PcError::Io)?;
        file.sync_data().await.map_err(|_| PcError::Io)?;
    }

    let received_bytes = fs::metadata(&part).await.map_err(|_| PcError::Io)?.len();
    if received_bytes != end {
        return Err(PcError::Conflict("Chunk uzunluğu beklenen offset ile eşleşmiyor.".to_string()));
    }

    if !query.complete {
        return Ok(ChunkOutcome {
            complete: false,
            received_bytes,
            final_path: None,
        });
    }

    let final_path = ensure_no_symlink_ancestors(&device_root, &relative)?;
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await.map_err(|_| PcError::Io)?;
    }

    let _ = ensure_no_symlink_ancestors(&device_root, &relative)?;

    if let Ok(meta) = fs::symlink_metadata(&final_path).await {
        if meta.file_type().is_symlink() || meta.is_dir() {
            return Err(PcError::BadRequest("Hedef dosya güvenli değil.".to_string()));
        }
    }

    fs::rename(&part, &final_path).await.map_err(|_| PcError::Io)?;

    if let Some(modified_at) = query.modified_at.filter(|value| *value > 0) {
        let timestamp = format!("@{modified_at}");
        let _ = Command::new("/usr/bin/touch")
            .args(["-m", "-d"])
            .arg(timestamp)
            .arg(&final_path)
            .status()
            .await;
    }

    Ok(ChunkOutcome {
        complete: true,
        received_bytes,
        final_path: Some(final_path),
    })
}

fn folder_stats(path: &Path) -> (u64, i64) {
    let mut size = 0_u64;
    let mut modified = 0_i64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let timestamp = meta
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_secs() as i64)
            .unwrap_or(0);
        modified = modified.max(timestamp);
        if meta.is_dir() {
            let (child_size, child_modified) = folder_stats(&entry.path());
            size = size.saturating_add(child_size);
            modified = modified.max(child_modified);
        } else if meta.is_file() {
            size = size.saturating_add(meta.len());
        }
    }
    (size, modified)
}

fn scan_computers(disk: &PcDisk) -> Vec<PcComputerSummary> {
    let mut computers = Vec::new();
    let Ok(entries) = std::fs::read_dir(&disk.root) else {
        return computers;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let (size_bytes, modified_at) = folder_stats(&entry.path());
        computers.push(PcComputerSummary {
            disk_index: disk.index,
            name,
            modified_at,
            size_bytes,
        });
    }
    computers.sort_by(|left, right| left.name.cmp(&right.name));
    computers
}

async fn list_clients(db: &SqlitePool) -> Result<Vec<PcClientSummary>, PcError> {
    let cutoff = unix_now().saturating_sub(PC_ONLINE_SECONDS);
    let rows = sqlx::query(
        "SELECT id,name,last_seen,files_uploaded,bytes_uploaded FROM pc_backup_devices ORDER BY last_seen DESC,name ASC",
    )
    .fetch_all(db)
    .await
    .map_err(|_| PcError::Database)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let last_seen: i64 = row.get("last_seen");
            PcClientSummary {
                id: row.get("id"),
                name: row.get("name"),
                status: if last_seen >= cutoff { "online" } else { "offline" }.to_string(),
                last_seen,
                files_uploaded: row.get("files_uploaded"),
                bytes_uploaded: row.get("bytes_uploaded"),
            }
        })
        .collect())
}

async fn update_transfer_counters(
    db: &SqlitePool,
    device_id: &str,
    bytes: u64,
) -> Result<(), PcError> {
    let result = sqlx::query(
        "UPDATE pc_backup_devices SET last_seen=?,status='online',files_uploaded=files_uploaded+1,bytes_uploaded=bytes_uploaded+? WHERE id=?",
    )
    .bind(unix_now())
    .bind(bytes as i64)
    .bind(device_id)
    .execute(db)
    .await
    .map_err(|_| PcError::Database)?;

    if result.rows_affected() != 1 {
        return Err(PcError::Unauthorized);
    }
    Ok(())
}

async fn record_completed_transfer(
    db: &SqlitePool,
    device_id: &str,
    bytes: u64,
    finalized_path: &Path,
) -> bool {
    match update_transfer_counters(db, device_id, bytes).await {
        Ok(()) => true,
        Err(_) => {
            eprintln!(
                "PC Backup accounting update failed after finalized file was preserved: {}",
                finalized_path.display()
            );
            false
        }
    }
}

pub async fn create_pc_pairing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if initialize_pc_backup(&state.db).await.is_err() {
        return PcError::Database.into_response();
    }
    let user_id = match photoos_admin_user_id(&state.db, &headers).await {
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
                "expires_in_seconds": PC_PAIRING_TTL_SECONDS,
            })),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn pair_pc_device(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<PairDeviceRequest>,
) -> Response {
    if initialize_pc_backup(&state.db).await.is_err() {
        return PcError::Database.into_response();
    }
    let ip = request_ip(peer, &headers);
    if !pair_attempt_allowed(ip, unix_now()) {
        return PcError::RateLimited.into_response();
    }
    match pair_device(&state.db, request).await {
        Ok(result) => {
            clear_pair_attempts(ip);
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "device_id": result.device_id,
                    "device_name": result.device_name,
                    "token": result.token,
                })),
            )
                .into_response()
        }
        Err(error) => error.into_response(),
    }
}

pub async fn pc_device_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if initialize_pc_backup(&state.db).await.is_err() {
        return PcError::Database.into_response();
    }
    let device = match authenticate_device(&state.db, &headers).await {
        Ok(device) => device,
        Err(error) => return error.into_response(),
    };
    let now = unix_now();
    let result = sqlx::query(
        "UPDATE pc_backup_devices SET last_seen=?,status='online' WHERE id=?",
    )
    .bind(now)
    .bind(&device.id)
    .execute(&state.db)
    .await;

    match result {
        Ok(value) if value.rows_affected() == 1 => Json(json!({
            "ok": true,
            "device_id": device.id,
            "device_name": device.name,
            "status": "online",
            "last_seen": now,
        }))
        .into_response(),
        Ok(_) => PcError::Unauthorized.into_response(),
        Err(_) => PcError::Database.into_response(),
    }
}

pub async fn upload_pc_chunk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PcChunkQuery>,
    body: Bytes,
) -> Response {
    if initialize_pc_backup(&state.db).await.is_err() {
        return PcError::Database.into_response();
    }
    let device = match authenticate_device(&state.db, &headers).await {
        Ok(device) => device,
        Err(error) => return error.into_response(),
    };
    let disk = match selected_primary_disk().await {
        Ok(disk) => disk,
        Err(error) => return error.into_response(),
    };

    match write_pc_chunk_to_root(&disk.root, &device, &query, &body).await {
        Ok(outcome) => {
            let accounting_synced = if outcome.complete {
                match outcome.final_path.as_ref() {
                    Some(path) => {
                        record_completed_transfer(
                            &state.db,
                            &device.id,
                            outcome.received_bytes,
                            path,
                        )
                        .await
                    }
                    None => false,
                }
            } else {
                true
            };

            Json(json!({
                "ok": true,
                "upload_id": query.upload_id,
                "path": query.path,
                "offset": outcome.received_bytes,
                "complete": outcome.complete,
                "accounting_synced": accounting_synced,
            }))
            .into_response()
        }
        Err(error) => error.into_response(),
    }
}

pub async fn upload_pc_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PcUploadQuery>,
    body: Bytes,
) -> Response {
    let chunk = PcChunkQuery {
        path: query.path.clone(),
        upload_id: Uuid::new_v4().to_string(),
        offset: 0,
        complete: true,
        modified_at: query.modified_at,
    };

    if initialize_pc_backup(&state.db).await.is_err() {
        return PcError::Database.into_response();
    }
    let device = match authenticate_device(&state.db, &headers).await {
        Ok(device) => device,
        Err(error) => return error.into_response(),
    };
    let disk = match selected_primary_disk().await {
        Ok(disk) => disk,
        Err(error) => return error.into_response(),
    };

    match write_pc_chunk_to_root(&disk.root, &device, &chunk, &body).await {
        Ok(outcome) => {
            let accounting_synced = match outcome.final_path.as_ref() {
                Some(path) => {
                    record_completed_transfer(
                        &state.db,
                        &device.id,
                        outcome.received_bytes,
                        path,
                    )
                    .await
                }
                None => false,
            };

            (StatusCode::CREATED, Json(json!({
                "ok": true,
                "path": query.path,
                "size_bytes": outcome.received_bytes,
                "complete": true,
                "accounting_synced": accounting_synced,
            }))).into_response()
        }
        Err(error) => error.into_response(),
    }
}

pub async fn get_pc_backup_library(
    State(state): State<AppState>,
    _headers: HeaderMap,
) -> Response {
    if initialize_pc_backup(&state.db).await.is_err() {
        return PcError::Database.into_response();
    }
    let assignments = match load_pc_disks().await {
        Ok(disks) => disks,
        Err(error) => return error.into_response(),
    };
    let configured = assignments.iter().any(|disk| {
        disk.primary
            && validate_production_root(&disk.root).is_ok()
            && disk.root.is_dir()
    });

    let clients = match list_clients(&state.db).await {
        Ok(clients) => clients,
        Err(error) => return error.into_response(),
    };

    let disks_for_scan: Vec<PcDisk> = assignments
        .into_iter()
        .filter(|disk| validate_production_root(&disk.root).is_ok() && disk.root.is_dir())
        .collect();

    let disks = tokio::task::spawn_blocking(move || {
        disks_for_scan
            .into_iter()
            .map(|disk| PcDiskSummary {
                index: disk.index,
                name: disk.name.clone(),
                root: disk.root.display().to_string(),
                computers: scan_computers(&disk),
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();

    let computer_count = disks.iter().map(|disk| disk.computers.len()).sum();

    Json(PcLibraryResponse {
        ok: true,
        configured,
        share_name: SHARE_NAME.to_string(),
        computer_count,
        disks,
        clients,
    })
    .into_response()
}

pub async fn browse_pc_backup(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    Query(query): Query<PcBrowseQuery>,
) -> Response {
    let disks = match load_pc_disks().await {
        Ok(disks) => disks,
        Err(error) => return error.into_response(),
    };
    let Some(disk) = disks.into_iter().find(|disk| disk.index == query.disk) else {
        return PcError::NotFound("PC Backup diski bulunamadı.".to_string()).into_response();
    };
    if validate_production_root(&disk.root).is_err() || !disk.root.is_dir() {
        return PcError::NotConfigured.into_response();
    }

    let computer = match validate_device_name(&query.computer) {
        Ok(name) => name,
        Err(error) => return error.into_response(),
    };
    let relative = match safe_browse_path(&query.path) {
        Ok(path) => path,
        Err(error) => return error.into_response(),
    };
    let computer_root = match device_root(&disk.root, &computer) {
        Ok(root) => root,
        Err(error) => return error.into_response(),
    };
    let browse_root = match ensure_no_symlink_ancestors(&computer_root, &relative.join("_probe")) {
        Ok(_) => computer_root.join(&relative),
        Err(error) => return error.into_response(),
    };

    let relative_prefix = relative.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<Vec<PcBrowseEntry>, PcError> {
        let meta = std::fs::symlink_metadata(&browse_root)
            .map_err(|_| PcError::NotFound("Klasör bulunamadı.".to_string()))?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(PcError::BadRequest("Geçersiz klasör.".to_string()));
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&browse_root).map_err(|_| PcError::Io)?.flatten() {
            let meta = match std::fs::symlink_metadata(entry.path()) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".photoos-parts" {
                continue;
            }
            let mut path = relative_prefix.clone();
            path.push(&name);
            entries.push(PcBrowseEntry {
                path: path.to_string_lossy().replace('\\', "/"),
                name,
                directory: meta.is_dir(),
                size_bytes: if meta.is_file() { meta.len() } else { 0 },
            });
        }
        entries.sort_by(|left, right| {
            right
                .directory
                .cmp(&left.directory)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(entries)
    })
    .await;

    match result {
        Ok(Ok(entries)) => Json(json!({ "ok": true, "entries": entries })).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(_) => PcError::Io.into_response(),
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
            "CREATE TABLE users(id INTEGER PRIMARY KEY, role TEXT NOT NULL)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO users(id,role) VALUES(1,'admin')")
            .execute(&db)
            .await
            .unwrap();
        initialize_pc_backup(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn pairing_is_one_time_and_only_hash_is_stored() {
        let db = test_db().await;
        let pairing = create_pairing_for_user(&db, 1).await.unwrap();
        assert_eq!(pairing.code.len(), 6);
        assert!(pairing.expires_at >= unix_now() + PC_PAIRING_TTL_SECONDS - 1);

        let paired = pair_device(
            &db,
            PairDeviceRequest {
                code: pairing.code.clone(),
                device_name: "PHOTOOS-PC".to_string(),
            },
        )
        .await
        .unwrap();

        let stored: String = sqlx::query_scalar(
            "SELECT token_hash FROM pc_backup_devices WHERE id=?",
        )
        .bind(&paired.device_id)
        .fetch_one(&db)
        .await
        .unwrap();

        assert_eq!(stored, hash_token(&paired.token));
        assert_ne!(stored, paired.token);

        let second = pair_device(
            &db,
            PairDeviceRequest {
                code: pairing.code,
                device_name: "OTHER-PC".to_string(),
            },
        )
        .await;
        assert!(second.is_err());
    }

    #[test]
    fn assignment_parser_prefers_primary_and_keeps_secondary() {
        let value = json!({
            "disk2": {
                "slot": 2,
                "slot_name": "disk2",
                "role": "pc_backup",
                "mountpoint": "/srv/photoos/disks/disk2"
            },
            "disk1": {
                "slot": 1,
                "slot_name": "disk1",
                "role": "pc_primary",
                "mountpoint": "/srv/photoos/disks/disk1"
            },
            "other": {
                "slot": 3,
                "slot_name": "disk3",
                "role": "phone_primary",
                "mountpoint": "/srv/photoos/disks/disk3"
            }
        });
        let disks = parse_pc_disks(&value).unwrap();
        assert_eq!(disks.len(), 2);
        assert!(disks[0].primary);
        assert_eq!(disks[0].index, 1);
        assert!(!disks[1].primary);
    }

    #[test]
    fn relative_paths_and_upload_ids_are_strict() {
        assert_eq!(
            safe_relative_path(r"Documents\PhotoOS\a.txt")
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
            "Documents/PhotoOS/a.txt"
        );
        for bad in ["../a", "/etc/passwd", r"C:\Windows\a", "a/../../b", ""] {
            assert!(safe_relative_path(bad).is_err(), "{bad}");
        }
        assert!(validate_upload_id("abc-123_DEF").is_ok());
        assert!(validate_upload_id("../bad").is_err());
    }

    #[tokio::test]
    async fn chunks_require_exact_offset_and_finalize_atomically() {
        let root = std::env::temp_dir().join(format!("photoos-pc-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).await.unwrap();

        let device = PcDevice {
            id: Uuid::new_v4().to_string(),
            name: "TEST-PC".to_string(),
        };
        let upload_id = Uuid::new_v4().to_string();

        let first = PcChunkQuery {
            path: "Docs/a.txt".to_string(),
            upload_id: upload_id.clone(),
            offset: 0,
            complete: false,
            modified_at: None,
        };
        let outcome = write_pc_chunk_to_root(&root, &device, &first, b"abc")
            .await
            .unwrap();
        assert!(!outcome.complete);
        assert_eq!(outcome.received_bytes, 3);

        let wrong = PcChunkQuery {
            path: "Docs/a.txt".to_string(),
            upload_id: upload_id.clone(),
            offset: 2,
            complete: false,
            modified_at: None,
        };
        assert!(write_pc_chunk_to_root(&root, &device, &wrong, b"x").await.is_err());

        let final_chunk = PcChunkQuery {
            path: "Docs/a.txt".to_string(),
            upload_id,
            offset: 3,
            complete: true,
            modified_at: None,
        };
        let outcome = write_pc_chunk_to_root(&root, &device, &final_chunk, b"def")
            .await
            .unwrap();
        assert!(outcome.complete);
        let final_path = outcome.final_path.unwrap();
        assert_eq!(fs::read(&final_path).await.unwrap(), b"abcdef");

        let _ = fs::remove_dir_all(&root).await;
    }

    #[test]
    fn symlink_ancestors_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("photoos-pc-symlink-test-{}", Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("photoos-pc-outside-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("escape")).unwrap();

        assert!(ensure_no_symlink_ancestors(&root, Path::new("escape/file.txt")).is_err());

        let _ = std::fs::remove_file(root.join("escape"));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn library_json_preserves_live_ui_contract() {
        let response = PcLibraryResponse {
            ok: true,
            configured: true,
            share_name: SHARE_NAME.to_string(),
            computer_count: 1,
            disks: vec![PcDiskSummary {
                index: 1,
                name: "PC Ana Yedek 1".to_string(),
                root: "/srv/photoos/disks/disk1".to_string(),
                computers: vec![PcComputerSummary {
                    disk_index: 1,
                    name: "TEST-PC".to_string(),
                    modified_at: 123,
                    size_bytes: 456,
                }],
            }],
            clients: vec![PcClientSummary {
                id: "device-1".to_string(),
                name: "TEST-PC".to_string(),
                status: "online".to_string(),
                last_seen: 123,
                files_uploaded: 2,
                bytes_uploaded: 456,
            }],
        };

        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["share_name"], SHARE_NAME);
        assert_eq!(value["computer_count"], 1);
        assert_eq!(value["disks"][0]["computers"][0]["disk_index"], 1);
        assert_eq!(value["clients"][0]["files_uploaded"], 2);
        assert_eq!(value["clients"][0]["last_seen"], 123);
    }

    #[test]
    fn online_window_and_pairing_ttl_match_live_contract() {
        assert_eq!(PC_PAIRING_TTL_SECONDS, 600);
        assert_eq!(PC_ONLINE_SECONDS, 120);
    }
    #[tokio::test]
    async fn accounting_failure_preserves_finalized_file() {
        let db = test_db().await;
        let root = std::env::temp_dir().join(format!("photoos-pc-accounting-green-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).await.unwrap();
        let finalized = root.join("finished.bin");
        fs::write(&finalized, b"backup-data").await.unwrap();

        let synced = record_completed_transfer(&db, "missing-device", 11, &finalized).await;
        assert!(!synced);
        assert_eq!(fs::read(&finalized).await.unwrap(), b"backup-data");

        let _ = fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn symlinked_internal_parts_directory_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("photoos-pc-parts-green-{}", Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("photoos-pc-parts-outside-green-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).await.unwrap();
        fs::create_dir_all(&outside).await.unwrap();
        symlink(&outside, root.join(".photoos-parts")).unwrap();

        let device = PcDevice {
            id: Uuid::new_v4().to_string(),
            name: "TEST-PC".to_string(),
        };
        let query = PcChunkQuery {
            path: "Docs/a.txt".to_string(),
            upload_id: Uuid::new_v4().to_string(),
            offset: 0,
            complete: false,
            modified_at: None,
        };

        let result = write_pc_chunk_to_root(&root, &device, &query, b"abc").await;
        assert!(result.is_err(), "symlinked .photoos-parts must be rejected");

        let _ = std::fs::remove_file(root.join(".photoos-parts"));
        let _ = fs::remove_dir_all(&root).await;
        let _ = fs::remove_dir_all(&outside).await;
    }

    #[test]
    fn assigned_root_must_be_real_directory_inside_allowed_root() {
        use std::os::unix::fs::symlink;

        let allowed = std::env::temp_dir().join(format!("photoos-pc-allowed-green-{}", Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("photoos-pc-root-outside-green-{}", Uuid::new_v4()));
        let real = allowed.join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        assert!(validate_root_under_allowed(&real, &allowed).is_ok());

        let linked = allowed.join("disk1");
        symlink(&outside, &linked).unwrap();
        assert!(validate_root_under_allowed(&linked, &allowed).is_err());

        let _ = std::fs::remove_file(&linked);
        let _ = std::fs::remove_dir_all(&allowed);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn internal_staging_namespace_is_reserved_from_device_names() {
        assert!(validate_device_name(".photoos-parts").is_err());
        assert!(validate_device_name(" .PHOTOOS-PARTS ").is_err());
        assert_eq!(validate_device_name("OFFICE-PC").unwrap(), "OFFICE-PC");
    }

}
