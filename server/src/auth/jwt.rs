use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::OnceLock,
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode,
    encode,
    DecodingKey,
    EncodingKey,
    Header,
    Validation,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};

static SECRET: OnceLock<Vec<u8>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,
    pub exp: usize,
}

fn secure_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

fn read_existing_secret(path: &Path) -> Result<Vec<u8>, String> {
    let secret = fs::read(path)
        .map_err(|error| format!("JWT secret okunamadı: {}: {error}", path.display()))?;

    if secret.len() < 32 {
        return Err(format!(
            "JWT secret çok kısa: {} ({} byte, minimum 32)",
            path.display(),
            secret.len()
        ));
    }

    secure_permissions(path).map_err(|error| {
        format!(
            "JWT secret dosya izinleri güvenli hale getirilemedi: {}: {error}",
            path.display()
        )
    })?;

    Ok(secret)
}

fn load_or_create_secret(path: &Path) -> Result<Vec<u8>, String> {
    if path.exists() {
        return read_existing_secret(path);
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("JWT secret parent dizini yok: {}", path.display()))?;

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "JWT secret dizini oluşturulamadı: {}: {error}",
            parent.display()
        )
    })?;

    let mut secret = vec![0_u8; 64];
    let mut rng = OsRng;
    rng.fill_bytes(&mut secret);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        options.mode(0o600);
    }

    match options.open(path) {
        Ok(mut file) => {
            file.write_all(&secret).map_err(|error| {
                format!("JWT secret yazılamadı: {}: {error}", path.display())
            })?;

            file.sync_all().map_err(|error| {
                format!("JWT secret diske yazılamadı: {}: {error}", path.display())
            })?;

            secure_permissions(path).map_err(|error| {
                format!(
                    "JWT secret dosya izinleri ayarlanamadı: {}: {error}",
                    path.display()
                )
            })?;

            Ok(secret)
        }

        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            read_existing_secret(path)
        }

        Err(error) => Err(format!(
            "JWT secret oluşturulamadı: {}: {error}",
            path.display()
        )),
    }
}

pub fn initialize_secret(runtime_dir: &Path) -> Result<(), String> {
    if SECRET.get().is_some() {
        return Ok(());
    }

    let path = runtime_dir.join("jwt-secret");
    let secret = load_or_create_secret(&path)?;

    match SECRET.set(secret) {
        Ok(()) => Ok(()),
        Err(_) if SECRET.get().is_some() => Ok(()),
        Err(_) => Err("JWT secret başlatılamadı.".to_string()),
    }
}

fn production_secret() -> &'static [u8] {
    SECRET
        .get()
        .expect("JWT secret initialize_secret ile başlatılmalıdır")
        .as_slice()
}

fn create_token_with_secret(
    user_id: i64,
    secret: &[u8],
) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = Utc::now() + Duration::hours(24);

    let claims = Claims {
        sub: user_id,
        exp: expiration.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
}

fn decode_token_with_secret(
    token: &str,
    secret: &[u8],
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )?;

    Ok(data.claims)
}

pub fn create_token(user_id: i64) -> String {
    create_token_with_secret(user_id, production_secret())
        .expect("JWT token oluşturulamadı")
}

pub fn decode_token(
    token: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode_token_with_secret(token, production_secret())
}

// PHOTOOS PHASE2A JWT TESTS
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn test_secret_path(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!(
            "photoos-jwt-test-{}-{}-{}",
            std::process::id(),
            unique,
            label
        ))
    }

    #[test]
    fn secret_file_is_created_and_reused() {
        let dir = test_secret_path("create-reuse");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("jwt-secret");

        let first = load_or_create_secret(&path).unwrap();
        let second = load_or_create_secret(&path).unwrap();

        assert!(first.len() >= 32);
        assert_eq!(first, second);
        assert_eq!(fs::read(&path).unwrap(), first);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_validation_depends_on_secret() {
        let secret_a = [0x11_u8; 64];
        let secret_b = [0x22_u8; 64];

        let token = create_token_with_secret(42, &secret_a).unwrap();

        assert_eq!(
            decode_token_with_secret(&token, &secret_a).unwrap().sub,
            42
        );
        assert!(decode_token_with_secret(&token, &secret_b).is_err());
    }
}
