use axum::{Json, extract::{Path, State}};
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::{models::storage_info::StorageInfo, state::AppState};

#[derive(Debug, Deserialize)]
struct LsblkResponse {
    #[serde(default)]
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
struct LsblkDevice {
    #[serde(default)]
    name: String,

    #[serde(default)]
    path: String,

    #[serde(rename = "type", default)]
    device_type: String,

    #[serde(default, deserialize_with = "deserialize_u64")]
    size: u64,

    #[serde(default)]
    fstype: Option<String>,

    #[serde(default)]
    label: Option<String>,

    #[serde(default)]
    uuid: Option<String>,

    #[serde(default)]
    model: Option<String>,

    #[serde(default)]
    mountpoints: Option<Vec<Option<String>>>,

    #[serde(default)]
    children: Vec<LsblkDevice>,
}

#[derive(Debug, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub device: String,
    pub parent_device: String,
    pub device_type: String,
    pub model: String,
    pub label: String,
    pub uuid: String,
    pub filesystem: String,
    pub mountpoint: String,

    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub usage_percent: f64,

    pub mounted: bool,
    pub system_disk: bool,
    pub photoos_data_disk: bool,
}

#[derive(Debug, Serialize)]
pub struct StorageDisksResponse {
    pub disks: Vec<DiskInfo>,

    pub physical_disk_count: usize,
    pub mounted_disk_count: usize,
    pub photoos_data_disk_count: usize,

    pub data_total_bytes: u64,
    pub data_used_bytes: u64,
    pub data_free_bytes: u64,
    pub data_usage_percent: f64,
}

#[derive(Debug, Default)]
struct FilesystemUsage {
    total_bytes: u64,
    used_bytes: u64,
    free_bytes: u64,
    usage_percent: f64,
}

fn deserialize_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::Number(number) => Ok(number.as_u64().unwrap_or(0)),

        serde_json::Value::String(text) => Ok(text.parse::<u64>().unwrap_or(0)),

        _ => Ok(0),
    }
}

fn round_percent(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn first_mountpoint(mountpoints: &Option<Vec<Option<String>>>) -> String {
    mountpoints
        .as_ref()
        .and_then(|items| items.iter().flatten().find(|item| !item.trim().is_empty()))
        .cloned()
        .unwrap_or_default()
}

fn filesystem_usage(path: &str) -> FilesystemUsage {
    if path.trim().is_empty() {
        return FilesystemUsage::default();
    }

    let output = Command::new("df")
        .args(["-B1", "--output=size,used,avail,pcent", path])
        .output();

    let Ok(output) = output else {
        return FilesystemUsage::default();
    };

    if !output.status.success() {
        return FilesystemUsage::default();
    }

    let text = String::from_utf8_lossy(&output.stdout);

    let Some(line) = text.lines().nth(1) else {
        return FilesystemUsage::default();
    };

    let parts: Vec<&str> = line.split_whitespace().collect();

    let total_bytes = parts
        .first()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    let used_bytes = parts
        .get(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    let free_bytes = parts
        .get(2)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    let usage_percent = parts
        .get(3)
        .map(|value| value.trim_end_matches('%'))
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| {
            if total_bytes > 0 {
                used_bytes as f64 / total_bytes as f64 * 100.0
            } else {
                0.0
            }
        });

    FilesystemUsage {
        total_bytes,
        used_bytes,
        free_bytes,
        usage_percent: round_percent(usage_percent),
    }
}

fn flatten_device(
    device: &LsblkDevice,
    parent_device: &str,
    parent_model: &str,
    output: &mut Vec<DiskInfo>,
) {
    let model = device
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(parent_model)
        .to_string();

    let mountpoint = first_mountpoint(&device.mountpoints);

    let mounted = !mountpoint.is_empty();

    let system_disk = mountpoint == "/" || mountpoint == "/boot";

    let photoos_data_disk = mountpoint.starts_with("/srv/photoos/disks/");

    let usage = if mounted {
        filesystem_usage(&mountpoint)
    } else {
        FilesystemUsage::default()
    };

    /*
     * Diskin kendisini ve kullanılabilir bölümlerini listeliyoruz.
     * loop, rom ve zram gibi sanal aygıtları göstermiyoruz.
     */
    if matches!(device.device_type.as_str(), "disk" | "part" | "raid" | "md") {
        output.push(DiskInfo {
            name: device.name.clone(),

            device: if device.path.is_empty() {
                format!("/dev/{}", device.name)
            } else {
                device.path.clone()
            },

            parent_device: parent_device.to_string(),

            device_type: device.device_type.clone(),

            model: model.clone(),

            label: device.label.clone().unwrap_or_default(),

            uuid: device.uuid.clone().unwrap_or_default(),

            filesystem: device.fstype.clone().unwrap_or_default(),

            mountpoint,

            total_bytes: if usage.total_bytes > 0 {
                usage.total_bytes
            } else {
                device.size
            },

            used_bytes: usage.used_bytes,
            free_bytes: usage.free_bytes,

            usage_percent: usage.usage_percent,

            mounted,
            system_disk,
            photoos_data_disk,
        });
    }

    let current_parent = if device.path.is_empty() {
        format!("/dev/{}", device.name)
    } else {
        device.path.clone()
    };

    for child in &device.children {
        flatten_device(child, &current_parent, &model, output);
    }
}

pub async fn get_storage_info(State(state): State<AppState>) -> Json<StorageInfo> {
    let status = state.storage.status().await;

    Json(StorageInfo {
        total_bytes: status.total_bytes,
        used_bytes: status.used_bytes,
        free_bytes: status.available_bytes,
        usage_percent: round_percent(status.usage_percent),
    })
}

pub async fn get_storage_disks() -> Json<StorageDisksResponse> {
    let command = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--paths",
            "--output",
            "NAME,PATH,TYPE,SIZE,FSTYPE,LABEL,UUID,MOUNTPOINTS,MODEL",
        ])
        .output();

    let mut disks = Vec::new();

    if let Ok(command_output) = command {
        if command_output.status.success() {
            if let Ok(lsblk) = serde_json::from_slice::<LsblkResponse>(&command_output.stdout) {
                for device in &lsblk.blockdevices {
                    flatten_device(device, "", "", &mut disks);
                }
            }
        }
    }

    /*
     * Önce sistem diski, ardından PhotoOS veri
     * diskleri, sonra diğer aygıtlar gösterilir.
     */
    disks.sort_by(|left, right| {
        let left_order = if left.system_disk {
            0
        } else if left.photoos_data_disk {
            1
        } else if left.device_type == "disk" {
            2
        } else {
            3
        };

        let right_order = if right.system_disk {
            0
        } else if right.photoos_data_disk {
            1
        } else if right.device_type == "disk" {
            2
        } else {
            3
        };

        left_order
            .cmp(&right_order)
            .then_with(|| left.device.cmp(&right.device))
    });

    let physical_disk_count = disks
        .iter()
        .filter(|disk| disk.device_type == "disk")
        .count();

    let mounted_disk_count = disks.iter().filter(|disk| disk.mounted).count();

    let photoos_data_disk_count = disks.iter().filter(|disk| disk.photoos_data_disk).count();

    /*
     * Toplam kapasite hesabında yalnızca
     * /srv/photoos/disks altına bağlı veri
     * bölümleri kullanılır. Böylece fiziksel disk ve
     * bölüm kapasitesi iki kez toplanmaz.
     */
    let data_total_bytes = disks
        .iter()
        .filter(|disk| disk.photoos_data_disk)
        .map(|disk| disk.total_bytes)
        .sum::<u64>();

    let data_used_bytes = disks
        .iter()
        .filter(|disk| disk.photoos_data_disk)
        .map(|disk| disk.used_bytes)
        .sum::<u64>();

    let data_free_bytes = disks
        .iter()
        .filter(|disk| disk.photoos_data_disk)
        .map(|disk| disk.free_bytes)
        .sum::<u64>();

    let data_usage_percent = if data_total_bytes > 0 {
        round_percent(data_used_bytes as f64 / data_total_bytes as f64 * 100.0)
    } else {
        0.0
    };

    Json(StorageDisksResponse {
        disks,

        physical_disk_count,
        mounted_disk_count,
        photoos_data_disk_count,

        data_total_bytes,
        data_used_bytes,
        data_free_bytes,
        data_usage_percent,
    })
}

#[derive(Debug, Serialize)]
pub struct SmartDiskHealth {
    pub device: String,
    pub model: String,
    pub serial: String,
    pub smart_available: bool,
    pub smart_passed: bool,
    pub status: String,
    pub temperature_celsius: Option<i64>,
    pub power_on_hours: Option<i64>,
    pub reallocated_sectors: Option<i64>,
    pub pending_sectors: Option<i64>,
    pub offline_uncorrectable: Option<i64>,
    pub crc_errors: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RaidHealth {
    pub configured: bool,
    pub status: String,
    pub details: String,
}

#[derive(Debug, Serialize)]
pub struct StorageHealthResponse {
    pub disks: Vec<SmartDiskHealth>,
    pub raid: RaidHealth,
    pub checked_at: String,
}

fn json_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|item| i64::try_from(item).ok()))
        .or_else(|| value.as_str().and_then(|item| item.parse::<i64>().ok()))
}

fn smart_attribute_raw(smart: &serde_json::Value, attribute_id: i64) -> Option<i64> {
    smart
        .get("ata_smart_attributes")
        .and_then(|value| value.get("table"))
        .and_then(serde_json::Value::as_array)
        .and_then(|attributes| {
            attributes.iter().find_map(|attribute| {
                let id = attribute.get("id").and_then(json_i64)?;

                if id != attribute_id {
                    return None;
                }

                attribute
                    .get("raw")
                    .and_then(|raw| raw.get("value"))
                    .and_then(json_i64)
            })
        })
}

fn smart_temperature(smart: &serde_json::Value) -> Option<i64> {
    smart
        .get("temperature")
        .and_then(|value| value.get("current"))
        .and_then(json_i64)
        .or_else(|| smart_attribute_raw(smart, 194))
}

fn smart_power_on_hours(smart: &serde_json::Value) -> Option<i64> {
    smart
        .get("power_on_time")
        .and_then(|value| value.get("hours"))
        .and_then(json_i64)
        .or_else(|| smart_attribute_raw(smart, 9))
}

fn physical_disks() -> Vec<(String, String, String)> {
    let output = Command::new("lsblk")
        .args([
            "--json",
            "--nodeps",
            "--paths",
            "--output",
            "PATH,TYPE,MODEL,SERIAL",
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Vec::new();
    };

    value
        .get("blockdevices")
        .and_then(serde_json::Value::as_array)
        .map(|devices| {
            devices
                .iter()
                .filter_map(|device| {
                    let device_type = device.get("type")?.as_str()?;

                    if device_type != "disk" {
                        return None;
                    }

                    let path = device
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string();

                    if path.is_empty() {
                        return None;
                    }

                    let model = device
                        .get("model")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    let serial = device
                        .get("serial")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    Some((path, model, serial))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn read_smart_health(device: &str, fallback_model: &str, fallback_serial: &str) -> SmartDiskHealth {
    let output = Command::new("/usr/sbin/smartctl")
        .args(["-a", "-j", device])
        .output();

    let Ok(output) = output else {
        return SmartDiskHealth {
            device: device.to_string(),
            model: fallback_model.to_string(),
            serial: fallback_serial.to_string(),
            smart_available: false,
            smart_passed: false,
            status: "SMART okunamadı".to_string(),
            temperature_celsius: None,
            power_on_hours: None,
            reallocated_sectors: None,
            pending_sectors: None,
            offline_uncorrectable: None,
            crc_errors: None,
        };
    };

    let smart = match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
        Ok(value) => value,

        Err(error) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

            eprintln!(
                "SMART JSON HATASI device={} status={:?} parse={} stderr={} stdout={}",
                device,
                output.status.code(),
                error,
                stderr,
                stdout,
            );

            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("Çıktı alınamadı, exit={:?}", output.status.code())
            };

            return SmartDiskHealth {
                device: device.to_string(),
                model: fallback_model.to_string(),
                serial: fallback_serial.to_string(),
                smart_available: false,
                smart_passed: false,
                status: format!("SMART hatası: {}", detail),
                temperature_celsius: None,
                power_on_hours: None,
                reallocated_sectors: None,
                pending_sectors: None,
                offline_uncorrectable: None,
                crc_errors: None,
            };
        }
    };

    let model = smart
        .get("model_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(fallback_model)
        .trim()
        .to_string();

    let serial = smart
        .get("serial_number")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(fallback_serial)
        .trim()
        .to_string();

    let smart_available = smart
        .get("smart_support")
        .and_then(|value| value.get("available"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let smart_passed = smart
        .get("smart_status")
        .and_then(|value| value.get("passed"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let reallocated_sectors = smart_attribute_raw(&smart, 5);

    let pending_sectors = smart_attribute_raw(&smart, 197);

    let offline_uncorrectable = smart_attribute_raw(&smart, 198);

    let crc_errors = smart_attribute_raw(&smart, 199);

    let has_sector_warning = reallocated_sectors.unwrap_or(0) > 0
        || pending_sectors.unwrap_or(0) > 0
        || offline_uncorrectable.unwrap_or(0) > 0;

    let status = if !smart_available {
        "SMART desteklenmiyor".to_string()
    } else if smart_passed && !has_sector_warning {
        "Sağlıklı".to_string()
    } else if smart_passed {
        "Uyarı".to_string()
    } else {
        "Arızalı".to_string()
    };

    SmartDiskHealth {
        device: device.to_string(),
        model,
        serial,
        smart_available,
        smart_passed,
        status,
        temperature_celsius: smart_temperature(&smart),
        power_on_hours: smart_power_on_hours(&smart),
        reallocated_sectors,
        pending_sectors,
        offline_uncorrectable,
        crc_errors,
    }
}

fn read_raid_health() -> RaidHealth {
    let text = std::fs::read_to_string("/proc/mdstat").unwrap_or_default();

    let configured = text.lines().any(|line| {
        let trimmed = line.trim();

        !trimmed.is_empty()
            && !trimmed.starts_with("Personalities")
            && !trimmed.starts_with("unused devices")
            && trimmed.contains(" : ")
    });

    if configured {
        let degraded = text.contains("[_") || text.contains("_]") || text.contains("inactive");

        RaidHealth {
            configured: true,
            status: if degraded {
                "Uyarı".to_string()
            } else {
                "Sağlıklı".to_string()
            },
            details: text.trim().to_string(),
        }
    } else {
        RaidHealth {
            configured: false,
            status: "RAID yapılandırılmamış".to_string(),
            details: "Bağımsız diskler kullanılıyor.".to_string(),
        }
    }
}

pub async fn get_storage_health() -> Json<StorageHealthResponse> {
    let disks = physical_disks()
        .into_iter()
        /*
         * USB sistem belleği SMART desteklemeyebilir.
         * Yine de tüm fiziksel diskleri listelemeyi
         * tercih ediyoruz.
         */
        .map(|(device, model, serial)| read_smart_health(&device, &model, &serial))
        .collect();

    let checked_at = Command::new("date")
        .arg("--iso-8601=seconds")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();

    Json(StorageHealthResponse {
        disks,
        raid: read_raid_health(),
        checked_at,
    })
}

// PHOTOOS_SMART_TEST_API_V1

#[derive(Debug, Serialize)]
pub struct SmartTestResponse {
    pub device: String,
    pub success: bool,
    pub message: String,
}

fn resolve_physical_device(requested_device: &str) -> Option<String> {
    /*
     * İstemciden yalnızca sdb, sdc veya nvme0n1
     * biçiminde aygıt adı kabul edilir.
     */
    if requested_device.is_empty()
        || !requested_device.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return None;
    }

    let requested_path = format!("/dev/{requested_device}");

    /*
     * Aygıtın gerçekten fiziksel disk listesinde
     * bulunduğunu doğrular. Bölümler ve rastgele
     * dosya yolları kabul edilmez.
     */
    physical_disks()
        .into_iter()
        .map(|(device, _, _)| device)
        .find(|device| device == &requested_path)
}

fn smartctl_message(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout_text = String::from_utf8_lossy(stdout).trim().to_string();

    let stderr_text = String::from_utf8_lossy(stderr).trim().to_string();

    if !stdout_text.is_empty() {
        stdout_text
    } else if !stderr_text.is_empty() {
        stderr_text
    } else {
        "smartctl çıktı üretmedi.".to_string()
    }
}

pub async fn start_smart_test(Path(device): Path<String>) -> Json<SmartTestResponse> {
    let Some(device_path) = resolve_physical_device(&device) else {
        return Json(SmartTestResponse {
            device,
            success: false,
            message: "Geçersiz veya bulunamayan fiziksel disk.".to_string(),
        });
    };

    let output = Command::new("/usr/sbin/smartctl")
        .args(["-t", "short", &device_path])
        .output();

    match output {
        Ok(output) => {
            let message = smartctl_message(&output.stdout, &output.stderr);

            Json(SmartTestResponse {
                device: device_path,
                success: output.status.success(),
                message,
            })
        }

        Err(error) => Json(SmartTestResponse {
            device: device_path,
            success: false,
            message: format!("SMART testi başlatılamadı: {error}"),
        }),
    }
}

pub async fn get_smart_test_status(Path(device): Path<String>) -> Json<serde_json::Value> {
    let Some(device_path) = resolve_physical_device(&device) else {
        return Json(serde_json::json!({
            "success": false,
            "device": device,
            "message":
                "Geçersiz veya bulunamayan fiziksel disk."
        }));
    };

    /*
     * -c: Devam eden test durumunu verir.
     * -l selftest: Önceki test kayıtlarını verir.
     * -j: JSON çıktı üretir.
     */
    let output = Command::new("/usr/sbin/smartctl")
        .args(["-j", "-c", "-l", "selftest", &device_path])
        .output();

    match output {
        Ok(output) => match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            Ok(mut value) => {
                if let Some(object) = value.as_object_mut() {
                    object.insert("photoos_success".to_string(), serde_json::json!(true));

                    object.insert("photoos_device".to_string(), serde_json::json!(device_path));

                    object.insert(
                        "photoos_exit_code".to_string(),
                        serde_json::json!(output.status.code()),
                    );
                }

                Json(value)
            }

            Err(error) => {
                let message = smartctl_message(&output.stdout, &output.stderr);

                Json(serde_json::json!({
                    "success": false,
                    "device": device_path,
                    "message": format!(
                        "SMART test sonucu okunamadı: {}",
                        error
                    ),
                    "smartctl_output": message
                }))
            }
        },

        Err(error) => Json(serde_json::json!({
            "success": false,
            "device": device_path,
            "message": format!(
                "smartctl çalıştırılamadı: {}",
                error
            )
        })),
    }
}

pub fn photoos_data_disks() -> Vec<String> {
    let storage = crate::config::storage_config();
    [storage.disk1, storage.disk2]
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .collect()
}
