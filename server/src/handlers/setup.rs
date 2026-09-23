use argon2::{Argon2, password_hash::{PasswordHasher, SaltString, rand_core::OsRng}};
use axum::{Json, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Row, SqlitePool};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SetupStatusResponse { pub initialized: bool, pub user_count: i64, pub requires_setup: bool, pub preferences_saved: bool }
#[derive(Debug, Deserialize)]
pub struct InitializeSetupRequest {
    pub username: String, pub email: String, pub password: String,
    #[serde(default = "default_server_name", alias="serverName")] pub server_name: String,
    #[serde(default = "default_hostname")] pub hostname: String,
    #[serde(default = "default_timezone")] pub timezone: String,
    #[serde(default = "default_language")] pub language: String,
    #[serde(default, alias="selectedDisks")] pub selected_disks: Vec<String>,
    #[serde(default = "default_raid_type", alias="raidType")] pub raid_type: String,
    #[serde(default = "default_true", alias="webNotifications")] pub web_notifications: bool,
    #[serde(default = "default_true", alias="emailNotifications")] pub email_notifications: bool,
    #[serde(default, alias="criticalOnly")] pub critical_only: bool,
}
#[derive(Debug, Serialize)]
pub struct InitializeSetupResponse { pub success: bool, pub message: String, pub username: String, pub role: String, pub preferences_saved: bool }
#[derive(Debug, Serialize)]
pub struct SetupPreferencesResponse {
    pub configured: bool, pub server_name: String, pub hostname: String, pub timezone: String, pub language: String,
    pub selected_disks: Vec<String>, pub raid_type: String, pub web_notifications: bool,
    pub email_notifications: bool, pub critical_only: bool, pub setup_version: String,
    pub created_at: Option<String>, pub updated_at: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct UpdateSetupPreferencesRequest {
    #[serde(alias="serverName")] pub server_name: String, pub hostname: String, pub timezone: String, pub language: String,
    #[serde(default = "default_true", alias="webNotifications")] pub web_notifications: bool,
    #[serde(default = "default_true", alias="emailNotifications")] pub email_notifications: bool,
    #[serde(default, alias="criticalOnly")] pub critical_only: bool,
}
#[derive(Debug, Serialize)]
pub struct UpdateSetupPreferencesResponse { pub success: bool, pub message: String, pub preferences: SetupPreferencesResponse }

fn default_server_name() -> String { "PhotoOS".to_string() }
fn default_hostname() -> String { "photoos".to_string() }
fn default_timezone() -> String { "Europe/Istanbul".to_string() }
fn default_language() -> String { "tr-TR".to_string() }
fn default_raid_type() -> String { "basic".to_string() }
fn default_true() -> bool { true }
fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"success": false, "error": code, "message": message}))).into_response()
}
async fn ensure_users_schema(db: &SqlitePool) -> Result<(), sqlx::Error> {
    let columns = sqlx::query("PRAGMA table_info(users)").fetch_all(db).await?;
    let has_role = columns.iter().any(|r| r.try_get::<String,_>("name").map(|n| n == "role").unwrap_or(false));
    if !has_role { sqlx::query("ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user'").execute(db).await?; }
    Ok(())
}
async fn ensure_setup_preferences_schema(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(r#"CREATE TABLE IF NOT EXISTS setup_preferences (
        id INTEGER PRIMARY KEY CHECK (id = 1), server_name TEXT NOT NULL DEFAULT 'PhotoOS', hostname TEXT NOT NULL DEFAULT 'photoos',
        timezone TEXT NOT NULL DEFAULT 'Europe/Istanbul', language TEXT NOT NULL DEFAULT 'tr-TR', selected_disks_json TEXT NOT NULL DEFAULT '[]',
        raid_type TEXT NOT NULL DEFAULT 'basic', web_notifications_enabled INTEGER NOT NULL DEFAULT 1,
        email_notifications_enabled INTEGER NOT NULL DEFAULT 1, critical_only INTEGER NOT NULL DEFAULT 0, setup_version TEXT NOT NULL DEFAULT '2.1',
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"#).execute(db).await?;
    Ok(())
}
async fn ensure_setup_schema(db: &SqlitePool) -> Result<(), sqlx::Error> { ensure_users_schema(db).await?; ensure_setup_preferences_schema(db).await }
async fn user_count(db: &SqlitePool) -> Result<i64, sqlx::Error> { sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(db).await }
async fn preferences_count(db: &SqlitePool) -> Result<i64, sqlx::Error> { sqlx::query_scalar("SELECT COUNT(*) FROM setup_preferences WHERE id=1").fetch_one(db).await }
fn normalize_server_name(v:&str)->String { let s=v.trim(); if s.is_empty(){default_server_name()}else{s.to_string()} }
fn normalize_hostname(v:&str)->String { v.trim().to_lowercase() }
fn validate_hostname(v:&str)->bool { !v.is_empty() && v.len()<=63 && !v.starts_with('-') && !v.ends_with('-') && v.chars().all(|c| c.is_ascii_lowercase()||c.is_ascii_digit()||c=='-') }
fn validate_timezone(v:&str)->bool { !v.trim().is_empty() && v.len()<=64 && v.chars().all(|c| c.is_ascii_alphanumeric()||matches!(c,'/'|'_'|'-'|'+')) }
fn validate_language(v:&str)->bool { matches!(v,"tr-TR"|"en-US"|"de-DE") }
fn validate_raid_type(v:&str)->bool { matches!(v,"basic"|"raid1"|"raid5"|"raid6"|"jbod") }
fn row_to_preferences(row: &sqlx::sqlite::SqliteRow) -> SetupPreferencesResponse {
    let selected_disks_json = row.try_get::<String,_>("selected_disks_json").unwrap_or_else(|_| "[]".to_string());
    SetupPreferencesResponse {
        configured: true,
        server_name: row.try_get("server_name").unwrap_or_else(|_| default_server_name()),
        hostname: row.try_get("hostname").unwrap_or_else(|_| default_hostname()),
        timezone: row.try_get("timezone").unwrap_or_else(|_| default_timezone()),
        language: row.try_get("language").unwrap_or_else(|_| default_language()),
        selected_disks: serde_json::from_str(&selected_disks_json).unwrap_or_default(),
        raid_type: row.try_get("raid_type").unwrap_or_else(|_| default_raid_type()),
        web_notifications: row.try_get::<i64,_>("web_notifications_enabled").unwrap_or(1) != 0,
        email_notifications: row.try_get::<i64,_>("email_notifications_enabled").unwrap_or(1) != 0,
        critical_only: row.try_get::<i64,_>("critical_only").unwrap_or(0) != 0,
        setup_version: row.try_get("setup_version").unwrap_or_else(|_| "2.1".to_string()),
        created_at: row.try_get("created_at").ok(), updated_at: row.try_get("updated_at").ok(),
    }
}
fn default_preferences() -> SetupPreferencesResponse {
    SetupPreferencesResponse { configured:false, server_name:default_server_name(), hostname:default_hostname(), timezone:default_timezone(), language:default_language(),
        selected_disks:Vec::new(), raid_type:default_raid_type(), web_notifications:true, email_notifications:true,
        critical_only:false, setup_version:"2.1".to_string(), created_at:None, updated_at:None }
}
pub async fn setup_status(State(state): State<AppState>) -> Response {
    if let Err(e)=ensure_setup_schema(&state.db).await { eprintln!("Setup schema check error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"setup_schema_failed","Kurulum veritabanı hazırlanamadı."); }
    let count=match user_count(&state.db).await { Ok(v)=>v, Err(e)=>{ eprintln!("Setup status database error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"setup_status_failed","Kurulum durumu okunamadı."); }};
    let preference_count=preferences_count(&state.db).await.unwrap_or(0);
    Json(SetupStatusResponse{initialized:count>0,user_count:count,requires_setup:count==0,preferences_saved:preference_count>0}).into_response()
}

pub async fn get_setup_preferences(State(state): State<AppState>) -> Response {
    if let Err(e)=ensure_setup_schema(&state.db).await { eprintln!("Setup preferences schema error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"setup_schema_failed","Kurulum tercihleri veritabanı hazırlanamadı."); }
    let row=match sqlx::query(r#"SELECT server_name,hostname,timezone,language,selected_disks_json,raid_type,web_notifications_enabled,email_notifications_enabled,critical_only,setup_version,created_at,updated_at FROM setup_preferences WHERE id=1"#).fetch_optional(&state.db).await {
        Ok(v)=>v, Err(e)=>{eprintln!("Setup preferences read error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"preferences_read_failed","Kurulum tercihleri okunamadı.");}
    };
    match row { Some(r)=>Json(row_to_preferences(&r)).into_response(), None=>Json(default_preferences()).into_response() }
}

pub async fn update_setup_preferences(State(state): State<AppState>, Json(request): Json<UpdateSetupPreferencesRequest>) -> Response {
    if let Err(e)=ensure_setup_schema(&state.db).await { eprintln!("Settings schema check error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"setup_schema_failed","Ayar veritabanı hazırlanamadı."); }
    let server_name=normalize_server_name(&request.server_name); let hostname=normalize_hostname(&request.hostname);
    let timezone=request.timezone.trim().to_string(); let language=request.language.trim().to_string();
    if server_name.len()>80 { return error_response(StatusCode::BAD_REQUEST,"invalid_server_name","Sunucu adı en fazla 80 karakter olabilir."); }
    if !validate_hostname(&hostname) { return error_response(StatusCode::BAD_REQUEST,"invalid_hostname","Hostname yalnızca küçük harf, rakam ve tire içerebilir."); }
    if !validate_timezone(&timezone) { return error_response(StatusCode::BAD_REQUEST,"invalid_timezone","Geçerli bir saat dilimi seçin."); }
    if !validate_language(&language) { return error_response(StatusCode::BAD_REQUEST,"invalid_language","Desteklenmeyen dil seçimi."); }
    let save=sqlx::query(r#"INSERT INTO setup_preferences (id,server_name,hostname,timezone,language,selected_disks_json,raid_type,web_notifications_enabled,email_notifications_enabled,critical_only,setup_version,created_at,updated_at)
        VALUES (1,?,?,?,?, '[]','basic',?,?,?, '2.2',datetime('now'),datetime('now'))
        ON CONFLICT(id) DO UPDATE SET server_name=excluded.server_name,hostname=excluded.hostname,timezone=excluded.timezone,language=excluded.language,
        web_notifications_enabled=excluded.web_notifications_enabled,
        email_notifications_enabled=excluded.email_notifications_enabled,critical_only=excluded.critical_only,setup_version=excluded.setup_version,updated_at=datetime('now')"#)
        .bind(&server_name).bind(&hostname).bind(&timezone).bind(&language)
        .bind(i64::from(request.web_notifications)).bind(i64::from(request.email_notifications)).bind(i64::from(request.critical_only))
        .execute(&state.db).await;
    if let Err(e)=save { eprintln!("Settings preferences update error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"preferences_update_failed","PhotoOS ayarları kaydedilemedi."); }
    let row=match sqlx::query(r#"SELECT server_name,hostname,timezone,language,selected_disks_json,raid_type,web_notifications_enabled,email_notifications_enabled,critical_only,setup_version,created_at,updated_at FROM setup_preferences WHERE id=1"#).fetch_one(&state.db).await {
        Ok(r)=>r, Err(e)=>{eprintln!("Updated settings read error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"preferences_read_failed","Kaydedilen ayarlar yeniden okunamadı.");}
    };
    Json(UpdateSetupPreferencesResponse{success:true,message:"PhotoOS ayarları başarıyla kaydedildi.".to_string(),preferences:row_to_preferences(&row)}).into_response()
}

fn validate_initialize(request:&InitializeSetupRequest)->Result<(String,String,String,String,String,String,String,String),Response> {
    let username=request.username.trim().to_string(); let email=request.email.trim().to_lowercase();
    let server_name=normalize_server_name(&request.server_name); let hostname=normalize_hostname(&request.hostname);
    let timezone=request.timezone.trim().to_string(); let language=request.language.trim().to_string(); let raid_type=request.raid_type.trim().to_lowercase();
    if username.len()<3 || username.len()>64 { return Err(error_response(StatusCode::BAD_REQUEST,"invalid_username","Kullanıcı adı 3-64 karakter olmalıdır.")); }
    if !email.contains('@') || email.len()>254 { return Err(error_response(StatusCode::BAD_REQUEST,"invalid_email","Geçerli bir e-posta adresi girin.")); }
    if request.password.len()<8 { return Err(error_response(StatusCode::BAD_REQUEST,"weak_password","Parola en az 8 karakter olmalıdır.")); }
    if server_name.len()>80 { return Err(error_response(StatusCode::BAD_REQUEST,"invalid_server_name","Sunucu adı en fazla 80 karakter olabilir.")); }
    if !validate_hostname(&hostname) { return Err(error_response(StatusCode::BAD_REQUEST,"invalid_hostname","Hostname yalnızca küçük harf, rakam ve tire içerebilir.")); }
    if !validate_timezone(&timezone) { return Err(error_response(StatusCode::BAD_REQUEST,"invalid_timezone","Geçerli bir saat dilimi seçin.")); }
    if !validate_language(&language) { return Err(error_response(StatusCode::BAD_REQUEST,"invalid_language","Desteklenmeyen dil seçimi.")); }
    if !validate_raid_type(&raid_type) { return Err(error_response(StatusCode::BAD_REQUEST,"invalid_raid_type","Desteklenmeyen RAID tipi.")); }
    let disks=serde_json::to_string(&request.selected_disks).map_err(|_|error_response(StatusCode::BAD_REQUEST,"invalid_selected_disks","Disk seçimi hazırlanamadı."))?;
    Ok((username,email,server_name,hostname,timezone,language,raid_type,disks))
}
pub async fn initialize_setup(State(state): State<AppState>, Json(request): Json<InitializeSetupRequest>) -> Response {
    if let Err(e)=ensure_setup_schema(&state.db).await { eprintln!("Setup schema initialize error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"setup_schema_failed","Kurulum veritabanı hazırlanamadı."); }
    let existing=match user_count(&state.db).await { Ok(v)=>v, Err(e)=>{eprintln!("Setup count error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"database_error","Kullanıcı veritabanı kontrol edilemedi.");} };
    if existing>0 { return error_response(StatusCode::CONFLICT,"already_initialized","PhotoOS ilk kurulumu zaten tamamlanmış."); }
    let (username,email,server_name,hostname,timezone,language,raid_type,selected_disks_json)=match validate_initialize(&request){Ok(v)=>v,Err(r)=>return r};
    let salt=SaltString::generate(&mut OsRng);
    let password_hash=match Argon2::default().hash_password(request.password.as_bytes(),&salt){Ok(v)=>v.to_string(),Err(e)=>{eprintln!("Password hash error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"password_hash_failed","Parola güvenli biçimde hazırlanamadı.");}};
    let mut tx=match state.db.begin().await {Ok(v)=>v,Err(e)=>{eprintln!("Setup transaction start error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"database_error","Kurulum işlemi başlatılamadı.");}};
    let count=match sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM users").fetch_one(&mut *tx).await {Ok(v)=>v,Err(e)=>{eprintln!("Setup transaction count error: {e}");let _=tx.rollback().await;return error_response(StatusCode::INTERNAL_SERVER_ERROR,"database_error","Kullanıcı veritabanı kontrol edilemedi.");}};
    if count>0 {let _=tx.rollback().await;return error_response(StatusCode::CONFLICT,"already_initialized","PhotoOS ilk kurulumu başka bir oturum tarafından tamamlandı.");}
    let user_result=sqlx::query("INSERT INTO users (username,email,password_hash,created_at,role) VALUES (?,?,?,datetime('now'),'admin')")
        .bind(&username).bind(&email).bind(&password_hash).execute(&mut *tx).await;
    if let Err(e)=user_result { let text=e.to_string(); let _=tx.rollback().await;
        if text.contains("users.username") { return error_response(StatusCode::CONFLICT,"username_exists","Bu kullanıcı adı zaten kullanılıyor."); }
        if text.contains("users.email") { return error_response(StatusCode::CONFLICT,"email_exists","Bu e-posta adresi zaten kullanılıyor."); }
        eprintln!("Setup admin insert error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"user_creation_failed","Yönetici hesabı oluşturulamadı."); }
    let pref_result=sqlx::query(r#"INSERT INTO setup_preferences (id,server_name,hostname,timezone,language,selected_disks_json,raid_type,web_notifications_enabled,email_notifications_enabled,critical_only,setup_version,created_at,updated_at)
        VALUES (1,?,?,?,?,?,?,?,?,?, '2.1',datetime('now'),datetime('now'))
        ON CONFLICT(id) DO UPDATE SET server_name=excluded.server_name,hostname=excluded.hostname,timezone=excluded.timezone,language=excluded.language,
        selected_disks_json=excluded.selected_disks_json,raid_type=excluded.raid_type,
        web_notifications_enabled=excluded.web_notifications_enabled,email_notifications_enabled=excluded.email_notifications_enabled,
        critical_only=excluded.critical_only,setup_version=excluded.setup_version,updated_at=datetime('now')"#)
        .bind(&server_name).bind(&hostname).bind(&timezone).bind(&language).bind(&selected_disks_json).bind(&raid_type)
        .bind(i64::from(request.web_notifications)).bind(i64::from(request.email_notifications)).bind(i64::from(request.critical_only))
        .execute(&mut *tx).await;
    if let Err(e)=pref_result { eprintln!("Setup preferences insert error: {e}"); let _=tx.rollback().await; return error_response(StatusCode::INTERNAL_SERVER_ERROR,"preferences_save_failed","Kurulum tercihleri kaydedilemedi."); }
    if let Err(e)=tx.commit().await { eprintln!("Setup transaction commit error: {e}"); return error_response(StatusCode::INTERNAL_SERVER_ERROR,"database_error","Kurulum değişiklikleri kaydedilemedi."); }
    (StatusCode::CREATED,Json(InitializeSetupResponse{success:true,message:"PhotoOS ilk kurulumu tamamlandı.".to_string(),username,role:"admin".to_string(),preferences_saved:true})).into_response()
}

#[cfg(test)]
mod retired_feature_tests {
    use super::*;

    #[test]
    fn retired_cloud_preference_is_not_exposed() {
        let value =
            serde_json::to_value(default_preferences()).expect("preferences serialize");

        let retired_key = ["google", "drive"].join("_");

        assert!(
            value.get(&retired_key).is_none(),
            "retired cloud preference must not be exposed"
        );
    }

    #[tokio::test]
    async fn retired_cloud_preference_is_not_in_fresh_schema() {
        let db = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("memory sqlite");

        ensure_setup_preferences_schema(&db)
            .await
            .expect("setup schema");

        let rows = sqlx::query("PRAGMA table_info(setup_preferences)")
            .fetch_all(&db)
            .await
            .expect("table info");

        let retired_column = ["google", "drive", "enabled"].join("_");

        assert!(
            !rows.iter().any(|row| {
                row.try_get::<String, _>("name")
                    .map(|name| name == retired_column)
                    .unwrap_or(false)
            }),
            "fresh schema must not contain retired cloud preference"
        );
    }
}
