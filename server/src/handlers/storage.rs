use axum::Json;

use crate::models::storage_info::StorageInfo;

pub async fn get_storage_info() -> Json<StorageInfo> {
    let output = std::process::Command::new("df")
        .arg("-B1")
        .arg("/")
        .output()
        .expect("df komutu çalışmadı");

    let text = String::from_utf8_lossy(&output.stdout);

    let line = text
        .lines()
        .nth(1)
        .unwrap_or("");

    let parts: Vec<&str> = line
        .split_whitespace()
        .collect();

    let total = parts.get(1)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let used = parts.get(2)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let free = parts.get(3)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    Json(StorageInfo {
        total_bytes: total,
        used_bytes: used,
        free_bytes: free,
    })
}
