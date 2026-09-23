use std::{path::{Path, PathBuf}, process::Stdio, sync::{Arc, OnceLock}, time::{SystemTime, UNIX_EPOCH}};

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, sync::Mutex};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct BackupStatus {
    pub running: bool,
    pub job_id: Option<i64>,
    pub source: String,
    pub target: String,
    pub phase: String,
    pub progress_percent: f64,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_second: f64,
    pub eta_seconds: Option<u64>,
    pub files_transferred: u64,
    pub message: String,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub pid: Option<u32>,
}

impl Default for BackupStatus {
    fn default() -> Self {
        Self {
            running: false,
            job_id: None,
            source: String::new(),
            target: String::new(),
            phase: "idle".to_string(),
            progress_percent: 0.0,
            transferred_bytes: 0,
            total_bytes: 0,
            speed_bytes_per_second: 0.0,
            eta_seconds: None,
            files_transferred: 0,
            message: "Yedekleme beklemede.".to_string(),
            started_at: None,
            finished_at: None,
            pid: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StartBackupRequest {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub delete_extraneous: bool,
}

#[derive(Debug, Serialize)]
pub struct ApiMessage {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BackupTarget {
    pub name: String,
    pub path: String,
    pub available: bool,
    pub kind: String,
}

#[derive(Debug, Serialize)]
pub struct BackupTargetsResponse {
    pub sources: Vec<BackupTarget>,
    pub targets: Vec<BackupTarget>,
}

#[derive(Debug, Serialize)]
pub struct BackupHistoryEntry {
    pub id: i64,
    pub source: String,
    pub target: String,
    pub status: String,
    pub transferred_bytes: i64,
    pub total_bytes: i64,
    pub files_transferred: i64,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BackupHistoryResponse {
    pub history: Vec<BackupHistoryEntry>,
}


fn backup_target_for_disk(name: &str, disk_path: &str, available: bool) -> BackupTarget {
    let disk_path = disk_path.trim_end_matches('/');

    BackupTarget {
        name: format!("{name} Yedekleri"),
        path: format!("{disk_path}/backups/photoos"),
        available,
        kind: "backup_directory".to_string(),
    }
}

async fn is_exact_mountpoint(path: &Path) -> bool {
    Command::new("findmnt")
        .args(["-n", "--mountpoint"])
        .arg(path)
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

fn photoos_disk_root(path: &Path) -> Option<PathBuf> {
    let allowed_root = Path::new("/srv/photoos/disks");
    let relative = path.strip_prefix(allowed_root).ok()?;
    let first = relative.components().next()?;

    match first {
        std::path::Component::Normal(name) => Some(allowed_root.join(name)),
        _ => None,
    }
}

async fn ensure_mounted_photoos_path(path: &Path) -> Result<(), String> {
    let disk_root = photoos_disk_root(path)
        .ok_or_else(|| "Yol bir PhotoOS veri diskinin altında olmalıdır.".to_string())?;

    if !is_exact_mountpoint(&disk_root).await {
        return Err(format!(
            "PhotoOS veri diski bağlı değil: {}",
            disk_root.display()
        ));
    }

    Ok(())
}

type SharedRuntime = Arc<Mutex<BackupStatus>>;
static RUNTIME: OnceLock<SharedRuntime> = OnceLock::new();

fn runtime() -> SharedRuntime {
    RUNTIME
        .get_or_init(|| Arc::new(Mutex::new(BackupStatus::default())))
        .clone()
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub async fn initialize_backup_manager(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS backup_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            status TEXT NOT NULL,
            transferred_bytes INTEGER NOT NULL DEFAULT 0,
            total_bytes INTEGER NOT NULL DEFAULT 0,
            files_transferred INTEGER NOT NULL DEFAULT 0,
            started_at INTEGER NOT NULL,
            finished_at INTEGER,
            message TEXT NOT NULL DEFAULT ''
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_backup_jobs_started_at ON backup_jobs(started_at DESC)")
        .execute(db)
        .await?;

    // Sunucu yeniden başladıysa yarım kalmış işleri kapat.
    sqlx::query(
        "UPDATE backup_jobs SET status='interrupted', finished_at=?, message='Sunucu yeniden başlatıldığı için işlem kesildi.' WHERE status='running'",
    )
    .bind(now_ts())
    .execute(db)
    .await?;

    Ok(())
}

fn normalize_allowed_path(value: &str) -> Result<PathBuf, String> {
    let raw = PathBuf::from(value.trim());
    if !raw.is_absolute() {
        return Err("Yol mutlak olmalıdır.".to_string());
    }

    if raw.components().any(|part| matches!(part, std::path::Component::ParentDir)) {
        return Err("Yolda '..' kullanılamaz.".to_string());
    }

    let allowed_root = Path::new("/srv/photoos/disks");
    if !raw.starts_with(allowed_root) {
        return Err("Yalnızca /srv/photoos/disks altındaki yollar kullanılabilir.".to_string());
    }

    Ok(raw)
}

fn parse_human_speed(value: &str) -> f64 {
    let clean = value.trim().replace(',', ".");
    let split_at = clean.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(clean.len());
    let number = clean[..split_at].parse::<f64>().unwrap_or(0.0);
    let unit = clean[split_at..].to_ascii_lowercase();
    if unit.starts_with("gb") { number * 1024.0 * 1024.0 * 1024.0 }
    else if unit.starts_with("mb") { number * 1024.0 * 1024.0 }
    else if unit.starts_with("kb") { number * 1024.0 }
    else { number }
}

fn parse_progress_line(line: &str, status: &mut BackupStatus) {
    let normalized = line.replace('\r', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if let Some(percent_index) = parts.iter().position(|part| part.ends_with('%')) {
        if let Ok(percent) = parts[percent_index].trim_end_matches('%').parse::<f64>() {
            status.progress_percent = percent.clamp(0.0, 100.0);
        }
        if percent_index > 0 {
            let bytes_text = parts[percent_index - 1].replace([',', '.'], "");
            if let Ok(bytes) = bytes_text.parse::<u64>() {
                status.transferred_bytes = bytes;
                if status.progress_percent > 0.0 {
                    status.total_bytes = ((bytes as f64) * 100.0 / status.progress_percent) as u64;
                }
            }
        }
        if let Some(speed) = parts.get(percent_index + 1) {
            status.speed_bytes_per_second = parse_human_speed(speed);
        }
        if let Some(eta) = parts.get(percent_index + 2) {
            let time_parts: Vec<u64> = eta.split(':').filter_map(|v| v.parse().ok()).collect();
            status.eta_seconds = match time_parts.as_slice() {
                [h, m, s] => Some(h * 3600 + m * 60 + s),
                [m, s] => Some(m * 60 + s),
                _ => None,
            };
        }
        status.phase = "copying".to_string();
        status.message = "Dosyalar aktarılıyor.".to_string();
    }
    if let Some(index) = normalized.find("xfr#") {
        let digits: String = normalized[index + 4..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(count) = digits.parse::<u64>() {
            status.files_transferred = count;
        }
    }
}

pub async fn get_backup_status() -> Json<BackupStatus> {
    Json(runtime().lock().await.clone())
}

pub async fn get_backup_targets() -> Json<BackupTargetsResponse> {
    let disks = [
        ("PHOTOOS_DATA1", "/srv/photoos/disks/disk1"),
        ("PHOTOOS_DATA2", "/srv/photoos/disks/disk2"),
    ];

    let mut sources = Vec::with_capacity(disks.len());
    let mut targets = Vec::with_capacity(disks.len());

    for (name, path) in disks {
        let available = is_exact_mountpoint(Path::new(path)).await;

        sources.push(BackupTarget {
            name: name.to_string(),
            path: path.to_string(),
            available,
            kind: "disk".to_string(),
        });

        targets.push(backup_target_for_disk(name, path, available));
    }

    Json(BackupTargetsResponse { sources, targets })
}

pub async fn get_backup_history(State(state): State<AppState>) -> Result<Json<BackupHistoryResponse>, (StatusCode, Json<ApiMessage>)> {
    let rows = sqlx::query(
        "SELECT id, source, target, status, transferred_bytes, total_bytes, files_transferred, started_at, finished_at, message FROM backup_jobs ORDER BY id DESC LIMIT 50",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiMessage { success: false, message: error.to_string() })))?;

    let history = rows.into_iter().map(|row| BackupHistoryEntry {
        id: row.get("id"), source: row.get("source"), target: row.get("target"), status: row.get("status"),
        transferred_bytes: row.get("transferred_bytes"), total_bytes: row.get("total_bytes"), files_transferred: row.get("files_transferred"),
        started_at: row.get("started_at"), finished_at: row.get("finished_at"), message: row.get("message"),
    }).collect();

    Ok(Json(BackupHistoryResponse { history }))
}

pub async fn start_backup(
    State(state): State<AppState>,
    Json(request): Json<StartBackupRequest>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let source = normalize_allowed_path(&request.source).map_err(|message| (StatusCode::BAD_REQUEST, Json(ApiMessage { success: false, message })))?;
    let target = normalize_allowed_path(&request.target).map_err(|message| (StatusCode::BAD_REQUEST, Json(ApiMessage { success: false, message })))?;

    ensure_mounted_photoos_path(&source)
        .await
        .map_err(|message| (StatusCode::BAD_REQUEST, Json(ApiMessage { success: false, message })))?;
    ensure_mounted_photoos_path(&target)
        .await
        .map_err(|message| (StatusCode::BAD_REQUEST, Json(ApiMessage { success: false, message })))?;

    if !source.is_dir() {
        return Err((StatusCode::BAD_REQUEST, Json(ApiMessage { success: false, message: "Kaynak klasör bulunamadı.".to_string() })));
    }
    if source == target || target.starts_with(&source) {
        return Err((StatusCode::BAD_REQUEST, Json(ApiMessage { success: false, message: "Hedef, kaynakla aynı veya kaynağın altında olamaz.".to_string() })));
    }

    {
        let current = runtime();
        let guard = current.lock().await;
        if guard.running {
            return Err((StatusCode::CONFLICT, Json(ApiMessage { success: false, message: "Başka bir yedekleme zaten çalışıyor.".to_string() })));
        }
    }

    tokio::fs::create_dir_all(&target).await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiMessage { success: false, message: format!("Hedef oluşturulamadı: {error}") })))?;

    let started_at = now_ts();
    let result = sqlx::query("INSERT INTO backup_jobs(source,target,status,started_at,message) VALUES(?,?,'running',?,'Yedekleme başlatıldı.')")
        .bind(source.to_string_lossy().to_string())
        .bind(target.to_string_lossy().to_string())
        .bind(started_at)
        .execute(&state.db).await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiMessage { success: false, message: error.to_string() })))?;
    let job_id = result.last_insert_rowid();

    let shared = runtime();
    {
        let mut guard = shared.lock().await;
        *guard = BackupStatus { running: true, job_id: Some(job_id), source: source.to_string_lossy().to_string(), target: target.to_string_lossy().to_string(), phase: "starting".to_string(), message: "rsync başlatılıyor.".to_string(), started_at: Some(started_at), ..BackupStatus::default() };
    }

    let db = state.db.clone();
    tokio::spawn(async move {
        let mut command = Command::new("rsync");
        command.arg("-aH").arg("--numeric-ids").arg("--info=progress2").arg("--human-readable");
        if request.delete_extraneous { command.arg("--delete"); }
        command.arg(format!("{}/", source.to_string_lossy().trim_end_matches('/')))
            .arg(format!("{}/", target.to_string_lossy().trim_end_matches('/')))
            .stdout(Stdio::piped()).stderr(Stdio::piped());

        match command.spawn() {
            Ok(mut child) => {
                {
                    let mut guard = shared.lock().await;
                    guard.pid = child.id();
                    guard.phase = "scanning".to_string();
                    guard.message = "Dosyalar taranıyor.".to_string();
                }
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();
                let shared_out = shared.clone();
                let output_task = tokio::spawn(async move {
                    if let Some(output) = stdout {
                        let mut reader = BufReader::new(output).lines();
                        while let Ok(Some(line)) = reader.next_line().await {
                            let mut guard = shared_out.lock().await;
                            parse_progress_line(&line, &mut guard);
                        }
                    }
                });
                let error_task = tokio::spawn(async move {
                    let mut last = String::new();
                    if let Some(output) = stderr {
                        let mut reader = BufReader::new(output).lines();
                        while let Ok(Some(line)) = reader.next_line().await { if !line.trim().is_empty() { last = line; } }
                    }
                    last
                });
                let exit = child.wait().await;
                let _ = output_task.await;
                let stderr_text = error_task.await.unwrap_or_default();
                let mut guard = shared.lock().await;
                let stopped = guard.phase == "stopping";
                let success = exit.as_ref().map(|s| s.success()).unwrap_or(false) && !stopped;
                guard.running = false;
                guard.pid = None;
                guard.finished_at = Some(now_ts());
                guard.phase = if stopped { "stopped" } else if success { "completed" } else { "failed" }.to_string();
                if success { guard.progress_percent = 100.0; guard.message = "Yedekleme başarıyla tamamlandı.".to_string(); }
                else if stopped { guard.message = "Yedekleme kullanıcı tarafından durduruldu.".to_string(); }
                else { guard.message = if stderr_text.is_empty() { "Yedekleme başarısız oldu.".to_string() } else { stderr_text }; }
                let snapshot = guard.clone();
                drop(guard);
                let _ = sqlx::query("UPDATE backup_jobs SET status=?, transferred_bytes=?, total_bytes=?, files_transferred=?, finished_at=?, message=? WHERE id=?")
                    .bind(&snapshot.phase).bind(snapshot.transferred_bytes as i64).bind(snapshot.total_bytes as i64).bind(snapshot.files_transferred as i64)
                    .bind(snapshot.finished_at).bind(&snapshot.message).bind(job_id).execute(&db).await;
            }
            Err(error) => {
                let mut guard = shared.lock().await;
                guard.running = false; guard.phase = "failed".to_string(); guard.finished_at = Some(now_ts());
                guard.message = format!("rsync başlatılamadı: {error}");
                let message = guard.message.clone(); drop(guard);
                let _ = sqlx::query("UPDATE backup_jobs SET status='failed', finished_at=?, message=? WHERE id=?")
                    .bind(now_ts()).bind(message).bind(job_id).execute(&db).await;
            }
        }
    });

    Ok((StatusCode::ACCEPTED, Json(ApiMessage { success: true, message: "Yedekleme başlatıldı.".to_string() })))
}

pub async fn stop_backup() -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let shared = runtime();
    let pid = {
        let mut guard = shared.lock().await;
        if !guard.running {
            return Err((StatusCode::CONFLICT, Json(ApiMessage { success: false, message: "Çalışan yedekleme yok.".to_string() })));
        }
        guard.phase = "stopping".to_string();
        guard.message = "Yedekleme durduruluyor.".to_string();
        guard.pid
    };

    if let Some(pid) = pid {
        let status = Command::new("kill").arg("-TERM").arg(pid.to_string()).status().await;
        if status.as_ref().map(|s| s.success()).unwrap_or(false) {
            return Ok(Json(ApiMessage { success: true, message: "Durdurma sinyali gönderildi.".to_string() }));
        }
    }

    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiMessage { success: false, message: "Yedekleme işlemi durdurulamadı.".to_string() })))
}



#[cfg(test)]
mod wave2_tests {
    use super::*;

    #[test]
    fn backup_target_can_be_selected_before_destination_folder_exists() {
        let root = "/srv/photoos/disks/disk2";
        let target = backup_target_for_disk("TEST", root, true);

        assert!(target.available);
        assert_eq!(target.path, "/srv/photoos/disks/disk2/backups/photoos");
    }

    #[test]
    fn photoos_disk_root_resolves_only_a_disk_child() {
        assert_eq!(
            photoos_disk_root(Path::new("/srv/photoos/disks/disk2/backups/photoos")),
            Some(PathBuf::from("/srv/photoos/disks/disk2"))
        );
        assert_eq!(
            photoos_disk_root(Path::new("/srv/photoos/disks")),
            None
        );
        assert_eq!(
            photoos_disk_root(Path::new("/tmp/not-photoos")),
            None
        );
    }
}
