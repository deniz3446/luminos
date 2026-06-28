use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

use crate::{
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
