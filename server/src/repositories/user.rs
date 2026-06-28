use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{
    create_user::CreateUserRequest,
    user::User,
};

pub async fn all(
    db: &SqlitePool,
) -> Result<Vec<User>, sqlx::Error> {

    let users = sqlx::query_as::<_, User>(
        r#"
        SELECT
            id,
            username,
            email,
            password_hash,
            created_at
        FROM users
        ORDER BY id;
        "#
    )
    .fetch_all(db)
    .await?;

    Ok(users)
}

pub async fn find_by_email(
    db: &SqlitePool,
    email: &str,
) -> Result<User, sqlx::Error> {

    sqlx::query_as::<_, User>(
        r#"
        SELECT
            id,
            username,
            email,
            password_hash,
            created_at
        FROM users
        WHERE email = ?
        "#
    )
    .bind(email)
    .fetch_one(db)
    .await
}

pub async fn create(
    db: &SqlitePool,
    req: &CreateUserRequest,
    password_hash: String,
) -> Result<(), sqlx::Error> {

    sqlx::query(
        r#"
        INSERT INTO users (
            username,
            email,
            password_hash,
            created_at
        )
        VALUES (?, ?, ?, ?)
        "#
    )
    .bind(&req.username)
    .bind(&req.email)
    .bind(password_hash)
    .bind(Utc::now().to_rfc3339())
    .execute(db)
    .await?;

    Ok(())
}
