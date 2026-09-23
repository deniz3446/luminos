use std::{net::SocketAddr, sync::Arc};

mod app;
mod auth;
mod config;
mod database;
mod handlers;
mod models;
mod repositories;
mod routes;
mod services;
mod state;
mod storage;
mod storage_monitor;

#[tokio::main]
async fn main() {
    println!("=== Starting LuminOS ===");

    // Yapılandırmayı yükle
    let settings = config::load();

    settings
        .prepare_directories()
        .expect("PhotoOS runtime dizinleri hazırlanamadı");

    auth::initialize_secret(std::path::Path::new(&settings.runtime.directory))
        .expect("PhotoOS JWT secret başlatılamadı");

    println!("==============================");
    println!("LuminOS Configuration Loaded");
    println!("{:#?}", settings);
    println!("==============================");

    // Ortak uygulama durumunu oluştur
    let settings = Arc::new(settings);

    let db = database::connect(&settings.database.path).await;

    let storage = Arc::new(storage::StorageEngine::photoos_default());

    storage
        .initialize()
        .await
        .expect("Storage Engine başlatılamadı");

    let state = state::AppState {
        settings,
        db,
        storage,
    };

    handlers::mobile::initialize_mobile(&state)
        .await
        .expect("Mobile tabloları ve fotoğraf metadata şeması hazırlanamadı");

    handlers::storage_history::initialize_storage_history(&state.db)
        .await
        .expect("Storage history tabloları oluşturulamadı");

    handlers::storage_history::spawn_storage_history_collector(state.db.clone());
    storage_monitor::start(state.db.clone());
    handlers::backups::initialize_backup_manager(&state.db)
        .await
        .expect("Backup Manager tablosu oluşturulamadı");

    handlers::pc_backups::initialize_pc_backup(&state.db)
        .await
        .expect("PC Backup tabloları oluşturulamadı");

    handlers::notifications::initialize_notification_runtime(&state)
        .await
        .expect("Notification Runtime başlatılamadı");

    handlers::notifications::spawn_notification_runtime();

    handlers::performance_alerts::initialize_performance_alerts(&state)
        .await
        .expect("Performance Alert Engine başlatılamadı");

    handlers::performance_alerts::spawn_performance_alert_engine(state.clone());

    // Router'ı oluştur
    let app = app::create_router(state.clone());

    // Config'den host ve port bilgisini al
    let addr = state.settings.bind_address();

    println!("Binding to {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Cannot bind port");

    println!("Server is running!");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server crashed");
}

mod update_agent;

mod update_trust;

mod update_state;

mod update_distribution;

mod update_discovery;

mod update_download;

mod update_agent_flow;
