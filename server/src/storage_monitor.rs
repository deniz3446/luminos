use std::{
    collections::HashMap,
    process::Command,
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{Query, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::state::AppState;

const SAMPLE_INTERVAL_SECONDS: u64 = 30;
const RETENTION_DAYS: i64 = 7;
const HISTORY_BUCKET_SECONDS: i64 = 300;

#[derive(Debug, Clone)]
struct DiskDescriptor {
    device: String,
    name: String,
    model: String,
}

#[derive(Debug, Clone, Copy)]
struct IoCounters {
    reads_completed: u64,
    writes_completed: u64,
    sectors_read: u64,
    sectors_written: u64,
    io_milliseconds: u64,
}

#[derive(Debug, Clone, Copy)]
struct PreviousIo {
    counters: IoCounters,
    sampled_at: Instant,
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    hours: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct StorageMetricsResponse {
    checked_at: String,
    sample_interval_seconds: u64,
    retention_days: i64,
    devices: Vec<DeviceMetrics>,
}

#[derive(Debug, Serialize)]
pub struct DeviceMetrics {
    device: String,
    model: String,
    current: Option<MetricPoint>,
    history: Vec<MetricPoint>,
}

#[derive(Debug, Serialize)]
pub struct MetricPoint {
    timestamp: i64,
    temperature_celsius: Option<i64>,
    total_read_bytes: i64,
    total_write_bytes: i64,
    read_bytes_per_second: i64,
    write_bytes_per_second: i64,
    read_iops: f64,
    write_iops: f64,
    busy_percent: f64,
}

fn value_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
}

fn smart_attribute_raw(smart: &serde_json::Value, id: i64) -> Option<i64> {
    smart
        .get("ata_smart_attributes")?
        .get("table")?
        .as_array()?
        .iter()
        .find_map(|attribute| {
            if attribute.get("id").and_then(value_i64)? != id {
                return None;
            }

            attribute
                .get("raw")
                .and_then(|raw| raw.get("value"))
                .and_then(value_i64)
        })
}

fn read_temperature(device: &str) -> Option<i64> {
    let output = Command::new("/usr/sbin/smartctl")
        .args(["-a", "-j", device])
        .output()
        .ok()?;

    let smart = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok()?;

    smart
        .get("temperature")
        .and_then(|value| value.get("current"))
        .and_then(value_i64)
        .or_else(|| smart_attribute_raw(&smart, 194))
}

fn physical_disks() -> Vec<DiskDescriptor> {
    let output = match Command::new("lsblk")
        .args([
            "--json",
            "--nodeps",
            "--paths",
            "--output",
            "NAME,PATH,TYPE,MODEL",
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let value = match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    value
        .get("blockdevices")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if item.get("type")?.as_str()? != "disk" {
                        return None;
                    }

                    let device = item.get("path")?.as_str()?.to_string();
                    let name = device.trim_start_matches("/dev/").to_string();

                    if device.is_empty() || name.is_empty() {
                        return None;
                    }

                    let model = item
                        .get("model")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    Some(DiskDescriptor {
                        device,
                        name,
                        model,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn read_diskstats() -> HashMap<String, IoCounters> {
    let text = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let mut result = HashMap::new();

    for line in text.lines() {
        let columns = line.split_whitespace().collect::<Vec<_>>();

        if columns.len() < 14 {
            continue;
        }

        let parse = |index: usize| {
            columns
                .get(index)
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0)
        };

        result.insert(
            columns[2].to_string(),
            IoCounters {
                reads_completed: parse(3),
                sectors_read: parse(5),
                writes_completed: parse(7),
                sectors_written: parse(9),
                io_milliseconds: parse(12),
            },
        );
    }

    result
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

async fn ensure_schema(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS storage_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recorded_at INTEGER NOT NULL,
            device TEXT NOT NULL,
            model TEXT NOT NULL DEFAULT '',
            temperature_celsius INTEGER,
            total_read_bytes INTEGER NOT NULL DEFAULT 0,
            total_write_bytes INTEGER NOT NULL DEFAULT 0,
            read_bytes_per_second INTEGER NOT NULL DEFAULT 0,
            write_bytes_per_second INTEGER NOT NULL DEFAULT 0,
            read_iops REAL NOT NULL DEFAULT 0,
            write_iops REAL NOT NULL DEFAULT 0,
            busy_percent REAL NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_storage_metrics_device_time
        ON storage_metrics(device, recorded_at DESC)
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn collect_sample(
    db: &SqlitePool,
    previous: &mut HashMap<String, PreviousIo>,
) -> Result<(), sqlx::Error> {
    let recorded_at = Utc::now().timestamp();
    let now = Instant::now();
    let diskstats = read_diskstats();

    for disk in physical_disks() {
        let Some(counters) = diskstats.get(&disk.name).copied() else {
            continue;
        };

        let (read_bps, write_bps, read_iops, write_iops, busy_percent) =
            if let Some(old) = previous.get(&disk.name) {
                let elapsed = now.duration_since(old.sampled_at).as_secs_f64().max(0.001);
                let read_bytes = counters
                    .sectors_read
                    .saturating_sub(old.counters.sectors_read)
                    .saturating_mul(512);
                let write_bytes = counters
                    .sectors_written
                    .saturating_sub(old.counters.sectors_written)
                    .saturating_mul(512);

                (
                    (read_bytes as f64 / elapsed).round() as u64,
                    (write_bytes as f64 / elapsed).round() as u64,
                    counters.reads_completed.saturating_sub(old.counters.reads_completed)
                        as f64
                        / elapsed,
                    counters
                        .writes_completed
                        .saturating_sub(old.counters.writes_completed)
                        as f64
                        / elapsed,
                    (counters
                        .io_milliseconds
                        .saturating_sub(old.counters.io_milliseconds)
                        as f64
                        / (elapsed * 1000.0)
                        * 100.0)
                        .clamp(0.0, 100.0),
                )
            } else {
                (0, 0, 0.0, 0.0, 0.0)
            };

        let temperature = read_temperature(&disk.device);
        let total_read_bytes = counters.sectors_read.saturating_mul(512);
        let total_write_bytes = counters.sectors_written.saturating_mul(512);

        sqlx::query(
            r#"
            INSERT INTO storage_metrics (
                recorded_at, device, model, temperature_celsius,
                total_read_bytes, total_write_bytes,
                read_bytes_per_second, write_bytes_per_second,
                read_iops, write_iops, busy_percent
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(recorded_at)
        .bind(&disk.device)
        .bind(&disk.model)
        .bind(temperature)
        .bind(to_i64(total_read_bytes))
        .bind(to_i64(total_write_bytes))
        .bind(to_i64(read_bps))
        .bind(to_i64(write_bps))
        .bind(round_two(read_iops))
        .bind(round_two(write_iops))
        .bind(round_two(busy_percent))
        .execute(db)
        .await?;

        previous.insert(
            disk.name,
            PreviousIo {
                counters,
                sampled_at: now,
            },
        );
    }

    let cutoff = recorded_at - RETENTION_DAYS * 24 * 60 * 60;
    sqlx::query("DELETE FROM storage_metrics WHERE recorded_at < ?")
        .bind(cutoff)
        .execute(db)
        .await?;

    Ok(())
}

pub fn start(db: SqlitePool) {
    tokio::spawn(async move {
        if let Err(error) = ensure_schema(&db).await {
            eprintln!("Storage Monitor tablosu oluşturulamadı: {error}");
            return;
        }

        let mut previous = HashMap::new();

        loop {
            if let Err(error) = collect_sample(&db, &mut previous).await {
                eprintln!("Storage Monitor örneği kaydedilemedi: {error}");
            }

            tokio::time::sleep(Duration::from_secs(SAMPLE_INTERVAL_SECONDS)).await;
        }
    });
}

fn metric_from_row(row: &sqlx::sqlite::SqliteRow) -> MetricPoint {
    MetricPoint {
        timestamp: row.try_get("timestamp").unwrap_or(0),
        temperature_celsius: row.try_get("temperature_celsius").unwrap_or(None),
        total_read_bytes: row.try_get("total_read_bytes").unwrap_or(0),
        total_write_bytes: row.try_get("total_write_bytes").unwrap_or(0),
        read_bytes_per_second: row.try_get("read_bytes_per_second").unwrap_or(0),
        write_bytes_per_second: row.try_get("write_bytes_per_second").unwrap_or(0),
        read_iops: row.try_get("read_iops").unwrap_or(0.0),
        write_iops: row.try_get("write_iops").unwrap_or(0.0),
        busy_percent: row.try_get("busy_percent").unwrap_or(0.0),
    }
}

pub async fn get_storage_metrics(
    State(state): State<AppState>,
    Query(query): Query<MetricsQuery>,
) -> Json<StorageMetricsResponse> {
    if let Err(error) = ensure_schema(&state.db).await {
        eprintln!("Storage Monitor şeması okunamadı: {error}");
    }

    let hours = query.hours.unwrap_or(24).clamp(1, RETENTION_DAYS * 24);
    let cutoff = Utc::now().timestamp() - hours * 60 * 60;
    let mut devices = Vec::new();

    for disk in physical_disks() {
        let current = sqlx::query(
            r#"
            SELECT
                recorded_at AS timestamp,
                temperature_celsius,
                total_read_bytes,
                total_write_bytes,
                read_bytes_per_second,
                write_bytes_per_second,
                read_iops,
                write_iops,
                busy_percent
            FROM storage_metrics
            WHERE device = ?
            ORDER BY recorded_at DESC
            LIMIT 1
            "#,
        )
        .bind(&disk.device)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|row| metric_from_row(&row));

        let history_rows = sqlx::query(
            r#"
            SELECT
                (recorded_at / ?) * ? AS timestamp,
                CAST(ROUND(AVG(temperature_celsius)) AS INTEGER)
                    AS temperature_celsius,
                MAX(total_read_bytes) AS total_read_bytes,
                MAX(total_write_bytes) AS total_write_bytes,
                CAST(ROUND(AVG(read_bytes_per_second)) AS INTEGER)
                    AS read_bytes_per_second,
                CAST(ROUND(AVG(write_bytes_per_second)) AS INTEGER)
                    AS write_bytes_per_second,
                ROUND(AVG(read_iops), 2) AS read_iops,
                ROUND(AVG(write_iops), 2) AS write_iops,
                ROUND(AVG(busy_percent), 2) AS busy_percent
            FROM storage_metrics
            WHERE device = ? AND recorded_at >= ?
            GROUP BY (recorded_at / ?)
            ORDER BY timestamp ASC
            "#,
        )
        .bind(HISTORY_BUCKET_SECONDS)
        .bind(HISTORY_BUCKET_SECONDS)
        .bind(&disk.device)
        .bind(cutoff)
        .bind(HISTORY_BUCKET_SECONDS)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
        .iter()
        .map(metric_from_row)
        .collect();

        devices.push(DeviceMetrics {
            device: disk.device,
            model: disk.model,
            current,
            history: history_rows,
        });
    }

    Json(StorageMetricsResponse {
        checked_at: Utc::now().to_rfc3339(),
        sample_interval_seconds: SAMPLE_INTERVAL_SECONDS,
        retention_days: RETENTION_DAYS,
        devices,
    })
}
