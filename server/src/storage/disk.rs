use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::{fs, process::Command};

#[derive(Debug, Clone)]
pub struct StorageDisk {
    pub id: String,
    pub label: String,
    pub mountpoint: PathBuf,
    pub reserve_bytes: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskStatus {
    pub id: String,
    pub label: String,
    pub mountpoint: PathBuf,
    pub enabled: bool,
    pub mounted: bool,
    pub writable: bool,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
    pub allocatable_bytes: u64,
}

impl StorageDisk {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        mountpoint: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            mountpoint: mountpoint.into(),
            reserve_bytes: 0,
            enabled: true,
        }
    }

    pub fn with_reserve_bytes(mut self, reserve_bytes: u64) -> Self {
        self.reserve_bytes = reserve_bytes;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub async fn status(&self) -> DiskStatus {
        let mounted = is_mountpoint(&self.mountpoint).await;
        let writable = if self.enabled && mounted { test_writable(&self.mountpoint).await } else { false };
        let (total_bytes, used_bytes, available_bytes, usage_percent) = if mounted {
            read_df_usage(&self.mountpoint).await.unwrap_or((0, 0, 0, 0.0))
        } else {
            (0, 0, 0, 0.0)
        };
        let allocatable_bytes = available_bytes.saturating_sub(self.reserve_bytes);
        DiskStatus {
            id: self.id.clone(), label: self.label.clone(), mountpoint: self.mountpoint.clone(),
            enabled: self.enabled, mounted, writable, total_bytes, used_bytes, available_bytes,
            usage_percent, allocatable_bytes,
        }
    }

    pub async fn ensure_layout(&self) -> Result<(), std::io::Error> {
        for directory in ["photos", "videos", "thumbs", "tmp", "trash", "metadata"] {
            fs::create_dir_all(self.mountpoint.join(directory)).await?;
        }
        Ok(())
    }
}

async fn is_mountpoint(path: &Path) -> bool {
    Command::new("findmnt")
        .args(["-n", "--mountpoint"])
        .arg(path)
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn test_writable(path: &Path) -> bool {
    let probe = path.join(".photoos-write-test");
    match fs::write(&probe, b"").await {
        Ok(()) => {
            let _ = fs::remove_file(probe).await;
            true
        }
        Err(_) => false,
    }
}

async fn read_df_usage(path: &Path) -> Option<(u64, u64, u64, f64)> {
    let output = Command::new("df")
        .arg("-B1")
        .arg("--output=size,used,avail,pcent")
        .arg(path)
        .output()
        .await
        .ok()?;
    if !output.status.success() { return None; }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let columns: Vec<&str> = line.split_whitespace().collect();
    if columns.len() < 4 { return None; }
    let total_bytes = columns[0].parse::<u64>().ok()?;
    let used_bytes = columns[1].parse::<u64>().ok()?;
    let available_bytes = columns[2].parse::<u64>().ok()?;
    let usage_percent = columns[3].trim_end_matches('%').replace(',', ".").parse::<f64>().ok()?.clamp(0.0, 100.0);
    Some((total_bytes, used_bytes, available_bytes, usage_percent))
}
