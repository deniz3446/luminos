use std::sync::Arc;

mod app;
mod config;
mod database;
mod handlers;
mod models;
mod repositories;
mod routes;
mod services;
mod auth;
mod state;

#[tokio::main]
async fn main() {
    println!("=== Starting LuminOS ===");

    // Yapılandırmayı yükle
    let settings = config::load();

    println!("==============================");
    println!("LuminOS Configuration Loaded");
    println!("{:#?}", settings);
    println!("==============================");

    // Ortak uygulama durumunu oluştur
let settings = Arc::new(settings);

let db = database::connect(
    &settings.database.path,
)
.await;

let state = state::AppState {
    settings,
    db,
};
    // Router'ı oluştur
    let app = app::create_router(state.clone());

    // Config'den host ve port bilgisini al
    let addr = format!(
        "{}:{}",
        state.settings.server.host,
        state.settings.server.port
    );

    println!("Binding to {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Cannot bind port");

    println!("Server is running!");

    axum::serve(listener, app)
        .await
        .expect("Server crashed");
}
