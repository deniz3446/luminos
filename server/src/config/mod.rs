use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub root: String,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub engine: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
}

pub fn load() -> Settings {
    config::Config::builder()
        .add_source(config::File::with_name("config/config"))
        .build()
        .expect("Config dosyası okunamadı")
        .try_deserialize()
        .expect("Config parse edilemedi")
}
