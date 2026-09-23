use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, process::Command};

#[derive(Debug, Serialize)]
pub struct RaidArray {
    pub device: String,
    pub name: String,
    pub level: String,
    pub state: String,
    pub active_devices: usize,
    pub total_devices: usize,
    pub failed_devices: usize,
    pub spare_devices: usize,
    pub sync_action: Option<String>,
    pub sync_percent: Option<f64>,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct RaidStatusResponse {
    pub configured: bool,
    pub healthy: bool,
    pub status: String,
    pub arrays: Vec<RaidArray>,
    pub mdstat: String,
}

#[derive(Debug, Serialize)]
pub struct RaidDiskCandidate {
    pub device: String,
    pub model: String,
    pub serial: String,
    pub size_bytes: u64,
    pub mounted: bool,
    pub mountpoints: Vec<String>,
    pub system_disk: bool,
    pub photoos_disk: bool,
    pub eligible: bool,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct RaidPlanRequest {
    pub level: String,
    pub devices: Vec<String>,
    pub array_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RaidPlanResponse {
    pub valid: bool,
    pub destructive: bool,
    pub level: String,
    pub devices: Vec<String>,
    pub array_device: String,
    pub estimated_usable_bytes: u64,
    pub warnings: Vec<String>,
    pub command_preview: String,
    pub execution_enabled: bool,
}

fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn parse_mdstat() -> (Vec<RaidArray>, String) {
    let mdstat = std::fs::read_to_string("/proc/mdstat").unwrap_or_default();
    let mut arrays = Vec::new();
    let lines: Vec<&str> = mdstat.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("md") && line.contains(" : ") {
            let mut parts = line.split_whitespace();
            let name = parts.next().unwrap_or("md?").to_string();
            let _colon = parts.next();
            let state_word = parts.next().unwrap_or("inactive").to_string();
            let level = parts.next().unwrap_or("unknown").to_string();
            let members: Vec<String> = parts
                .filter(|p| p.contains('[') && !p.starts_with('['))
                .map(|p| p.split('[').next().unwrap_or(p).to_string())
                .collect();

            let detail_line = lines.get(i + 1).map(|x| x.trim()).unwrap_or("");
            let bitmap_line = lines.get(i + 2).map(|x| x.trim()).unwrap_or("");
            let combined = format!("{} {}", detail_line, bitmap_line);

            let counts = detail_line
                .split_whitespace()
                .find(|p| p.starts_with('[') && p.contains('/'))
                .map(|p| p.trim_matches(&['[', ']'][..]))
                .unwrap_or("0/0");
            let mut count_parts = counts.split('/');
            let total_devices = count_parts.next().and_then(|x| x.parse().ok()).unwrap_or(members.len());
            let active_devices = count_parts.next().and_then(|x| x.parse().ok()).unwrap_or(members.len());

            let health_map = detail_line
                .split_whitespace()
                .find(|p| p.starts_with('[') && !p.contains('/'))
                .unwrap_or("");
            let failed_devices = health_map.chars().filter(|c| *c == '_').count();

            let sync_action = ["recovery", "resync", "reshape", "check"]
                .iter()
                .find(|key| combined.contains(**key))
                .map(|x| x.to_string());
            let sync_percent = combined
                .split_whitespace()
                .find(|p| p.ends_with('%'))
                .and_then(|p| p.trim_end_matches('%').parse::<f64>().ok());

            arrays.push(RaidArray {
                device: format!("/dev/{name}"),
                name,
                level,
                state: state_word,
                active_devices,
                total_devices,
                failed_devices,
                spare_devices: 0,
                sync_action,
                sync_percent,
                detail: line.to_string(),
            });
        }
        i += 1;
    }

    (arrays, mdstat)
}

pub async fn get_raid_manager_status() -> Json<RaidStatusResponse> {
    let (arrays, mdstat) = parse_mdstat();
    let configured = !arrays.is_empty();
    let healthy = configured && arrays.iter().all(|a| a.failed_devices == 0 && a.active_devices >= a.total_devices);
    let status = if !configured {
        "RAID yapılandırılmamış".to_string()
    } else if healthy {
        "RAID dizileri sağlıklı".to_string()
    } else {
        "RAID dizilerinden biri dikkat gerektiriyor".to_string()
    };
    Json(RaidStatusResponse { configured, healthy, status, arrays, mdstat })
}

pub async fn get_raid_candidates() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let output = Command::new("lsblk")
        .args(["-J", "-b", "-o", "NAME,PATH,TYPE,SIZE,MODEL,SERIAL,MOUNTPOINTS,PKNAME"])
        .output()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if !output.status.success() {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "lsblk çalıştırılamadı"}))));
    }

    let root_source = command_output("findmnt", &["-n", "-o", "SOURCE", "/"]);
    let root_parent = if root_source.starts_with("/dev/") {
        command_output("lsblk", &["-no", "PKNAME", &root_source])
    } else { String::new() };

    let raw: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    let mut photoos_devices = HashSet::new();
    for mount in ["/srv/photoos/disks/disk1", "/srv/photoos/disks/disk2"] {
        let src = command_output("findmnt", &["-n", "-o", "SOURCE", mount]);
        if src.starts_with("/dev/") {
            let parent = command_output("lsblk", &["-no", "PKNAME", &src]);
            photoos_devices.insert(if parent.is_empty() { src.trim_start_matches("/dev/").to_string() } else { parent });
        }
    }

    let mut candidates = Vec::new();
    for disk in raw.get("blockdevices").and_then(Value::as_array).cloned().unwrap_or_default() {
        if disk.get("type").and_then(Value::as_str) != Some("disk") { continue; }
        let name = disk.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let device = disk.get("path").and_then(Value::as_str).unwrap_or("").to_string();
        let mut mountpoints = Vec::new();
        collect_mountpoints(&disk, &mut mountpoints);
        let mounted = !mountpoints.is_empty();
        let system_disk = name == root_parent || root_source == device;
        let photoos_disk = photoos_devices.contains(&name);
        let eligible = !mounted && !system_disk && !photoos_disk;
        let reason = if system_disk {
            "Sistem diski; kullanılamaz".to_string()
        } else if photoos_disk {
            "PhotoOS veri diski; önce yedekleme ve taşıma gerekir".to_string()
        } else if mounted {
            format!("Bağlı bölüm var: {}", mountpoints.join(", "))
        } else {
            "RAID için uygun, boş ve bağlı olmayan fiziksel disk".to_string()
        };
        candidates.push(RaidDiskCandidate {
            device,
            model: disk.get("model").and_then(Value::as_str).unwrap_or("Bilinmiyor").trim().to_string(),
            serial: disk.get("serial").and_then(Value::as_str).unwrap_or("").trim().to_string(),
            size_bytes: disk.get("size").and_then(Value::as_u64).unwrap_or(0),
            mounted,
            mountpoints,
            system_disk,
            photoos_disk,
            eligible,
            reason,
        });
    }

    Ok(Json(json!({"disks": candidates})))
}

fn collect_mountpoints(device: &Value, out: &mut Vec<String>) {
    if let Some(points) = device.get("mountpoints").and_then(Value::as_array) {
        for point in points {
            if let Some(p) = point.as_str() { if !p.is_empty() { out.push(p.to_string()); } }
        }
    }
    if let Some(children) = device.get("children").and_then(Value::as_array) {
        for child in children { collect_mountpoints(child, out); }
    }
}

pub async fn create_raid_plan(Json(req): Json<RaidPlanRequest>) -> Result<Json<RaidPlanResponse>, (StatusCode, Json<Value>)> {
    let valid_levels = ["raid0", "raid1", "raid5", "raid6", "raid10"];
    if !valid_levels.contains(&req.level.as_str()) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Desteklenmeyen RAID seviyesi"}))));
    }
    let minimum = match req.level.as_str() { "raid0" | "raid1" => 2, "raid5" => 3, "raid6" | "raid10" => 4, _ => 99 };
    if req.devices.len() < minimum {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": format!("{} için en az {} disk gerekir", req.level, minimum)}))));
    }
    if req.devices.iter().any(|d| !d.starts_with("/dev/")) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Geçersiz disk yolu"}))));
    }
    let array_name = req.array_name.unwrap_or_else(|| "md0".to_string());
    let array_device = format!("/dev/{}", array_name.trim_start_matches("/dev/"));
    let sizes: Vec<u64> = req.devices.iter().map(|d| command_output("blockdev", &["--getsize64", d]).parse().unwrap_or(0)).collect();
    let smallest = sizes.iter().copied().filter(|x| *x > 0).min().unwrap_or(0);
    let n = req.devices.len() as u64;
    let estimated = match req.level.as_str() { "raid0" => smallest*n, "raid1" => smallest, "raid5" => smallest*(n-1), "raid6" => smallest*(n-2), "raid10" => smallest*(n/2), _ => 0 };
    let command_preview = format!("sudo mdadm --create {} --level={} --raid-devices={} {}", array_device, req.level.trim_start_matches("raid"), req.devices.len(), req.devices.join(" "));
    Ok(Json(RaidPlanResponse {
        valid: true,
        destructive: true,
        level: req.level,
        devices: req.devices,
        array_device,
        estimated_usable_bytes: estimated,
        warnings: vec![
            "Bu işlem seçilen disklerdeki tüm verileri kalıcı olarak siler.".to_string(),
            "PhotoOS1 ve PhotoOS2 şu anda veri diski olduğundan otomatik seçime kapalıdır.".to_string(),
            "V1 güvenlik nedeniyle yalnızca plan üretir; web arayüzünden mdadm çalıştırmaz.".to_string(),
        ],
        command_preview,
        execution_enabled: false,
    }))
}
