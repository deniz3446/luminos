use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Photo {
    pub id: i64,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub created_at: String,
}
