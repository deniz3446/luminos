use axum::Json;
use serde::Serialize;
use std::{fs, time::Duration};
use tokio::{process::Command, time::sleep};

#[derive(Debug, Clone, Copy)]
struct CpuSnapshot {
    idle: u64,
    total: u64,
}

#[derive(Debug, Serialize)]
pub struct MemoryMetrics {
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
    usage_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct NetworkInterfaceMetrics {
    interface: String,
    received_bytes: u64,
    transmitted_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct DiskMetrics {
    name: String,
    model: String,
    size_bytes: u64,
    fs_type: String,
    label: String,
    mountpoint: String,
    mounted: bool,
    used_bytes: u64,
    free_bytes: u64,
    usage_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    hostname: String,
    operating_system: String,
    kernel_version: String,
    architecture: String,

    cpu_model: String,
    cpu_cores: usize,
    cpu_usage_percent: f64,

    memory: MemoryMetrics,

    uptime_seconds: u64,
    load_average_1m: f64,
    load_average_5m: f64,
    load_average_15m: f64,

    network: Vec<NetworkInterfaceMetrics>,
    disks: Vec<DiskMetrics>,
}

fn read_cpu_snapshot() -> Option<CpuSnapshot> {
    let contents = fs::read_to_string("/proc/stat").ok()?;
    let line = contents.lines().next()?;

    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect();

    if values.len() < 4 {
        return None;
    }

    let user = values.first().copied().unwrap_or(0);
    let nice = values.get(1).copied().unwrap_or(0);
    let system = values.get(2).copied().unwrap_or(0);
    let idle = values.get(3).copied().unwrap_or(0);
    let iowait = values.get(4).copied().unwrap_or(0);
    let irq = values.get(5).copied().unwrap_or(0);
    let softirq = values.get(6).copied().unwrap_or(0);
    let steal = values.get(7).copied().unwrap_or(0);

    let idle_all = idle.saturating_add(iowait);

    let total = user
        .saturating_add(nice)
        .saturating_add(system)
        .saturating_add(idle)
        .saturating_add(iowait)
        .saturating_add(irq)
        .saturating_add(softirq)
        .saturating_add(steal);

    Some(CpuSnapshot {
        idle: idle_all,
        total,
    })
}

async fn cpu_usage_percent() -> f64 {
    let Some(first) = read_cpu_snapshot() else {
        return 0.0;
    };

    sleep(Duration::from_millis(150)).await;

    let Some(second) = read_cpu_snapshot() else {
        return 0.0;
    };

    let total_delta = second.total.saturating_sub(first.total);
    let idle_delta = second.idle.saturating_sub(first.idle);

    if total_delta == 0 {
        return 0.0;
    }

    let busy_delta = total_delta.saturating_sub(idle_delta);

    ((busy_delta as f64 / total_delta as f64) * 100.0).clamp(0.0, 100.0)
}

fn read_memory_metrics() -> MemoryMetrics {
    let contents = fs::read_to_string("/proc/meminfo").unwrap_or_default();

    let mut total_kb = 0_u64;
    let mut available_kb = 0_u64;

    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            total_kb = value
                .split_whitespace()
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
        }

        if let Some(value) = line.strip_prefix("MemAvailable:") {
            available_kb = value
                .split_whitespace()
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
        }
    }

    let total_bytes = total_kb.saturating_mul(1024);
    let available_bytes = available_kb.saturating_mul(1024);
    let used_bytes = total_bytes.saturating_sub(available_bytes);

    let usage_percent = if total_bytes == 0 {
        0.0
    } else {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    };

    MemoryMetrics {
        total_bytes,
        used_bytes,
        available_bytes,
        usage_percent: usage_percent.clamp(0.0, 100.0),
    }
}

fn read_network_metrics() -> Vec<NetworkInterfaceMetrics> {
    let contents = fs::read_to_string("/proc/net/dev").unwrap_or_default();

    contents
        .lines()
        .skip(2)
        .filter_map(|line| {
            let (interface, values) = line.split_once(':')?;

            let values: Vec<&str> = values.split_whitespace().collect();

            if values.len() < 16 {
                return None;
            }

            let interface = interface.trim().to_string();

            if interface == "lo" {
                return None;
            }

            let received_bytes = values[0].parse::<u64>().ok()?;

            let transmitted_bytes = values[8].parse::<u64>().ok()?;

            Some(NetworkInterfaceMetrics {
                interface,
                received_bytes,
                transmitted_bytes,
            })
        })
        .collect()
}

fn read_uptime_seconds() -> u64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|value| {
            value
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<f64>().ok())
        })
        .map(|value| value as u64)
        .unwrap_or(0)
}

fn read_load_average() -> (f64, f64, f64) {
    let contents = fs::read_to_string("/proc/loadavg").unwrap_or_default();

    let mut values = contents
        .split_whitespace()
        .take(3)
        .filter_map(|value| value.parse::<f64>().ok());

    (
        values.next().unwrap_or(0.0),
        values.next().unwrap_or(0.0),
        values.next().unwrap_or(0.0),
    )
}

fn read_hostname() -> String {
    fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "PhotoOS".to_string())
        .trim()
        .to_string()
}

fn read_os_name() -> String {
    let contents = fs::read_to_string("/etc/os-release").unwrap_or_default();

    contents
        .lines()
        .find_map(|line| {
            line.strip_prefix("PRETTY_NAME=")
                .map(|value| value.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "Debian GNU/Linux".to_string())
}

fn read_kernel_version() -> String {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn read_cpu_model() -> String {
    let contents = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();

    contents
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;

            if key.trim() == "model name" {
                Some(value.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "Bilinmeyen işlemci".to_string())
}

fn read_cpu_cores() -> usize {
    let contents = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();

    let count = contents
        .lines()
        .filter(|line| line.starts_with("processor"))
        .count();

    count.max(1)
}

fn json_string(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn json_u64(value: Option<&serde_json::Value>) -> u64 {
    value
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
        })
        .unwrap_or(0)
}

fn child_value(device: &serde_json::Value, key: &str) -> String {
    let direct = json_string(device.get(key));

    if !direct.is_empty() {
        return direct;
    }

    let Some(children) = device.get("children").and_then(|value| value.as_array()) else {
        return String::new();
    };

    for child in children {
        let value = child_value(child, key);

        if !value.is_empty() {
            return value;
        }
    }

    String::new()
}

async fn read_mount_usage(mountpoint: &str) -> (u64, u64, f64) {
    if mountpoint.is_empty() {
        return (0, 0, 0.0);
    }

    let output = Command::new("df")
        .args(["-B1", "--output=used,avail,pcent", mountpoint])
        .output()
        .await;

    let Ok(output) = output else {
        return (0, 0, 0.0);
    };

    if !output.status.success() {
        return (0, 0, 0.0);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let Some(line) = stdout.lines().nth(1) else {
        return (0, 0, 0.0);
    };

    let values: Vec<&str> = line.split_whitespace().collect();

    if values.len() < 3 {
        return (0, 0, 0.0);
    }

    let used_bytes = values[0].parse::<u64>().unwrap_or(0);

    let free_bytes = values[1].parse::<u64>().unwrap_or(0);

    let usage_percent = values[2]
        .trim_end_matches('%')
        .replace(',', ".")
        .parse::<f64>()
        .unwrap_or(0.0);

    (used_bytes, free_bytes, usage_percent.clamp(0.0, 100.0))
}

async fn read_disks() -> Vec<DiskMetrics> {
    let output = Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,SIZE,TYPE,FSTYPE,LABEL,MOUNTPOINT,MODEL",
        ])
        .output()
        .await;

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let Ok(root) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Vec::new();
    };

    let Some(devices) = root.get("blockdevices").and_then(|value| value.as_array()) else {
        return Vec::new();
    };

    let mut disks = Vec::new();

    for device in devices {
        if json_string(device.get("type")) != "disk" {
            continue;
        }

        let name = json_string(device.get("name"));
        let model = json_string(device.get("model"));
        let size_bytes = json_u64(device.get("size"));

        let fs_type = child_value(device, "fstype");

        let label = child_value(device, "label");

        let mountpoint = child_value(device, "mountpoint");

        let mounted = !mountpoint.is_empty();

        let (used_bytes, free_bytes, usage_percent) = read_mount_usage(&mountpoint).await;

        disks.push(DiskMetrics {
            name,
            model: if model.is_empty() {
                "Model bilgisi yok".to_string()
            } else {
                model
            },
            size_bytes,
            fs_type: if fs_type.is_empty() {
                "Biçimlendirilmemiş".to_string()
            } else {
                fs_type
            },
            label,
            mountpoint,
            mounted,
            used_bytes,
            free_bytes,
            usage_percent,
        });
    }

    disks
}

pub async fn system_metrics() -> Json<SystemMetrics> {
    let cpu_usage_percent = cpu_usage_percent().await;
    let memory = read_memory_metrics();
    let network = read_network_metrics();
    let disks = read_disks().await;
    let uptime_seconds = read_uptime_seconds();

    let (load_average_1m, load_average_5m, load_average_15m) = read_load_average();

    Json(SystemMetrics {
        hostname: read_hostname(),
        operating_system: read_os_name(),
        kernel_version: read_kernel_version(),
        architecture: std::env::consts::ARCH.to_string(),

        cpu_model: read_cpu_model(),
        cpu_cores: read_cpu_cores(),
        cpu_usage_percent,

        memory,

        uptime_seconds,
        load_average_1m,
        load_average_5m,
        load_average_15m,

        network,
        disks,
    })
}

