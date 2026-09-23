use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{config::Settings, storage::StorageEngine};

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub db: SqlitePool,
    pub storage: Arc<StorageEngine>,
}
