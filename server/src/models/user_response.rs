use serde::{Serialize, Deserialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub created_at: String,
}
