use std::{path::Path, str::FromStr};

use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePool},
    ConnectOptions,
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn connect(database_path: &str) -> SqlitePool {
    println!("==============================");
    println!("Connecting database...");

    let url = format!("sqlite://{}", database_path);

    println!("Database URL      : {}", url);
    println!(
        "Current Directory : {:?}",
        std::env::current_dir().unwrap()
    );
    println!(
        "Database Exists   : {}",
        Path::new(database_path).exists()
    );

    let options = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options)
        .await
        .expect("Cannot connect database");

    println!("Database connected.");

    println!("Running migrations...");

    MIGRATOR
        .run(&pool)
        .await
        .expect("Migration failed");

    println!("Migrations completed.");
    println!("==============================");

    pool
}
