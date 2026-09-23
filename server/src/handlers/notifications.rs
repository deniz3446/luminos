use axum::{
    Json,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs,
    time::{Duration, MissedTickBehavior, interval},
};

use crate::state::AppState;

const DEFAULT_AGGREGATE_PATH: &str =
    "/var/lib/photoos/runtime/notification-producers/notifications-aggregate.json";

static NOTIFICATION_AGGREGATE_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationItem {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub producer: String,

    #[serde(default)]
    pub producer_version: String,

    #[serde(default)]
    pub status: String,

    #[serde(default)]
    pub level: String,

    #[serde(default)]
    pub code: String,

    #[serde(default)]
    pub title: String,

    #[serde(default)]
    pub message: String,

    #[serde(default)]
    pub error: String,

    #[serde(default)]
    pub generated_at: Option<i64>,

    #[serde(default)]
    pub age_seconds: Option<i64>,

    #[serde(default)]
    pub fresh: bool,

    #[serde(default)]
    pub state_file: String,

    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationCounts {
    #[serde(default)]
    pub total: usize,

    #[serde(default)]
    pub active: usize,

    #[serde(default)]
    pub success: usize,

    #[serde(default)]
    pub info: usize,

    #[serde(default)]
    pub warning: usize,

    #[serde(default)]
    pub error: usize,

    #[serde(default)]
    pub critical: usize,

    #[serde(default)]
    pub fresh: usize,

    #[serde(default)]
    pub stale: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationOverall {
    #[serde(default)]
    pub status: String,

    #[serde(default)]
    pub level: String,

    #[serde(default)]
    pub code: String,

    #[serde(default)]
    pub title: String,

    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAggregate {
    #[serde(default)]
    pub schema_version: u32,

    #[serde(default)]
    pub producer_name: String,

    #[serde(default)]
    pub producer_version: String,

    #[serde(default)]
    pub generated_at: i64,

    #[serde(default)]
    pub overall: NotificationOverall,

    #[serde(default)]
    pub counts: NotificationCounts,

    #[serde(default)]
    pub notifications: Vec<NotificationItem>,

    #[serde(default)]
    pub problems: Vec<NotificationItem>,

    #[serde(default)]
    pub information: Vec<NotificationItem>,

    #[serde(default)]
    pub healthy: Vec<NotificationItem>,
}

#[derive(Debug)]
pub enum NotificationApiError {
    FileNotFound(PathBuf),
    ReadFailed(String),
    InvalidJson(String),
}

impl IntoResponse for NotificationApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::FileNotFound(path) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "notification_aggregate_missing",
                format!(
                    "Notification aggregate dosyası bulunamadı: {}",
                    path.display()
                ),
            ),

            Self::ReadFailed(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "notification_aggregate_read_failed",
                format!("Notification aggregate okunamadı: {error}"),
            ),

            Self::InvalidJson(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "notification_aggregate_invalid_json",
                format!("Notification aggregate JSON geçersiz: {error}"),
            ),
        };

        (
            status,
            Json(json!({
                "ok": false,
                "error": {
                    "code": code,
                    "message": message,
                }
            })),
        )
            .into_response()
    }
}

fn aggregate_path() -> PathBuf {
    if let Some(path) = NOTIFICATION_AGGREGATE_PATH.get() {
        return path.clone();
    }

    std::env::var("PHOTOOS_NOTIFICATION_AGGREGATE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_AGGREGATE_PATH))
}

// PHOTOOS_NOTIFICATION_RUNTIME_6_0B

#[derive(Debug, Clone, Serialize)]
struct NotificationProducerFile {
    name: String,
    path: String,
    size_bytes: u64,
    modified_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct NotificationRuntimeStatus {
    producer_directory: String,
    aggregate_path: String,
    aggregate_exists: bool,
    aggregate_size_bytes: u64,
    aggregate_modified_at: Option<i64>,
    aggregate_age_seconds: Option<i64>,
    producer_file_count: usize,
}

fn notification_producer_directory() -> PathBuf {
    aggregate_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/var/lib/photoos/runtime/notification-producers"))
}

fn metadata_modified_unix(metadata: &std::fs::Metadata) -> Option<i64> {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
}

async fn notification_producer_inventory() -> Vec<NotificationProducerFile> {
    let directory = notification_producer_directory();
    let mut files = Vec::new();

    let mut entries = match fs::read_dir(&directory).await {
        Ok(entries) => entries,
        Err(_) => return files,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();

        if name == "notifications-aggregate.json"
            || name.ends_with(".tmp")
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }

        let metadata = match entry.metadata().await {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        files.push(NotificationProducerFile {
            name,
            path: path.display().to_string(),
            size_bytes: metadata.len(),
            modified_at: metadata_modified_unix(&metadata),
        });
    }

    files.sort_by(|left, right| left.name.cmp(&right.name));

    files
}

async fn notification_runtime_status() -> NotificationRuntimeStatus {
    let directory = notification_producer_directory();
    let aggregate = aggregate_path();
    let producer_files = notification_producer_inventory().await;

    let metadata = fs::metadata(&aggregate).await.ok();

    let aggregate_modified_at = metadata.as_ref().and_then(metadata_modified_unix);

    let aggregate_age_seconds =
        aggregate_modified_at.map(|modified| unix_now().saturating_sub(modified));

    NotificationRuntimeStatus {
        producer_directory: directory.display().to_string(),
        aggregate_path: aggregate.display().to_string(),
        aggregate_exists: metadata.is_some(),
        aggregate_size_bytes: metadata.as_ref().map(|value| value.len()).unwrap_or(0),
        aggregate_modified_at,
        aggregate_age_seconds,
        producer_file_count: producer_files.len(),
    }
}

// PHOTOOS_NOTIFICATION_RUNTIME_6_0A

const NOTIFICATION_RUNTIME_INTERVAL_SECONDS: u64 = 15;
const NOTIFICATION_FRESH_SECONDS: i64 = 120;

fn notification_level_rank(level: &str) -> u8 {
    match level.trim().to_ascii_lowercase().as_str() {
        "critical" => 5,
        "error" | "danger" => 4,
        "warning" | "warn" => 3,
        "info" | "information" => 2,
        "success" | "healthy" | "ok" => 1,
        _ => 0,
    }
}

fn notification_level_active(level: &str) -> bool {
    matches!(
        level.trim().to_ascii_lowercase().as_str(),
        "warning" | "warn" | "error" | "danger" | "critical"
    )
}

fn normalize_notification_item(
    mut item: NotificationItem,
    state_file: &Path,
    now: i64,
) -> NotificationItem {
    if item.id.trim().is_empty() {
        item.id = format!("{}:{}", item.producer.trim(), item.code.trim());
    }

    if item.producer.trim().is_empty() {
        item.producer = state_file
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("unknown")
            .to_string();
    }

    if item.producer_version.trim().is_empty() {
        item.producer_version = "1.0".to_string();
    }

    if item.level.trim().is_empty() {
        item.level = if item.active {
            "warning".to_string()
        } else {
            "info".to_string()
        };
    }

    if item.status.trim().is_empty() {
        item.status = if item.active {
            "problem".to_string()
        } else {
            "healthy".to_string()
        };
    }

    if item.generated_at.is_none() {
        item.generated_at = Some(now);
    }

    let generated_at = item.generated_at.unwrap_or(now);
    let age_seconds = now.saturating_sub(generated_at);

    item.age_seconds = Some(age_seconds);
    item.fresh = age_seconds <= NOTIFICATION_FRESH_SECONDS;
    item.state_file = state_file.display().to_string();

    if !item.active {
        item.active = notification_level_active(&item.level)
            || matches!(
                item.status.trim().to_ascii_lowercase().as_str(),
                "problem" | "warning" | "error" | "critical" | "degraded" | "failed"
            );
    }

    item
}

fn notifications_from_value(value: Value) -> Vec<NotificationItem> {
    if let Ok(item) = serde_json::from_value::<NotificationItem>(value.clone()) {
        if !item.producer.trim().is_empty()
            || !item.code.trim().is_empty()
            || !item.title.trim().is_empty()
        {
            return vec![item];
        }
    }

    if let Some(array) = value.as_array() {
        return array
            .iter()
            .filter_map(|item| serde_json::from_value::<NotificationItem>(item.clone()).ok())
            .collect();
    }

    for key in [
        "notifications",
        "problems",
        "information",
        "healthy",
        "items",
    ] {
        if let Some(array) = value.get(key).and_then(Value::as_array) {
            let items = array
                .iter()
                .filter_map(|item| serde_json::from_value::<NotificationItem>(item.clone()).ok())
                .collect::<Vec<_>>();

            if !items.is_empty() {
                return items;
            }
        }
    }

    Vec::new()
}

async fn read_notification_producer_items(directory: &Path) -> Vec<NotificationItem> {
    let mut notifications = Vec::new();
    let now = unix_now();

    let mut entries = match fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(_) => return notifications,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        if file_name == "notifications-aggregate.json"
            || file_name.ends_with(".tmp")
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }

        let content = match fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(error) => {
                eprintln!(
                    "Notification producer okunamadı: {}: {}",
                    path.display(),
                    error
                );
                continue;
            }
        };

        let value = match serde_json::from_str::<Value>(&content) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "Notification producer JSON geçersiz: {}: {}",
                    path.display(),
                    error
                );
                continue;
            }
        };

        notifications.extend(
            notifications_from_value(value)
                .into_iter()
                .map(|item| normalize_notification_item(item, &path, now)),
        );
    }

    notifications.sort_by(|left, right| {
        notification_level_rank(&right.level)
            .cmp(&notification_level_rank(&left.level))
            .then_with(|| {
                right
                    .generated_at
                    .unwrap_or_default()
                    .cmp(&left.generated_at.unwrap_or_default())
            })
    });

    notifications
}

fn build_notification_aggregate(notifications: Vec<NotificationItem>) -> NotificationAggregate {
    let now = unix_now();

    let active = notifications.iter().filter(|item| item.active).count();

    let success = notifications
        .iter()
        .filter(|item| {
            matches!(
                item.level.trim().to_ascii_lowercase().as_str(),
                "success" | "healthy" | "ok"
            )
        })
        .count();

    let info = notifications
        .iter()
        .filter(|item| {
            matches!(
                item.level.trim().to_ascii_lowercase().as_str(),
                "info" | "information"
            )
        })
        .count();

    let warning = notifications
        .iter()
        .filter(|item| {
            matches!(
                item.level.trim().to_ascii_lowercase().as_str(),
                "warning" | "warn"
            )
        })
        .count();

    let error = notifications
        .iter()
        .filter(|item| {
            matches!(
                item.level.trim().to_ascii_lowercase().as_str(),
                "error" | "danger"
            )
        })
        .count();

    let critical = notifications
        .iter()
        .filter(|item| item.level.eq_ignore_ascii_case("critical"))
        .count();

    let fresh = notifications.iter().filter(|item| item.fresh).count();

    let stale = notifications.len().saturating_sub(fresh);

    let problems = notifications
        .iter()
        .filter(|item| item.active)
        .cloned()
        .collect::<Vec<_>>();

    let information = notifications
        .iter()
        .filter(|item| {
            !item.active
                && matches!(
                    item.level.trim().to_ascii_lowercase().as_str(),
                    "info" | "information"
                )
        })
        .cloned()
        .collect::<Vec<_>>();

    let healthy = notifications
        .iter()
        .filter(|item| !item.active)
        .cloned()
        .collect::<Vec<_>>();

    let overall = if critical > 0 {
        NotificationOverall {
            status: "critical".to_string(),
            level: "critical".to_string(),
            code: "notification_critical".to_string(),
            title: "Kritik sistem uyarıları".to_string(),
            message: format!("{critical} kritik bildirim aktif."),
        }
    } else if error > 0 {
        NotificationOverall {
            status: "error".to_string(),
            level: "error".to_string(),
            code: "notification_error".to_string(),
            title: "Sistem hataları".to_string(),
            message: format!("{error} hata bildirimi aktif."),
        }
    } else if warning > 0 {
        NotificationOverall {
            status: "warning".to_string(),
            level: "warning".to_string(),
            code: "notification_warning".to_string(),
            title: "Sistem uyarıları".to_string(),
            message: format!("{warning} uyarı bildirimi aktif."),
        }
    } else {
        NotificationOverall {
            status: "healthy".to_string(),
            level: "success".to_string(),
            code: "notification_healthy".to_string(),
            title: "Sistem bildirimleri normal".to_string(),
            message: if notifications.is_empty() {
                "Aktif bildirim üreticisi bulunmuyor.".to_string()
            } else {
                "Aktif bir sistem problemi bulunmuyor.".to_string()
            },
        }
    };

    NotificationAggregate {
        schema_version: 1,
        producer_name: "photoos-notification-runtime".to_string(),
        producer_version: "6.0a".to_string(),
        generated_at: now,
        overall,
        counts: NotificationCounts {
            total: notifications.len(),
            active,
            success,
            info,
            warning,
            error,
            critical,
            fresh,
            stale,
        },
        notifications,
        problems,
        information,
        healthy,
    }
}

async fn write_notification_aggregate(aggregate: &NotificationAggregate) -> Result<(), String> {
    let target = aggregate_path();

    let directory = target
        .parent()
        .ok_or_else(|| "Notification aggregate üst klasörü yok.".to_string())?;

    fs::create_dir_all(directory)
        .await
        .map_err(|error| format!("Notification runtime klasörü oluşturulamadı: {error}"))?;

    let temporary = target.with_extension("json.tmp");

    let content = serde_json::to_vec_pretty(aggregate)
        .map_err(|error| format!("Notification aggregate JSON oluşturulamadı: {error}"))?;

    fs::write(&temporary, content)
        .await
        .map_err(|error| format!("Notification aggregate geçici dosyası yazılamadı: {error}"))?;

    fs::rename(&temporary, &target)
        .await
        .map_err(|error| format!("Notification aggregate aktifleştirilemedi: {error}"))?;

    Ok(())
}

async fn refresh_notification_aggregate() -> Result<(), String> {
    let target = aggregate_path();

    let directory = target
        .parent()
        .ok_or_else(|| "Notification producer klasörü çözümlenemedi.".to_string())?;

    fs::create_dir_all(directory)
        .await
        .map_err(|error| format!("Notification producer klasörü oluşturulamadı: {error}"))?;

    let items = read_notification_producer_items(directory).await;

    let aggregate = build_notification_aggregate(items);

    write_notification_aggregate(&aggregate).await
}

pub async fn initialize_notification_runtime(state: &AppState) -> Result<(), String> {
    let runtime_directory = PathBuf::from(&state.settings.runtime.directory);

    let producer_directory = runtime_directory.join("notification-producers");

    let configured_path = producer_directory.join("notifications-aggregate.json");

    let _ = NOTIFICATION_AGGREGATE_PATH.set(configured_path);

    refresh_notification_aggregate().await?;

    println!(
        "Notification Runtime 6.0A hazır: {}",
        aggregate_path().display()
    );

    Ok(())
}

pub fn spawn_notification_runtime() {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(NOTIFICATION_RUNTIME_INTERVAL_SECONDS));

        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;

            if let Err(error) = refresh_notification_aggregate().await {
                eprintln!("Notification aggregate yenileme hatası: {error}");
            }
        }
    });
}

async fn read_aggregate() -> Result<NotificationAggregate, NotificationApiError> {
    let path = aggregate_path();

    if !Path::new(&path).is_file() {
        return Err(NotificationApiError::FileNotFound(path));
    }

    let content = fs::read_to_string(&path)
        .await
        .map_err(|error| NotificationApiError::ReadFailed(error.to_string()))?;

    serde_json::from_str::<NotificationAggregate>(&content)
        .map_err(|error| NotificationApiError::InvalidJson(error.to_string()))
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

pub async fn list_notifications() -> Result<Json<Value>, NotificationApiError> {
    let aggregate = read_aggregate().await?;
    let runtime = notification_runtime_status().await;
    let producer_files = notification_producer_inventory().await;

    let generated_age_seconds = if aggregate.generated_at > 0 {
        Some(unix_now().saturating_sub(aggregate.generated_at))
    } else {
        None
    };

    Ok(Json(json!({
        "ok": true,
        "schema_version": aggregate.schema_version,
        "producer_name": aggregate.producer_name,
        "producer_version": aggregate.producer_version,
        "generated_at": aggregate.generated_at,
        "generated_age_seconds": generated_age_seconds,
        "overall": aggregate.overall,
        "counts": aggregate.counts,
        "notifications": aggregate.notifications,
        "problems": aggregate.problems,
        "information": aggregate.information,
        "healthy": aggregate.healthy,
        "runtime": runtime,
        "producer_files": producer_files,
    })))
}

pub async fn notification_summary() -> Result<Json<Value>, NotificationApiError> {
    let aggregate = read_aggregate().await?;

    let generated_age_seconds = if aggregate.generated_at > 0 {
        Some(unix_now().saturating_sub(aggregate.generated_at))
    } else {
        None
    };

    Ok(Json(json!({
        "ok": true,
        "generated_at": aggregate.generated_at,
        "generated_age_seconds": generated_age_seconds,
        "overall": aggregate.overall,
        "counts": aggregate.counts,
        "active_notifications": aggregate
            .notifications
            .iter()
            .filter(|item| item.active)
            .collect::<Vec<_>>(),
    })))
}

pub async fn notification_health() -> Result<Json<Value>, NotificationApiError> {
    let aggregate = read_aggregate().await?;
    let runtime = notification_runtime_status().await;

    let generated_age_seconds = unix_now().saturating_sub(aggregate.generated_at);

    let aggregate_fresh = aggregate.generated_at > 0 && generated_age_seconds <= 120;

    Ok(Json(json!({
        "ok": aggregate_fresh,
        "aggregate_fresh": aggregate_fresh,
        "generated_at": aggregate.generated_at,
        "generated_age_seconds": generated_age_seconds,
        "producer_version": aggregate.producer_version,
        "total_producers": aggregate.counts.total,
        "fresh_producers": aggregate.counts.fresh,
        "stale_producers": aggregate.counts.stale,
        "producer_file_count":
            runtime.producer_file_count,
        "producer_directory":
            runtime.producer_directory,
        "aggregate_path":
            runtime.aggregate_path,
        "aggregate_exists":
            runtime.aggregate_exists,
        "aggregate_size_bytes":
            runtime.aggregate_size_bytes,
        "aggregate_modified_at":
            runtime.aggregate_modified_at,
        "aggregate_age_seconds":
            runtime.aggregate_age_seconds,
        "runtime": runtime,
    })))
}

/* ==================================================
PHOTOOS NOTIFICATION CENTER 5.8
REALTIME WEBSOCKET
================================================== */

fn websocket_severity(level: &str) -> &'static str {
    match level.to_ascii_lowercase().as_str() {
        "critical" => "critical",
        "error" => "error",
        "warning" => "warning",
        "success" | "healthy" => "success",
        _ => "info",
    }
}

fn notification_identity(item: &NotificationItem) -> String {
    if !item.id.trim().is_empty() {
        format!("{}:{}", item.producer, item.id)
    } else if !item.code.trim().is_empty() {
        format!("{}:{}", item.producer, item.code)
    } else {
        format!("{}:{}:{}", item.producer, item.title, item.message)
    }
}

fn notification_content_signature(item: &NotificationItem) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        item.status, item.level, item.code, item.title, item.message, item.error, item.active,
    )
}

fn aggregate_content_signature(aggregate: &NotificationAggregate) -> String {
    let mut entries = aggregate
        .notifications
        .iter()
        .map(|item| {
            format!(
                "{}|{}",
                notification_identity(item),
                notification_content_signature(item),
            )
        })
        .collect::<Vec<_>>();

    entries.sort();

    format!(
        "{}|{}|{}|{}",
        aggregate.overall.status,
        aggregate.overall.level,
        aggregate.overall.code,
        entries.join("||"),
    )
}

fn build_socket_snapshot(
    aggregate: &NotificationAggregate,
    first_seen: &mut HashMap<String, (String, i64)>,
) -> Value {
    let now = unix_now();

    let active_keys = aggregate
        .notifications
        .iter()
        .map(notification_identity)
        .collect::<std::collections::HashSet<_>>();

    first_seen.retain(|key, _| active_keys.contains(key));

    let notifications = aggregate
        .notifications
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let identity = notification_identity(item);
            let signature = notification_content_signature(item);

            let created_at = match first_seen.get(&identity) {
                Some((old_signature, timestamp)) if old_signature == &signature => *timestamp,

                _ => {
                    let timestamp = now;
                    first_seen.insert(identity.clone(), (signature, timestamp));
                    timestamp
                }
            };

            json!({
                "source": item.producer,
                "source_id": index + 1,
                "notification_type": if item.code.is_empty() {
                    item.status.clone()
                } else {
                    item.code.clone()
                },
                "severity": websocket_severity(&item.level),
                "title": item.title,
                "message": item.message,
                "details": item.error,
                "created_at": created_at,
                "updated_at": created_at,
                "is_read": !item.active,
                "active": item.active,
                "status": item.status,
                "code": item.code,
                "producer_version": item.producer_version,
                "fresh": item.fresh
            })
        })
        .collect::<Vec<_>>();

    json!({
        "event": "notification_snapshot",
        "generated_at": aggregate.generated_at,
        "summary": {
            "total_count": aggregate.counts.total,
            "unread_count": aggregate.counts.active,
            "active_count": aggregate.counts.active,
            "critical_count": aggregate.counts.critical,
            "error_count": aggregate.counts.error,
            "warning_count": aggregate.counts.warning,
            "success_count": aggregate.counts.success,
            "status": aggregate.overall.status,
            "level": aggregate.overall.level,
            "title": aggregate.overall.title,
            "message": aggregate.overall.message
        },
        "notifications": notifications
    })
}

async fn send_socket_snapshot(
    socket: &mut WebSocket,
    aggregate: &NotificationAggregate,
    first_seen: &mut HashMap<String, (String, i64)>,
) -> bool {
    let snapshot = build_socket_snapshot(aggregate, first_seen);

    let Ok(serialized) = serde_json::to_string(&snapshot) else {
        return false;
    };

    socket.send(Message::Text(serialized.into())).await.is_ok()
}

async fn send_socket_error(socket: &mut WebSocket, code: &str, message: String) -> bool {
    let payload = json!({
        "event": "notification_error",
        "generated_at": unix_now(),
        "error": {
            "code": code,
            "message": message
        }
    });

    let Ok(serialized) = serde_json::to_string(&payload) else {
        return false;
    };

    socket.send(Message::Text(serialized.into())).await.is_ok()
}

async fn notification_socket_task(mut socket: WebSocket) {
    let mut first_seen: HashMap<String, (String, i64)> = HashMap::new();

    let mut last_signature = String::new();

    match read_aggregate().await {
        Ok(aggregate) => {
            last_signature = aggregate_content_signature(&aggregate);

            if !send_socket_snapshot(&mut socket, &aggregate, &mut first_seen).await {
                return;
            }
        }

        Err(error) => {
            if !send_socket_error(&mut socket, "initial_snapshot_failed", format!("{error:?}"))
                .await
            {
                return;
            }
        }
    }

    let mut ticker = interval(Duration::from_secs(5));

    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    ticker.tick().await;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let aggregate = match read_aggregate().await {
                    Ok(aggregate) => aggregate,

                    Err(error) => {
                        if !send_socket_error(
                            &mut socket,
                            "snapshot_read_failed",
                            format!("{error:?}"),
                        )
                        .await
                        {
                            break;
                        }

                        continue;
                    }
                };

                let signature =
                    aggregate_content_signature(&aggregate);

                if signature == last_signature {
                    continue;
                }

                last_signature = signature;

                if !send_socket_snapshot(
                    &mut socket,
                    &aggregate,
                    &mut first_seen,
                )
                .await
                {
                    break;
                }
            }

            incoming = socket.recv() => {
                let Some(result) = incoming else {
                    break;
                };

                let message = match result {
                    Ok(message) => message,
                    Err(_) => break,
                };

                match message {
                    Message::Text(text) => {
                        if text.as_str().trim()
                            .eq_ignore_ascii_case("ping")
                        {
                            if socket
                                .send(Message::Text(
                                    "pong".into(),
                                ))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }

                    Message::Ping(payload) => {
                        if socket
                            .send(Message::Pong(payload))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }

                    Message::Pong(_) => {}

                    Message::Close(_) => {
                        break;
                    }

                    Message::Binary(_) => {}
                }
            }
        }
    }
}

pub async fn notification_socket(websocket: WebSocketUpgrade) -> impl IntoResponse {
    websocket.on_upgrade(notification_socket_task)
}
