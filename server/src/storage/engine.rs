use serde::Serialize;
use std::{fmt, path::PathBuf};

use super::{
    disk::DiskStatus,
    pool::{PoolStatus, StoragePool},
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Photo,
    Video,
    Thumbnail,
    Temporary,
    Trash,
    Metadata,
}

impl MediaKind {
    pub fn directory_name(self) -> &'static str {
        match self {
            Self::Photo => "photos",
            Self::Video => "videos",
            Self::Thumbnail => "thumbs",
            Self::Temporary => "tmp",
            Self::Trash => "trash",
            Self::Metadata => "metadata",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageTarget {
    pub disk_id: String,
    pub disk_label: String,
    pub mountpoint: PathBuf,

    pub media_kind: MediaKind,
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,

    pub available_bytes_before_write: u64,
}

#[derive(Debug)]
pub enum StorageError {
    NoWritableDisk { required_bytes: u64 },

    InvalidFilename,

    Io(std::io::Error),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWritableDisk { required_bytes } => write!(
                formatter,
                "Yazılabilir disk bulunamadı. Gerekli alan: {required_bytes} bayt"
            ),
            Self::InvalidFilename => write!(formatter, "Geçersiz dosya adı"),
            Self::Io(error) => write!(formatter, "Depolama I/O hatası: {error}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone)]
pub struct StorageEngine {
    pool: StoragePool,
}

impl Default for StorageEngine {
    fn default() -> Self {
        Self::photoos_default()
    }
}

impl StorageEngine {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub fn photoos_default() -> Self {
        Self {
            pool: StoragePool::photoos_default(),
        }
    }

    pub async fn initialize(&self) -> Result<(), StorageError> {
        self.pool.ensure_layout().await?;
        Ok(())
    }

    pub async fn status(&self) -> PoolStatus {
        self.pool.status().await
    }

    pub async fn allocate(
        &self,
        media_kind: MediaKind,
        filename: &str,
        required_bytes: u64,
    ) -> Result<StorageTarget, StorageError> {
        validate_filename(filename)?;

        let disk = self
            .pool
            .choose_disk(required_bytes)
            .await
            .ok_or(StorageError::NoWritableDisk { required_bytes })?;

        Ok(create_target(&disk, media_kind, filename))
    }
}

fn validate_filename(filename: &str) -> Result<(), StorageError> {
    if filename.trim().is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename == "."
        || filename == ".."
    {
        return Err(StorageError::InvalidFilename);
    }
    Ok(())
}

fn create_target(disk: &DiskStatus, media_kind: MediaKind, filename: &str) -> StorageTarget {
    let relative_path = PathBuf::from(media_kind.directory_name()).join(filename);
    let absolute_path = disk.mountpoint.join(&relative_path);
    StorageTarget {
        disk_id: disk.id.clone(),
        disk_label: disk.label.clone(),
        mountpoint: disk.mountpoint.clone(),
        media_kind,
        relative_path,
        absolute_path,
        available_bytes_before_write: disk.available_bytes,
    }
}
