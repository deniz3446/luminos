use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tokio::{
    fs,
    process::Command,
    time::{Duration, MissedTickBehavior, interval, sleep},
};

use crate::state::AppState;

const PRODUCER_NAME: &str = "performance-alert-engine";
const PRODUCER_VERSION: &str = "1.0B";
const LOOP_SECONDS: u64 = 15;

#[derive(Debug, Clone, Copy)]
struct AlertThresholds {
    cpu_warning: f64,
    cpu_critical: f64,
    memory_warning: f64,
    memory_critical: f64,
    load_per_core_warning: f64,
    load_per_core_critical: f64,
    root_disk_warning: f64,
    root_disk_critical: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        let (cpu_warning, cpu_critical) = ordered_thresholds(
            env_f64("PHOTOOS_PERF_CPU_WARNING", 85.0),
            env_f64("PHOTOOS_PERF_CPU_CRITICAL", 95.0),
        );
        let (memory_warning, memory_critical) = ordered_thresholds(
            env_f64("PHOTOOS_PERF_MEMORY_WARNING", 85.0),
            env_f64("PHOTOOS_PERF_MEMORY_CRITICAL", 95.0),
        );
        let (load_per_core_warning, load_per_core_critical) = ordered_thresholds(
            env_f64("PHOTOOS_PERF_LOAD_WARNING", 1.25),
            env_f64("PHOTOOS_PERF_LOAD_CRITICAL", 2.0),
        );
        let (root_disk_warning, root_disk_critical) = ordered_thresholds(
            env_f64("PHOTOOS_PERF_ROOT_DISK_WARNING", 85.0),
            env_f64("PHOTOOS_PERF_ROOT_DISK_CRITICAL", 95.0),
        );

        Self {
            cpu_warning,
            cpu_critical,
            memory_warning,
            memory_critical,
            load_per_core_warning,
            load_per_core_critical,
            root_disk_warning,
            root_disk_critical,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PerformanceNotification {
    id: String,
    producer: String,
    producer_version: String,
    status: String,
    level: String,
    code: String,
    title: String,
    message: String,
    error: String,
    generated_at: i64,
    active: bool,
    value: f64,
}

#[derive(Debug, Serialize)]
struct PerformanceProducer {
    schema_version: u32,
    producer_name: String,
    producer_version: String,
    generated_at: i64,
    notifications: Vec<PerformanceNotification>,
}

#[derive(Debug, Clone, Copy)]
struct CpuSample {
    idle: u64,
    total: u64,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(default)
}

fn ordered_thresholds(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

fn truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn alert_test_mode() -> bool {
    std::env::var("PHOTOOS_ALERT_TEST_MODE")
        .ok()
        .is_some_and(|value| truthy(&value))
}

fn producer_path() -> PathBuf {
    std::env::var("PHOTOOS_NOTIFICATION_PRODUCER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("/var/lib/photoos/runtime/notification-producers")
        })
        .join("performance-alerts.json")
}

fn parse_cpu_sample(text: &str) -> Option<CpuSample> {
    let line = text.lines().find(|line| line.starts_with("cpu "))?;
    let mut values = line.split_whitespace().skip(1).map(str::parse::<u64>);

    let user = values.next()?.ok()?;
    let nice = values.next()?.ok()?;
    let system = values.next()?.ok()?;
    let idle = values.next()?.ok()?;
    let iowait = values.next().and_then(Result::ok).unwrap_or(0);
    let irq = values.next().and_then(Result::ok).unwrap_or(0);
    let softirq = values.next().and_then(Result::ok).unwrap_or(0);
    let steal = values.next().and_then(Result::ok).unwrap_or(0);

    Some(CpuSample {
        idle: idle.saturating_add(iowait),
        total: user
            .saturating_add(nice)
            .saturating_add(system)
            .saturating_add(idle)
            .saturating_add(iowait)
            .saturating_add(irq)
            .saturating_add(softirq)
            .saturating_add(steal),
    })
}

fn cpu_usage(first: CpuSample, second: CpuSample) -> Option<f64> {
    let total = second.total.checked_sub(first.total)?;
    let idle = second.idle.checked_sub(first.idle)?;

    if total == 0 || idle > total {
        return None;
    }

    Some((total - idle) as f64 / total as f64 * 100.0)
}

fn parse_memory_usage(text: &str) -> Option<f64> {
    let mut total = None;
    let mut available = None;

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next()? {
            "MemTotal:" => total = parts.next()?.parse::<f64>().ok(),
            "MemAvailable:" => available = parts.next()?.parse::<f64>().ok(),
            _ => {}
        }
    }

    let total = total?;
    let available = available?;

    if total <= 0.0 || available > total {
        return None;
    }

    Some((total - available) / total * 100.0)
}

fn parse_load_per_core(text: &str, cores: usize) -> Option<f64> {
    let one_minute = text.split_whitespace().next()?.parse::<f64>().ok()?;
    if cores == 0 {
        return None;
    }
    Some(one_minute / cores as f64)
}

fn parse_df_usage(text: &str) -> Option<f64> {
    let line = text.lines().nth(1)?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    let percent = fields.get(4)?.trim_end_matches('%').parse::<f64>().ok()?;
    Some(percent)
}

fn evaluate_metric(
    code: &str,
    title: &str,
    value: f64,
    warning: f64,
    critical: f64,
    unit: &str,
) -> PerformanceNotification {
    let (status, level, active) = if value >= critical {
        ("problem", "critical", true)
    } else if value >= warning {
        ("problem", "warning", true)
    } else {
        ("healthy", "success", false)
    };

    PerformanceNotification {
        id: format!("performance:{code}"),
        producer: PRODUCER_NAME.to_string(),
        producer_version: PRODUCER_VERSION.to_string(),
        status: status.to_string(),
        level: level.to_string(),
        code: format!("performance_{code}"),
        title: title.to_string(),
        message: format!("{title}: {value:.1}{unit}"),
        error: String::new(),
        generated_at: unix_now(),
        active,
        value,
    }
}

async fn read_cpu_usage() -> Option<f64> {
    let first = fs::read_to_string("/proc/stat").await.ok()?;
    let first = parse_cpu_sample(&first)?;
    sleep(Duration::from_millis(200)).await;
    let second = fs::read_to_string("/proc/stat").await.ok()?;
    cpu_usage(first, parse_cpu_sample(&second)?)
}

async fn read_root_disk_usage() -> Option<f64> {
    let output = Command::new("/usr/bin/df")
        .args(["-P", "-B1", "/"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_df_usage(&String::from_utf8_lossy(&output.stdout))
}

async fn collect_performance_alerts() -> Vec<PerformanceNotification> {
    if alert_test_mode() {
        return vec![evaluate_metric(
            "test_mode",
            "Performance Test Mode",
            100.0,
            1.0,
            2.0,
            "%",
        )];
    }

    let thresholds = AlertThresholds::default();
    let mut notifications = Vec::new();

    if let Some(cpu) = read_cpu_usage().await {
        notifications.push(evaluate_metric(
            "cpu",
            "CPU",
            cpu,
            thresholds.cpu_warning,
            thresholds.cpu_critical,
            "%",
        ));
    }

    if let Ok(memory) = fs::read_to_string("/proc/meminfo").await {
        if let Some(memory) = parse_memory_usage(&memory) {
            notifications.push(evaluate_metric(
                "memory",
                "RAM",
                memory,
                thresholds.memory_warning,
                thresholds.memory_critical,
                "%",
            ));
        }
    }

    if let Ok(load) = fs::read_to_string("/proc/loadavg").await {
        let cores = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1);

        if let Some(load_per_core) = parse_load_per_core(&load, cores) {
            notifications.push(evaluate_metric(
                "load",
                "Load Average / Core",
                load_per_core,
                thresholds.load_per_core_warning,
                thresholds.load_per_core_critical,
                "x",
            ));
        }
    }

    if let Some(root_disk) = read_root_disk_usage().await {
        notifications.push(evaluate_metric(
            "root_disk",
            "Root Disk",
            root_disk,
            thresholds.root_disk_warning,
            thresholds.root_disk_critical,
            "%",
        ));
    }

    notifications
}

async fn write_producer(path: &Path, notifications: Vec<PerformanceNotification>) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("Performance producer parent dizini yok.".to_string());
    };

    fs::create_dir_all(parent)
        .await
        .map_err(|_| "Performance producer klasörü oluşturulamadı.".to_string())?;

    let document = PerformanceProducer {
        schema_version: 1,
        producer_name: PRODUCER_NAME.to_string(),
        producer_version: PRODUCER_VERSION.to_string(),
        generated_at: unix_now(),
        notifications,
    };

    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|_| "Performance producer JSON oluşturulamadı.".to_string())?;

    let tmp = path.with_extension("json.tmp");

    fs::write(&tmp, bytes)
        .await
        .map_err(|_| "Performance producer geçici dosyası yazılamadı.".to_string())?;

    fs::rename(&tmp, path)
        .await
        .map_err(|_| "Performance producer atomik olarak yayınlanamadı.".to_string())?;

    Ok(())
}

async fn refresh_performance_alerts() -> Result<(), String> {
    let notifications = collect_performance_alerts().await;
    write_producer(&producer_path(), notifications).await
}

pub async fn initialize_performance_alerts(_state: &AppState) -> Result<(), String> {
    refresh_performance_alerts().await
}

pub fn spawn_performance_alert_engine(_state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(LOOP_SECONDS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;

            if let Err(error) = refresh_performance_alerts().await {
                eprintln!("Performance Alert Engine hatası: {error}");
            }
        }
    });

    println!("Performance Alert Engine 1.0B hazır");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_evaluation_is_stable() {
        let healthy = evaluate_metric("cpu", "CPU", 20.0, 85.0, 95.0, "%");
        assert_eq!(healthy.level, "success");
        assert!(!healthy.active);

        let warning = evaluate_metric("cpu", "CPU", 90.0, 85.0, 95.0, "%");
        assert_eq!(warning.level, "warning");
        assert!(warning.active);

        let critical = evaluate_metric("cpu", "CPU", 99.0, 85.0, 95.0, "%");
        assert_eq!(critical.level, "critical");
        assert!(critical.active);
    }

    #[test]
    fn proc_parsers_accept_linux_samples() {
        let first = parse_cpu_sample("cpu  100 0 50 850 0 0 0 0\n").unwrap();
        let second = parse_cpu_sample("cpu  150 0 70 880 0 0 0 0\n").unwrap();
        let cpu = cpu_usage(first, second).unwrap();
        assert!((cpu - 70.0).abs() < 0.001);

        let memory = parse_memory_usage(
            "MemTotal:       1000 kB\nMemAvailable:    250 kB\n",
        )
        .unwrap();
        assert!((memory - 75.0).abs() < 0.001);

        let load = parse_load_per_core("4.00 3.00 2.00 1/100 1\n", 4).unwrap();
        assert!((load - 1.0).abs() < 0.001);

        let disk = parse_df_usage(
            "Filesystem 1-blocks Used Available Capacity Mounted on\n/dev/sda1 1000 900 100 90% /\n",
        )
        .unwrap();
        assert!((disk - 90.0).abs() < 0.001);
    }

    #[test]
    fn live_producer_identity_and_interval_are_preserved() {
        assert_eq!(PRODUCER_NAME, "performance-alert-engine");
        assert_eq!(PRODUCER_VERSION, "1.0B");
        assert_eq!(LOOP_SECONDS, 15);
        assert!(producer_path().ends_with("performance-alerts.json"));
    }

    #[test]
    fn thresholds_are_ordered_and_test_mode_values_are_explicit() {
        assert_eq!(ordered_thresholds(95.0, 85.0), (85.0, 95.0));
        assert_eq!(ordered_thresholds(85.0, 95.0), (85.0, 95.0));
        assert!(truthy("1"));
        assert!(truthy("TRUE"));
        assert!(truthy("on"));
        assert!(!truthy("0"));
        assert!(!truthy("false"));
    }

}
