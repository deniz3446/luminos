use serde::Serialize;
use std::path::PathBuf;

use super::{
    allocator::DiskAllocator,
    disk::{DiskStatus, StorageDisk},
};

#[derive(Debug, Clone)]
pub struct StoragePool {
    pub name: String,
    pub disks: Vec<StorageDisk>,
    allocator: DiskAllocator,
}

#[derive(Debug, Clone, Serialize)]
pub struct PoolStatus {
    pub name: String,
    pub disk_count: usize,
    pub mounted_disk_count: usize,
    pub writable_disk_count: usize,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub allocatable_bytes: u64,
    pub usage_percent: f64,
    pub disks: Vec<DiskStatus>,
}

impl StoragePool {
    pub fn photoos_default() -> Self {
        let disks = crate::handlers::storage::photoos_data_disks()
            .into_iter()
            .enumerate()
            .map(|(index, mountpoint)| {
                let number = index + 1;
                StorageDisk::new(
                    format!("disk{number}"),
                    format!("PhotoOS Data {number}"),
                    PathBuf::from(mountpoint),
                )
            })
            .collect();

        Self {
            name: "PhotoOS Storage Pool".to_string(),
            disks,
            allocator: DiskAllocator::default(),
        }
    }

    pub async fn ensure_layout(&self) -> Result<(), std::io::Error> {
        for disk in &self.disks {
            let status = disk.status().await;
            if status.enabled && status.mounted && status.writable {
                disk.ensure_layout().await?;
            }
        }
        Ok(())
    }

    pub async fn status(&self) -> PoolStatus {
        let mut disks = Vec::new();
        for disk in &self.disks {
            disks.push(disk.status().await);
        }
        let total_bytes = disks.iter().filter(|disk| disk.mounted).map(|disk| disk.total_bytes).sum();
        let used_bytes = disks.iter().filter(|disk| disk.mounted).map(|disk| disk.used_bytes).sum();
        let available_bytes = disks.iter().filter(|disk| disk.mounted).map(|disk| disk.available_bytes).sum();
        let allocatable_bytes = disks.iter().filter(|disk| disk.mounted && disk.writable).map(|disk| disk.allocatable_bytes).sum();
        let mounted_disk_count = disks.iter().filter(|disk| disk.mounted).count();
        let writable_disk_count = disks.iter().filter(|disk| disk.mounted && disk.writable).count();
        let usage_percent = if total_bytes == 0 { 0.0 } else { (used_bytes as f64 / total_bytes as f64) * 100.0 };
        PoolStatus {
            name: self.name.clone(), disk_count: disks.len(), mounted_disk_count, writable_disk_count,
            total_bytes, used_bytes, available_bytes, allocatable_bytes,
            usage_percent: usage_percent.clamp(0.0, 100.0), disks,
        }
    }

    pub async fn choose_disk(&self, required_bytes: u64) -> Option<DiskStatus> {
        let status = self.status().await;
        self.allocator.choose(&status.disks, required_bytes).cloned()
    }
}
