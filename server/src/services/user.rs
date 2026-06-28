use argon2::{
    password_hash::{
        PasswordHash,
        PasswordVerifier,
        SaltString,
    },
    Argon2,
    PasswordHasher,
};

use sqlx::SqlitePool;

use crate::{
    auth,
    models::{
        create_user::CreateUserRequest,
        login::LoginRequest,
        login_response::LoginResponse,
        user_response::UserResponse,
    },
    repositories,
};

pub async fn get_users(
    db: &SqlitePool,
) -> Result<Vec<UserResponse>, sqlx::Error> {
    let users = repositories::user::all(db).await?;

    Ok(
        users
            .into_iter()
            .map(|u| UserResponse {
                id: u.id,
                username: u.username,
                created_at: u.created_at,
            })
            .collect(),
    )
}

pub async fn create_user(
    db: &SqlitePool,
    req: CreateUserRequest,
) -> Result<(), String> {
    if req.username.trim().len() < 3 {
        return Err("Kullanıcı adı en az 3 karakter olmalı.".to_string());
    }

    if !is_valid_email(&req.email) {
        return Err("Geçerli bir e-posta adresi gir.".to_string());
    }

    if req.password.len() < 8 {
        return Err("Şifre en az 8 karakter olmalı.".to_string());
    }

    let salt = SaltString::generate(
        &mut argon2::password_hash::rand_core::OsRng,
    );

    let password_hash = Argon2::default()
        .hash_password(
            req.password.as_bytes(),
            &salt,
        )
        .unwrap()
        .to_string();

    repositories::user::create(
        db,
        &req,
        password_hash,
    )
    .await
    .map_err(|err| err.to_string())
}

pub async fn login(
    db: &SqlitePool,
    req: LoginRequest,
) -> Result<LoginResponse, String> {
    let user = repositories::user::find_by_email(
        db,
        &req.email,
    )
    .await
    .map_err(|_| "E-posta veya şifre hatalı.".to_string())?;

    let parsed_hash = PasswordHash::new(
        &user.password_hash,
    )
    .map_err(|_| "Geçersiz parola.".to_string())?;

    Argon2::default()
        .verify_password(
            req.password.as_bytes(),
            &parsed_hash,
        )
        .map_err(|_| "E-posta veya şifre hatalı.".to_string())?;

    let token = auth::create_token(user.id);

    Ok(LoginResponse { token })
}

fn is_valid_email(email: &str) -> bool {
    let email = email.trim();

    if email.len() < 5 {
        return false;
    }

    if email.contains(' ') {
        return false;
    }

    let parts: Vec<&str> = email.split('@').collect();

    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() || domain.is_empty() {
        return false;
    }

    if !domain.contains('.') {
        return false;
    }

    true
}
