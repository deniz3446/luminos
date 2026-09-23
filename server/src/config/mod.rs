use std::{
    env,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use config::{Config, Environment, File};
use serde::Deserialize;

static INTERNAL_API_BASE_URL: OnceLock<String> = OnceLock::new();

static STORAGE_CONFIG: OnceLock<StorageConfig> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    pub runtime: RuntimeConfig,
    pub frontend: FrontendConfig,
    pub logging: LoggingConfig,
    pub release: ReleaseConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub root: String,
    pub disks_root: String,
    pub disk1: String,
    pub disk2: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub engine: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub directory: String,
    pub temporary_directory: String,
    pub cache_directory: String,
    pub log_directory: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FrontendConfig {
    pub dist_directory: String,
    pub index_file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ReleaseConfig {
    pub version: String,
    pub environment: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            database: DatabaseConfig::default(),
            runtime: RuntimeConfig::default(),
            frontend: FrontendConfig::default(),
            logging: LoggingConfig::default(),
            release: ReleaseConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8081,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            root: "/srv/photoos".to_string(),
            disks_root: "/srv/photoos/disks".to_string(),
            disk1: "/srv/photoos/disks/disk1".to_string(),
            disk2: "/srv/photoos/disks/disk2".to_string(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            engine: "sqlite".to_string(),
            path: "/var/lib/photoos/runtime/luminos.db".to_string(),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            directory: "/var/lib/photoos/runtime".to_string(),
            temporary_directory: "/var/lib/photoos/runtime/tmp".to_string(),
            cache_directory: "/var/lib/photoos/runtime/cache".to_string(),
            log_directory: "/var/lib/photoos/runtime/logs".to_string(),
        }
    }
}

impl Default for FrontendConfig {
    fn default() -> Self {
        Self {
            dist_directory: "/opt/photoos/current/client/dist".to_string(),
            index_file: "/opt/photoos/current/client/dist/index.html".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

impl Default for ReleaseConfig {
    fn default() -> Self {
        Self {
            version: "1.2.1-dev-fix8".to_string(),
            environment: "production".to_string(),
        }
    }
}

impl Settings {
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    pub fn internal_api_base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.server.port)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.server.host.trim().is_empty() {
            return Err("server.host boş olamaz".to_string());
        }

        if self.server.host.parse::<IpAddr>().is_err() && self.server.host != "localhost" {
            return Err(format!("server.host geçersiz: {}", self.server.host));
        }

        if self.server.port == 0 {
            return Err("server.port 1-65535 arasında olmalıdır".to_string());
        }

        if self.database.engine.trim() != "sqlite" {
            return Err(format!(
                "Desteklenmeyen database.engine: {}",
                self.database.engine
            ));
        }

        validate_non_empty_path("database.path", &self.database.path)?;
        validate_non_empty_path("storage.root", &self.storage.root)?;
        validate_non_empty_path("runtime.directory", &self.runtime.directory)?;
        validate_non_empty_path(
            "runtime.temporary_directory",
            &self.runtime.temporary_directory,
        )?;
        validate_non_empty_path("frontend.dist_directory", &self.frontend.dist_directory)?;

        Ok(())
    }

    pub fn prepare_directories(&self) -> Result<(), String> {
        let directories = [
            &self.runtime.directory,
            &self.runtime.temporary_directory,
            &self.runtime.cache_directory,
            &self.runtime.log_directory,
        ];

        for directory in directories {
            std::fs::create_dir_all(directory).map_err(|error| {
                format!("Runtime dizini oluşturulamadı: {}: {}", directory, error)
            })?;
        }

        if let Some(parent) = Path::new(&self.database.path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Veritabanı dizini oluşturulamadı: {}: {}",
                    parent.display(),
                    error
                )
            })?;
        }

        Ok(())
    }
}

fn validate_non_empty_path(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{name} boş olamaz"));
    }

    Ok(())
}

fn selected_config_file() -> PathBuf {
    if let Ok(value) = env::var("PHOTOOS_CONFIG_FILE") {
        let trimmed = value.trim();

        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let candidates = [
        PathBuf::from("/etc/photoos/photoos.toml"),
        PathBuf::from("/etc/photoos/config.toml"),
        PathBuf::from("config/config.toml"),
        PathBuf::from("config/config"),
    ];

    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("config/config.toml"))
}

fn apply_legacy_environment_overrides(settings: &mut Settings) {
    override_string("PHOTOOS_HOST", &mut settings.server.host);
    override_u16("PHOTOOS_PORT", &mut settings.server.port);

    if let Ok(value) = env::var("PHOTOOS_DATABASE_PATH") {
        let value = value.trim();

        if !value.is_empty() {
            settings.database.path = value.to_string();
        }
    }

    if let Ok(value) = env::var("PHOTOOS_DATABASE_URL") {
        let value = value.trim();

        if let Some(path) = sqlite_url_to_path(value) {
            settings.database.path = path;
        }
    }

    override_string("PHOTOOS_STORAGE_ROOT", &mut settings.storage.root);
    override_string("PHOTOOS_DISKS_ROOT", &mut settings.storage.disks_root);
    override_string("PHOTOOS_DISK1", &mut settings.storage.disk1);
    override_string("PHOTOOS_DISK2", &mut settings.storage.disk2);

    override_string("PHOTOOS_RUNTIME_DIR", &mut settings.runtime.directory);
    override_string(
        "PHOTOOS_TEMP_DIR",
        &mut settings.runtime.temporary_directory,
    );
    override_string("PHOTOOS_CACHE_ROOT", &mut settings.runtime.cache_directory);
    override_string("PHOTOOS_LOG_ROOT", &mut settings.runtime.log_directory);

    override_string(
        "PHOTOOS_FRONTEND_DIR",
        &mut settings.frontend.dist_directory,
    );
    override_string("PHOTOOS_FRONTEND_INDEX", &mut settings.frontend.index_file);

    override_string("RUST_LOG", &mut settings.logging.level);
    override_string("PHOTOOS_VERSION", &mut settings.release.version);
    override_string("PHOTOOS_ENVIRONMENT", &mut settings.release.environment);
}

fn override_string(variable: &str, target: &mut String) {
    if let Ok(value) = env::var(variable) {
        let value = value.trim();

        if !value.is_empty() {
            *target = value.to_string();
        }
    }
}

fn override_u16(variable: &str, target: &mut u16) {
    let Ok(value) = env::var(variable) else {
        return;
    };

    let value = value.trim();

    if value.is_empty() {
        return;
    }

    match value.parse::<u16>() {
        Ok(parsed) if parsed > 0 => *target = parsed,
        _ => panic!("{variable} geçerli bir port değil: {value}"),
    }
}

fn sqlite_url_to_path(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    if let Some(path) = value.strip_prefix("sqlite:///") {
        return Some(format!("/{path}"));
    }

    if let Some(path) = value.strip_prefix("sqlite://") {
        return Some(path.to_string());
    }

    if let Some(path) = value.strip_prefix("sqlite:") {
        return Some(path.to_string());
    }

    None
}

pub fn storage_config() -> StorageConfig {
    STORAGE_CONFIG.get().cloned().unwrap_or_default()
}

pub fn internal_api_base_url() -> String {
    INTERNAL_API_BASE_URL
        .get()
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
}

fn resolve_release_version(
    configured_version: &str,
    version_file: &std::path::Path,
) -> String {
    std::fs::read_to_string(version_file)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| configured_version.to_string())
}

pub fn load() -> Settings {
    let config_file = selected_config_file();

    println!("PhotoOS config file: {}", config_file.display());

    let mut builder = Config::builder();

    if config_file.exists() {
        builder = builder.add_source(File::from(config_file.clone()).required(true));
    } else {
        println!("PhotoOS config file bulunamadı; güvenli varsayılanlar kullanılacak.");
    }

    builder = builder.add_source(
        Environment::with_prefix("PHOTOOS")
            .prefix_separator("__")
            .separator("__")
            .try_parsing(true),
    );

    let mut settings: Settings = builder
        .build()
        .and_then(Config::try_deserialize)
        .unwrap_or_else(|error| {
            panic!(
                "PhotoOS yapılandırması yüklenemedi: {}: {}",
                config_file.display(),
                error
            )
        });

    apply_legacy_environment_overrides(&mut settings);

    settings.release.version = resolve_release_version(
        &settings.release.version,
        std::path::Path::new("/opt/photoos/current/VERSION"),
    );

    settings
        .validate()
        .unwrap_or_else(|error| panic!("Geçersiz PhotoOS yapılandırması: {error}"));

    let _ = STORAGE_CONFIG.set(settings.storage.clone());

    let configured_internal_api = settings.internal_api_base_url();

    let _ = INTERNAL_API_BASE_URL.set(configured_internal_api.clone());

    println!("PhotoOS internal API: {}", configured_internal_api);

    settings
}

// PHOTOOS PHASE2A SECURE DEFAULT TESTS
#[cfg(test)]
mod phase2a_tests {
    use super::*;

    #[test]
    fn secure_server_defaults_bind_localhost() {
        let server = ServerConfig::default();
        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 8081);
    }
    #[test]
    fn active_release_version_file_overrides_stale_config() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "photoos-active-version-{}-{stamp}",
            std::process::id()
        ));

        std::fs::write(&path, "1.2.4\n").unwrap();

        let resolved = resolve_release_version(
            "1.2.1",
            &path,
        );

        let _ = std::fs::remove_file(&path);

        assert_eq!(resolved, "1.2.4");
    }

}
