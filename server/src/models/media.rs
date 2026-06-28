use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Media {
    pub id: i64,
    pub user_id: i64,

    pub media_type: String,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub extension: String,

    pub sha256: String,
    pub size: i64,

    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration: Option<f64>,

    pub taken_at: Option<String>,
    pub uploaded_at: String,
    pub file_created_at: Option<String>,
    pub created_at: String,

    pub latitude: Option<f64>,
    pub longitude: Option<f64>,

    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<i64>,
    pub aperture: Option<f64>,
    pub shutter_speed: Option<String>,
    pub focal_length: Option<f64>,

    pub favorite: i64,
    pub deleted: i64,
}
