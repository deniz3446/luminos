use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::{collections::HashMap, process::Command, time::{SystemTime, UNIX_EPOCH}};
use tokio::time::{self, Duration};

use crate::state::AppState;

const RETENTION_SECONDS: i64 = 7 * 24 * 60 * 60;
const WARNING_TEMP: i64 = 50;
const CRITICAL_TEMP: i64 = 55;

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub range: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TemperatureHistoryPoint {
    pub timestamp: i64,
    pub temperature_celsius: i64,
}

#[derive(Debug, Serialize)]
pub struct IoHistoryPoint {
    pub timestamp: i64,
    pub read_bytes_per_second: f64,
    pub write_bytes_per_second: f64,
    pub read_iops: f64,
    pub write_iops: f64,
    pub utilization_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct DeviceHistory {
    pub device: String,
    pub temperatures: Vec<TemperatureHistoryPoint>,
    pub io: Vec<IoHistoryPoint>,
}

#[derive(Debug, Serialize)]
pub struct StorageHistoryResponse {
    pub range: String,
    pub from_timestamp: i64,
    pub to_timestamp: i64,
    pub devices: Vec<DeviceHistory>,
}

#[derive(Debug, Serialize)]
pub struct StorageAlert {
    pub id: i64,
    pub device: String,
    pub severity: String,
    pub alert_type: String,
    pub message: String,
    pub value: Option<f64>,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct StorageAlertsResponse {
    pub alerts: Vec<StorageAlert>,
    pub active_count: usize,
}

#[derive(Debug, Clone, Default)]
struct RawIo {
    reads: u64,
    sectors_read: u64,
    writes: u64,
    sectors_written: u64,
    io_ms: u64,
}

#[derive(Debug)]
struct SmartSample {
    device: String,
    temperature: Option<i64>,
    passed: bool,
    reallocated: i64,
    pending: i64,
    uncorrectable: i64,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub async fn initialize_storage_history(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS storage_temperature_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device TEXT NOT NULL,
            temperature_celsius INTEGER NOT NULL,
            recorded_at INTEGER NOT NULL
        )
    "#).execute(db).await?;

    sqlx::query(r#"
        CREATE INDEX IF NOT EXISTS idx_storage_temperature_device_time
        ON storage_temperature_history(device, recorded_at)
    "#).execute(db).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS storage_io_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device TEXT NOT NULL,
            read_bytes_per_second REAL NOT NULL,
            write_bytes_per_second REAL NOT NULL,
            read_iops REAL NOT NULL,
            write_iops REAL NOT NULL,
            utilization_percent REAL NOT NULL,
            recorded_at INTEGER NOT NULL
        )
    "#).execute(db).await?;

    sqlx::query(r#"
        CREATE INDEX IF NOT EXISTS idx_storage_io_device_time
        ON storage_io_history(device, recorded_at)
    "#).execute(db).await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS storage_alerts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device TEXT NOT NULL,
            severity TEXT NOT NULL,
            alert_type TEXT NOT NULL,
            message TEXT NOT NULL,
            value REAL,
            created_at INTEGER NOT NULL,
            resolved_at INTEGER
        )
    "#).execute(db).await?;

    sqlx::query(r#"
        CREATE INDEX IF NOT EXISTS idx_storage_alerts_active
        ON storage_alerts(resolved_at, created_at DESC)
    "#).execute(db).await?;
    Ok(())
}

fn physical_devices() -> Vec<String> {
    let Ok(output) = Command::new("lsblk").args(["--json", "--nodeps", "--paths", "--output", "PATH,TYPE"]).output() else { return vec![]; };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else { return vec![]; };
    value.get("blockdevices").and_then(|v| v.as_array()).map(|items| items.iter().filter_map(|item| {
        if item.get("type")?.as_str()? != "disk" { return None; }
        item.get("path")?.as_str().map(str::to_string)
    }).collect()).unwrap_or_default()
}

fn smart_raw(v: &serde_json::Value, id: i64) -> i64 {
    v.get("ata_smart_attributes").and_then(|x| x.get("table")).and_then(|x| x.as_array())
        .and_then(|a| a.iter().find(|x| x.get("id").and_then(|v| v.as_i64()) == Some(id)))
        .and_then(|x| x.get("raw")).and_then(|x| x.get("value")).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn read_smart_samples() -> Vec<SmartSample> {
    physical_devices().into_iter().filter_map(|device| {
        let output = Command::new("/usr/sbin/smartctl").args(["-a", "-j", &device]).output().ok()?;
        let v: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let temperature = v.get("temperature").and_then(|x| x.get("current")).and_then(|x| x.as_i64())
            .or_else(|| { let t = smart_raw(&v, 194); (t > 0).then_some(t) });
        Some(SmartSample {
            device,
            temperature,
            passed: v.get("smart_status").and_then(|x| x.get("passed")).and_then(|x| x.as_bool()).unwrap_or(false),
            reallocated: smart_raw(&v, 5),
            pending: smart_raw(&v, 197),
            uncorrectable: smart_raw(&v, 198),
        })
    }).collect()
}

fn raw_io() -> HashMap<String, RawIo> {
    let devices = physical_devices();
    let text = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let mut out = HashMap::new();
    for line in text.lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        if p.len() < 14 { continue; }
        let device = format!("/dev/{}", p[2]);
        if !devices.contains(&device) { continue; }
        out.insert(device, RawIo {
            reads: p[3].parse().unwrap_or(0), sectors_read: p[5].parse().unwrap_or(0),
            writes: p[7].parse().unwrap_or(0), sectors_written: p[9].parse().unwrap_or(0),
            io_ms: p[12].parse().unwrap_or(0),
        });
    }
    out
}

async fn set_alert(db: &SqlitePool, device: &str, alert_type: &str, active: bool, severity: &str, message: &str, value: Option<f64>) {
    if active {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM storage_alerts WHERE device=? AND alert_type=? AND resolved_at IS NULL")
            .bind(device).bind(alert_type).fetch_one(db).await.unwrap_or(0);
        if exists == 0 {
            let _ = sqlx::query("INSERT INTO storage_alerts(device,severity,alert_type,message,value,created_at) VALUES(?,?,?,?,?,?)")
                .bind(device).bind(severity).bind(alert_type).bind(message).bind(value).bind(now()).execute(db).await;
        }
    } else {
        let _ = sqlx::query("UPDATE storage_alerts SET resolved_at=? WHERE device=? AND alert_type=? AND resolved_at IS NULL")
            .bind(now()).bind(device).bind(alert_type).execute(db).await;
    }
}

async fn collect_once(db: &SqlitePool, previous: &mut HashMap<String, RawIo>) {
    let timestamp = now();
    for smart in read_smart_samples() {
        if let Some(temp) = smart.temperature {
            let _ = sqlx::query("INSERT INTO storage_temperature_history(device,temperature_celsius,recorded_at) VALUES(?,?,?)")
                .bind(&smart.device).bind(temp).bind(timestamp).execute(db).await;
            set_alert(db, &smart.device, "temperature_critical", temp >= CRITICAL_TEMP, "critical", &format!("Disk sıcaklığı kritik seviyede: {temp}°C"), Some(temp as f64)).await;
            set_alert(db, &smart.device, "temperature_warning", temp >= WARNING_TEMP && temp < CRITICAL_TEMP, "warning", &format!("Disk sıcaklığı yüksek: {temp}°C"), Some(temp as f64)).await;
        }
        set_alert(db, &smart.device, "smart_failed", !smart.passed, "critical", "SMART genel sağlık testi başarısız.", None).await;
        set_alert(db, &smart.device, "reallocated_sectors", smart.reallocated > 0, "warning", &format!("Yeniden eşlenen sektör sayısı: {}", smart.reallocated), Some(smart.reallocated as f64)).await;
        set_alert(db, &smart.device, "pending_sectors", smart.pending > 0, "critical", &format!("Bekleyen sektör sayısı: {}", smart.pending), Some(smart.pending as f64)).await;
        set_alert(db, &smart.device, "offline_uncorrectable", smart.uncorrectable > 0, "critical", &format!("Düzeltilemeyen sektör sayısı: {}", smart.uncorrectable), Some(smart.uncorrectable as f64)).await;
    }

    let current = raw_io();
    for (device, sample) in &current {
        if let Some(old) = previous.get(device) {
            let seconds = 60.0;
            let delta = |n: u64, o: u64| n.saturating_sub(o) as f64;
            let read_bps = delta(sample.sectors_read, old.sectors_read) * 512.0 / seconds;
            let write_bps = delta(sample.sectors_written, old.sectors_written) * 512.0 / seconds;
            let read_iops = delta(sample.reads, old.reads) / seconds;
            let write_iops = delta(sample.writes, old.writes) / seconds;
            let utilization = (delta(sample.io_ms, old.io_ms) / (seconds * 10.0)).clamp(0.0, 100.0);
            let _ = sqlx::query("INSERT INTO storage_io_history(device,read_bytes_per_second,write_bytes_per_second,read_iops,write_iops,utilization_percent,recorded_at) VALUES(?,?,?,?,?,?,?)")
                .bind(device).bind(read_bps).bind(write_bps).bind(read_iops).bind(write_iops).bind(utilization).bind(timestamp).execute(db).await;
        }
    }
    *previous = current;

    let cutoff = timestamp - RETENTION_SECONDS;
    let _ = sqlx::query("DELETE FROM storage_temperature_history WHERE recorded_at < ?").bind(cutoff).execute(db).await;
    let _ = sqlx::query("DELETE FROM storage_io_history WHERE recorded_at < ?").bind(cutoff).execute(db).await;
}

pub fn spawn_storage_history_collector(db: SqlitePool) {
    tokio::spawn(async move {
        let mut previous = raw_io();
        collect_once(&db, &mut previous).await;
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            collect_once(&db, &mut previous).await;
        }
    });
}

fn range_seconds(range: &str) -> i64 {
    match range { "1h" => 3600, "24h" => 86400, "7d" => 604800, _ => 3600 }
}

pub async fn get_storage_history(State(state): State<AppState>, Query(query): Query<HistoryQuery>) -> Json<StorageHistoryResponse> {
    let range = query.range.unwrap_or_else(|| "1h".to_string());
    let to = now();
    let from = to - range_seconds(&range);
    let temp_rows = sqlx::query("SELECT device,temperature_celsius,recorded_at FROM storage_temperature_history WHERE recorded_at>=? ORDER BY recorded_at")
        .bind(from).fetch_all(&state.db).await.unwrap_or_default();
    let io_rows = sqlx::query("SELECT device,read_bytes_per_second,write_bytes_per_second,read_iops,write_iops,utilization_percent,recorded_at FROM storage_io_history WHERE recorded_at>=? ORDER BY recorded_at")
        .bind(from).fetch_all(&state.db).await.unwrap_or_default();
    let mut devices: HashMap<String, DeviceHistory> = HashMap::new();
    for row in temp_rows {
        let device: String = row.get("device");
        devices.entry(device.clone()).or_insert(DeviceHistory { device, temperatures: vec![], io: vec![] }).temperatures.push(TemperatureHistoryPoint {
            temperature_celsius: row.get("temperature_celsius"), timestamp: row.get("recorded_at")
        });
    }
    for row in io_rows {
        let device: String = row.get("device");
        devices.entry(device.clone()).or_insert(DeviceHistory { device, temperatures: vec![], io: vec![] }).io.push(IoHistoryPoint {
            timestamp: row.get("recorded_at"), read_bytes_per_second: row.get("read_bytes_per_second"),
            write_bytes_per_second: row.get("write_bytes_per_second"), read_iops: row.get("read_iops"),
            write_iops: row.get("write_iops"), utilization_percent: row.get("utilization_percent")
        });
    }
    let mut devices: Vec<_> = devices.into_values().collect();
    devices.sort_by(|a,b| a.device.cmp(&b.device));
    Json(StorageHistoryResponse { range, from_timestamp: from, to_timestamp: to, devices })
}

pub async fn get_storage_alerts(State(state): State<AppState>) -> Json<StorageAlertsResponse> {
    let rows = sqlx::query("SELECT id,device,severity,alert_type,message,value,created_at,resolved_at FROM storage_alerts ORDER BY (resolved_at IS NULL) DESC, created_at DESC LIMIT 100")
        .fetch_all(&state.db).await.unwrap_or_default();
    let alerts: Vec<_> = rows.into_iter().map(|row| StorageAlert {
        id: row.get("id"), device: row.get("device"), severity: row.get("severity"), alert_type: row.get("alert_type"),
        message: row.get("message"), value: row.get("value"), created_at: row.get("created_at"), resolved_at: row.get("resolved_at")
    }).collect();
    let active_count = alerts.iter().filter(|a| a.resolved_at.is_none()).count();
    Json(StorageAlertsResponse { alerts, active_count })
}
