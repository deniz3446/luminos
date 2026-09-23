use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    Json,
};

use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    auth::decode_token,
    models::{
        api_response::ApiResponse,
        create_user::CreateUserRequest,
        login::LoginRequest,
        login_response::LoginResponse,
        user_response::UserResponse,
    },
    services,
    state::AppState,
};

pub async fn list_users(
    State(state): State<AppState>,
) -> Json<Vec<UserResponse>> {
    Json(
        services::user::get_users(&state.db)
            .await
            .unwrap(),
    )
}

pub async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match services::user::create_user(&state.db, req).await {
        Ok(_) => (
            StatusCode::CREATED,
            Json(ApiResponse {
                success: true,
                message: "Kullanıcı oluşturuldu.".to_string(),
                data: None,
            }),
        ),

        Err(msg) => {
            let status = if msg.contains("zaten kayıtlı")
                || msg.contains("UNIQUE constraint failed")
            {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };

            let message = if msg.contains("users.email") {
                "Bu e-posta zaten kayıtlı.".to_string()
            } else if msg.contains("users.username") {
                "Bu kullanıcı adı zaten kayıtlı.".to_string()
            } else {
                msg
            };

            (
                status,
                Json(ApiResponse {
                    success: false,
                    message,
                    data: None,
                }),
            )
        }
    }
}


#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CurrentUserResponse {
    pub username: String,
    pub role: String,
}

async fn current_user_by_id(
    db: &SqlitePool,
    user_id: i64,
) -> Result<Option<CurrentUserResponse>, sqlx::Error> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT username, role FROM users WHERE id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map(|row| {
        row.map(|(username, role)| CurrentUserResponse {
            username,
            role,
        })
    })
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<ApiResponse<CurrentUserResponse>>) {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .map(|value| value.strip_prefix("Bearer ").unwrap_or(value))
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(token) = token else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Oturum anahtarı bulunamadı.".to_string(),
                data: None,
            }),
        );
    };

    let claims = match decode_token(token) {
        Ok(claims) => claims,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse {
                    success: false,
                    message: "Oturum süresi dolmuş veya token geçersiz.".to_string(),
                    data: None,
                }),
            );
        }
    };

    match current_user_by_id(&state.db, claims.sub).await {
        Ok(Some(user)) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: "Oturum doğrulandı.".to_string(),
                data: Some(user),
            }),
        ),

        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: "Oturuma ait kullanıcı bulunamadı.".to_string(),
                data: None,
            }),
        ),

        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: "Oturum bilgisi okunamadı.".to_string(),
                data: None,
            }),
        ),
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> (StatusCode, Json<ApiResponse<LoginResponse>>) {
    match services::user::login(&state.db, req).await {
        Ok(token) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: "Giriş başarılı.".to_string(),
                data: Some(token),
            }),
        ),

        Err(msg) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse {
                success: false,
                message: msg,
                data: None,
            }),
        ),
    }
}

// PHOTOOS CURRENT SESSION CONTRACT TESTS
#[cfg(test)]
mod current_session_tests {
    use super::*;

    #[tokio::test]
    async fn current_user_lookup_returns_username_and_role() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                role TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO users(id, username, role) VALUES(7, 'DenemeAdmin', 'admin')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let user = current_user_by_id(&pool, 7)
            .await
            .unwrap()
            .expect("current user");

        assert_eq!(user.username, "DenemeAdmin");
        assert_eq!(user.role, "admin");
    }
}
