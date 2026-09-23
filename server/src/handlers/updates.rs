use axum::{
    Json,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{
    Digest,
    Sha256,
};
use uuid::Uuid;

use std::path::{Path, PathBuf};
use tokio::{
    fs,
    io::AsyncReadExt,
};

use crate::{auth::decode_token, state::AppState};
use crate::update_agent::AgentTransport;
use crate::update_agent_flow::{
    AgentPackageIdentity,
    InstallApproval,
    UpdateAgentVerificationFlow,
    VerifiedAgentPackage,
};

use crate::update_discovery::{
    DiscoveryOutcome,
    UpdateDiscoveryService,
    ValidatedManifest,
};
use crate::update_download::{
    PackageDownloadRequest,
    PackageDownloadService,
    StagedPackage,
};
use crate::update_state::{
    UpdateChannel,
    UpdateOperationRecord,
    UpdateOperationStatus,
    UpdateStateStore,
    VerifiedUpdateOperationRecord,
};

const AGENT_BASE_URL: &str = "http://127.0.0.1:8091";
const AGENT_ENV_FILE: &str = "/etc/photoos/update-agent.env";

#[derive(Debug, Deserialize, Clone)]
pub struct PackageActionRequest {
    pub filename: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RollbackRequest {
    pub version: String,
}

fn json_response(status: StatusCode, payload: Value) -> Response {
    (status, Json(payload)).into_response()
}

fn get_user_id(headers: &HeaderMap) -> Option<i64> {
    let value = headers.get(header::AUTHORIZATION)?;
    let auth = value.to_str().ok()?;
    let token = auth.strip_prefix("Bearer ").unwrap_or(auth);

    decode_token(token).ok().map(|claims| claims.sub)
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<i64, Response> {
    let Some(user_id) = get_user_id(headers) else {
        return Err(json_response(
            StatusCode::UNAUTHORIZED,
            json!({
                "error": "unauthorized",
                "message": "Oturum doğrulanamadı."
            }),
        ));
    };

    let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    if role.as_deref() != Some("admin") {
        return Err(json_response(
            StatusCode::FORBIDDEN,
            json!({
                "error": "forbidden",
                "message": "Bu işlem yalnızca yöneticiler içindir."
            }),
        ));
    }

    Ok(user_id)
}

async fn read_update_token() -> Result<String, String> {
    let text = fs::read_to_string(AGENT_ENV_FILE)
        .await
        .map_err(|error| format!("Update Agent yapılandırması okunamadı: {error}"))?;

    for raw_line in text.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if key.trim() == "PHOTOOS_UPDATE_TOKEN" {
            let token = value.trim();

            if token.is_empty() {
                return Err("Update Agent anahtarı boş.".to_string());
            }

            return Ok(token.to_string());
        }
    }

    Err("PHOTOOS_UPDATE_TOKEN bulunamadı.".to_string())
}

async fn agent_get(path: &str) -> Response {
    let url = format!("{AGENT_BASE_URL}{path}");

    let response = match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(12))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return json_response(
                StatusCode::BAD_GATEWAY,
                json!({
                    "error": "agent_unavailable",
                    "message": format!(
                        "Update Agent bağlantısı kurulamadı: {error}"
                    )
                }),
            );
        }
    };

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let body = response.json::<Value>().await.unwrap_or_else(|error| {
        json!({
            "error": "invalid_agent_response",
            "message": format!(
                "Update Agent yanıtı okunamadı: {error}"
            )
        })
    });

    json_response(status, body)
}

async fn agent_post(path: &str, payload: Value) -> Response {
    let token = match read_update_token().await {
        Ok(token) => token,
        Err(error) => {
            return json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error": "agent_configuration_error",
                    "message": error
                }),
            );
        }
    };

    let url = format!("{AGENT_BASE_URL}{path}");

    let response = match reqwest::Client::new()
        .post(&url)
        .header("X-PhotoOS-Update-Token", token)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return json_response(
                StatusCode::BAD_GATEWAY,
                json!({
                    "error": "agent_request_failed",
                    "message": format!(
                        "Update Agent isteği başarısız: {error}"
                    )
                }),
            );
        }
    };

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let body = response.json::<Value>().await.unwrap_or_else(|error| {
        json!({
            "error": "invalid_agent_response",
            "message": format!(
                "Update Agent yanıtı okunamadı: {error}"
            )
        })
    });

    json_response(status, body)
}

/*
 * Kurulum ve rollback sırasında Update Agent PhotoOS Server'ı
 * yeniden başlatacağı için tarayıcı isteği bekletilmez.
 * İstek Agent'a arka planda gönderilir ve panel sağlık
 * durumunu tekrar sorgular.
 */
async fn agent_post_background(path: &'static str, payload: Value) -> Response {
    let token = match read_update_token().await {
        Ok(token) => token,
        Err(error) => {
            return json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error": "agent_configuration_error",
                    "message": error
                }),
            );
        }
    };

    tokio::spawn(async move {
        let url = format!("{AGENT_BASE_URL}{path}");

        let result = reqwest::Client::new()
            .post(url)
            .header("X-PhotoOS-Update-Token", token)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .await;

        match result {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();

                println!(
                    "Update Agent arka plan işlemi tamamlandı: path={} status={} body={}",
                    path, status, body
                );
            }
            Err(error) => {
                /*
                 * Server restart edildiğinde bağlantının kesilmesi
                 * normal olabilir. Update Agent işlemi devam eder.
                 */
                eprintln!(
                    "Update Agent arka plan bağlantısı kapandı: path={} error={}",
                    path, error
                );
            }
        }
    });

    json_response(
        StatusCode::ACCEPTED,
        json!({
            "accepted": true,
            "message": "İşlem Update Agent'a gönderildi.",
            "poll_after_seconds": 5
        }),
    )
}

pub async fn update_status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    agent_get("/status").await
}

pub async fn update_releases(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    agent_get("/releases").await
}

pub async fn update_packages(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    agent_get("/packages").await
}

pub async fn update_history(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    agent_get("/history").await
}

pub async fn upload_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    const MAX_PACKAGE_BYTES: usize = 512 * 1024 * 1024;

    let mut package_name: Option<String> = None;
    let mut package_data: Option<axum::body::Bytes> = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "multipart_error",
                        "message": format!(
                            "Paket verisi okunamadı: {error}"
                        )
                    }),
                );
            }
        };

        if field.name() != Some("file") {
            continue;
        }

        let filename = field.file_name().unwrap_or("").trim().to_string();

        if filename.is_empty()
            || Path::new(&filename)
                .file_name()
                .and_then(|value| value.to_str())
                != Some(filename.as_str())
            || !filename.ends_with(".popkg")
        {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "invalid_filename",
                    "message": "Yalnızca geçerli .popkg paketleri yüklenebilir."
                }),
            );
        }

        let bytes = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(error) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "file_read_failed",
                        "message": format!(
                            "Paket dosyası okunamadı: {error}"
                        )
                    }),
                );
            }
        };

        if bytes.is_empty() || bytes.len() > MAX_PACKAGE_BYTES {
            return json_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({
                    "error": "invalid_package_size",
                    "message": "Paket boş veya 512 MB sınırından büyük."
                }),
            );
        }

        package_name = Some(filename);
        package_data = Some(bytes);

        break;
    }

    let Some(filename) = package_name else {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "file_required",
                "message": ".popkg dosyası seçilmedi."
            }),
        );
    };

    let Some(bytes) = package_data else {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "file_required",
                "message": "Paket içeriği bulunamadı."
            }),
        );
    };

    let token = match read_update_token().await {
        Ok(token) => token,
        Err(error) => {
            return json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error": "agent_configuration_error",
                    "message": error
                }),
            );
        }
    };

    let response = match reqwest::Client::new()
        .post(format!("{AGENT_BASE_URL}/upload-package"))
        .header("X-PhotoOS-Update-Token", token)
        .header("X-PhotoOS-Filename", filename)
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(bytes)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await
    {
        Ok(response) => response,

        Err(error) => {
            return json_response(
                StatusCode::BAD_GATEWAY,
                json!({
                    "error": "agent_upload_failed",
                    "message": format!(
                        "Paket Update Agent'a aktarılamadı: {error}"
                    )
                }),
            );
        }
    };

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let body = response.json::<Value>().await.unwrap_or_else(|error| {
        json!({
            "error": "invalid_agent_response",
            "message": format!(
                "Update Agent yanıtı okunamadı: {error}"
            )
        })
    });

    json_response(status, body)
}

pub async fn verify_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PackageActionRequest>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    let filename = payload.filename.trim();

    if filename.is_empty()
        || Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            != Some(filename)
        || !filename.ends_with(".popkg")
    {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "invalid_filename",
                "message": "Geçerli bir .popkg paket adı girilmelidir."
            }),
        );
    }

    agent_post(
        "/verify-package",
        json!({
            "filename": filename
        }),
    )
    .await
}

pub async fn install_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PackageActionRequest>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    let filename = payload.filename.trim();

    if filename.is_empty()
        || Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            != Some(filename)
        || !filename.ends_with(".popkg")
    {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "invalid_filename",
                "message": "Geçerli bir .popkg paket adı girilmelidir."
            }),
        );
    }

    agent_post_background(
        "/install-package",
        json!({
            "filename": filename
        }),
    )
    .await
}

pub async fn rollback_release(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RollbackRequest>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }

    let version = payload.version.trim();

    if version.is_empty()
        || version.contains('/')
        || version.contains('\\')
        || version.contains("..")
    {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "invalid_version",
                "message": "Geçersiz sürüm bilgisi."
            }),
        );
    }

    agent_post_background(
        "/rollback",
        json!({
            "version": version
        }),
    )
    .await
}

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateChannelRequest {
    pub channel: String,
}

fn update_channel_name(
    channel: UpdateChannel,
) -> &'static str {
    match channel {
        UpdateChannel::Stable =>
            "stable",

        UpdateChannel::Beta =>
            "beta",

        UpdateChannel::Disabled =>
            "disabled",
    }
}

fn parse_update_channel_name(
    value: &str,
) -> Result<UpdateChannel, String> {
    match value {
        "stable" =>
            Ok(UpdateChannel::Stable),

        "beta" =>
            Ok(UpdateChannel::Beta),

        "disabled" =>
            Ok(UpdateChannel::Disabled),

        _ =>
            Err(
                "channel must be stable, beta or disabled"
                    .to_string()
            ),
    }
}

async fn persist_update_channel_name(
    store: &UpdateStateStore,
    value: &str,
) -> Result<UpdateChannel, String> {
    let channel =
        parse_update_channel_name(
            value
        )?;

    store
        .set_channel(channel)
        .await
        .map_err(
            |error| error.to_string()
        )?;

    Ok(channel)
}

async fn update_state_store(
    state: &AppState,
) -> Result<UpdateStateStore, Response> {
    let store =
        UpdateStateStore::new(
            state.db.clone()
        );

    if let Err(error) =
        store.ensure_schema().await
    {
        return Err(
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error":
                        "update_state_error",

                    "message":
                        error.to_string()
                }),
            )
        );
    }

    Ok(store)
}

async fn fetch_update_agent_version(
) -> Result<String, Response> {
    let transport =
        AgentTransport::new(
            AGENT_BASE_URL.to_string(),
            None,
        );

    let response =
        transport
            .get_json("/status")
            .await
            .map_err(
                |error| {
                    json_response(
                        StatusCode::BAD_GATEWAY,
                        json!({
                            "error":
                                "agent_unavailable",

                            "message":
                                error.to_string()
                        }),
                    )
                }
            )?;

    if !(200..300)
        .contains(&response.status)
    {
        return Err(
            json_response(
                StatusCode::BAD_GATEWAY,
                json!({
                    "error":
                        "agent_status_failed",

                    "status":
                        response.status
                }),
            )
        );
    }

    let version =
        response
            .body
            .get("agent_version")
            .and_then(Value::as_str)
            .filter(
                |value| !value.is_empty()
            )
            .ok_or_else(
                || {
                    json_response(
                        StatusCode::BAD_GATEWAY,
                        json!({
                            "error":
                                "invalid_agent_response"
                        }),
                    )
                }
            )?;

    Ok(version.to_string())
}

async fn check_updates_with_service(
    service: &UpdateDiscoveryService,
    store: &UpdateStateStore,
    running_version: &str,
    update_agent_version: &str,
) -> Result<
    (
        UpdateChannel,
        DiscoveryOutcome,
    ),
    String,
> {
    let channel =
        store
            .get_channel()
            .await
            .map_err(
                |error| error.to_string()
            )?;

    let outcome =
        service
            .check_channel_for_versions(
                store,
                channel,
                running_version,
                update_agent_version,
            )
            .await
            .map_err(
                |error| error.to_string()
            )?;

    Ok(
        (
            channel,
            outcome,
        )
    )
}

fn discovery_response_payload(
    channel: UpdateChannel,
    running_version: &str,
    update_agent_version: &str,
    outcome: DiscoveryOutcome,
) -> Value {
    let channel =
        update_channel_name(channel);

    match outcome {
        DiscoveryOutcome::Disabled => {
            json!({
                "channel": channel,
                "running_version": running_version,
                "update_agent_version": update_agent_version,
                "status": "disabled",
                "update_available": false
            })
        }

        DiscoveryOutcome::UpToDate => {
            json!({
                "channel": channel,
                "running_version": running_version,
                "update_agent_version": update_agent_version,
                "status": "up_to_date",
                "update_available": false
            })
        }

        DiscoveryOutcome::Incompatible => {
            json!({
                "channel": channel,
                "running_version": running_version,
                "update_agent_version": update_agent_version,
                "status": "incompatible",
                "update_available": false
            })
        }

        DiscoveryOutcome::ReplayRejected => {
            json!({
                "channel": channel,
                "running_version": running_version,
                "update_agent_version": update_agent_version,
                "status": "replay_rejected",
                "update_available": false
            })
        }

        DiscoveryOutcome::UpdateAvailable {
            version,
            release_id,
            package_filename,
        } => {
            json!({
                "channel": channel,
                "running_version": running_version,
                "update_agent_version": update_agent_version,
                "status": "update_available",
                "update_available": true,
                "version": version,
                "release_id": release_id,
                "package_filename": package_filename
            })
        }
    }
}

pub async fn get_update_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) =
        require_admin(
            &state,
            &headers,
        )
        .await
    {
        return response;
    }

    let store =
        match update_state_store(&state)
            .await
        {
            Ok(store) => store,
            Err(response) =>
                return response,
        };

    match store.get_channel().await {
        Ok(channel) =>
            json_response(
                StatusCode::OK,
                json!({
                    "channel":
                        update_channel_name(
                            channel
                        )
                }),
            ),

        Err(error) =>
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error":
                        "update_state_error",

                    "message":
                        error.to_string()
                }),
            ),
    }
}

pub async fn set_update_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateChannelRequest>,
) -> Response {
    if let Err(response) =
        require_admin(
            &state,
            &headers,
        )
        .await
    {
        return response;
    }

    let store =
        match update_state_store(&state)
            .await
        {
            Ok(store) => store,
            Err(response) =>
                return response,
        };

    match persist_update_channel_name(
        &store,
        &payload.channel,
    )
    .await
    {
        Ok(channel) =>
            json_response(
                StatusCode::OK,
                json!({
                    "channel":
                        update_channel_name(
                            channel
                        ),

                    "saved":
                        true
                }),
            ),

        Err(error) =>
            json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error":
                        "invalid_update_channel",

                    "message":
                        error
                }),
            ),
    }
}

pub async fn check_for_updates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) =
        require_admin(
            &state,
            &headers,
        )
        .await
    {
        return response;
    }

    let store =
        match update_state_store(&state)
            .await
        {
            Ok(store) => store,
            Err(response) =>
                return response,
        };

    let channel =
        match store.get_channel().await {
            Ok(channel) =>
                channel,

            Err(error) =>
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "update_state_error",

                        "message":
                            error.to_string()
                    }),
                ),
        };

    let running_version =
        state
            .settings
            .release
            .version
            .clone();

    /*
     * Disabled must stop before Agent status
     * and before remote manifest traffic.
     */
    if channel
        == UpdateChannel::Disabled
    {
        return json_response(
            StatusCode::OK,
            discovery_response_payload(
                channel,
                &running_version,
                "not_checked",
                DiscoveryOutcome::Disabled,
            ),
        );
    }

    let update_agent_version =
        match fetch_update_agent_version()
            .await
        {
            Ok(version) =>
                version,

            Err(response) =>
                return response,
        };

    let service =
        match UpdateDiscoveryService::new()
        {
            Ok(service) =>
                service,

            Err(error) =>
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "discovery_initialization_failed",

                        "message":
                            error.to_string()
                    }),
                ),
        };

    let (
        checked_channel,
        outcome,
    ) =
        match check_updates_with_service(
            &service,
            &store,
            &running_version,
            &update_agent_version,
        )
        .await
        {
            Ok(result) =>
                result,

            Err(error) =>
                return json_response(
                    StatusCode::BAD_GATEWAY,
                    json!({
                        "error":
                            "update_check_failed",

                        "message":
                            error
                    }),
                ),
        };

    json_response(
        StatusCode::OK,
        discovery_response_payload(
            checked_channel,
            &running_version,
            &update_agent_version,
            outcome,
        ),
    )
}

#[derive(
    Debug,
    Deserialize,
    Serialize,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(deny_unknown_fields)]
pub struct DownloadUpdateRequest {
    pub version: String,
    pub release_id: String,
    pub package_filename: String,
}

fn validate_download_target(
    target: &DownloadUpdateRequest,
    manifest: &ValidatedManifest,
) -> Result<(), String> {
    if target.version
        != manifest.version
        || target.release_id
            != manifest.release_id
        || target.package_filename
            != manifest.package_filename
    {
        return Err(
            concat!(
                "requested update identity no longer ",
                "matches the current signed manifest"
            )
            .to_string()
        );
    }

    Ok(())
}

fn update_staging_directory(
    runtime_root: &Path,
) -> Result<PathBuf, String> {
    if runtime_root
        .as_os_str()
        .is_empty()
        || !runtime_root.is_absolute()
    {
        return Err(
            "runtime directory must be an absolute path"
                .to_string()
        );
    }

    if runtime_root
        .components()
        .any(
            |component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                )
            }
        )
    {
        return Err(
            "runtime directory cannot contain parent traversal"
                .to_string()
        );
    }

    Ok(
        runtime_root
            .join("updates")
            .join("staging")
    )
}

fn package_download_request_from_manifest(
    manifest: &ValidatedManifest,
    runtime_root: &Path,
) -> Result<
    PackageDownloadRequest,
    String,
> {
    let staging_directory =
        update_staging_directory(
            runtime_root
        )?;

    Ok(
        PackageDownloadRequest {
            download_url:
                manifest
                    .package_download_url
                    .clone(),

            signature_url:
                manifest
                    .package_signature_url
                    .clone(),

            filename:
                manifest
                    .package_filename
                    .clone(),

            expected_size:
                manifest.package_size,

            expected_sha256:
                manifest
                    .package_sha256
                    .clone(),

            staging_directory,
        }
    )
}

fn staged_download_response_payload(
    target: &DownloadUpdateRequest,
    staged: &StagedPackage,
) -> Value {
    json!({
        "status":
            "staged",

        "version":
            target.version,

        "release_id":
            target.release_id,

        "package_filename":
            target.package_filename,

        "size":
            staged.size,

        "sha256":
            staged.sha256
    })
}

pub async fn download_update(
    State(state):
        State<AppState>,
    headers:
        HeaderMap,
    Json(target):
        Json<DownloadUpdateRequest>,
) -> Response {
    if let Err(response) =
        require_admin(
            &state,
            &headers,
        )
        .await
    {
        return response;
    }

    let store =
        match update_state_store(
            &state
        )
        .await
        {
            Ok(store) =>
                store,

            Err(response) =>
                return response,
        };

    let channel =
        match store
            .get_channel()
            .await
        {
            Ok(channel) =>
                channel,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "update_state_error",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    if channel
        == UpdateChannel::Disabled
    {
        return json_response(
            StatusCode::CONFLICT,
            json!({
                "error":
                    "updates_disabled",

                "message":
                    "G?ncelleme kanal? devre d???."
            }),
        );
    }

    let running_version =
        state
            .settings
            .release
            .version
            .clone();

    /*
     * The Agent version is used only for signed
     * compatibility evaluation here. No package is
     * uploaded to the Agent in Task 8B.
     */
    let update_agent_version =
        match fetch_update_agent_version()
            .await
        {
            Ok(version) =>
                version,

            Err(response) =>
                return response,
        };

    let discovery =
        match UpdateDiscoveryService::new()
        {
            Ok(service) =>
                service,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "discovery_initialization_failed",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    /*
     * Re-fetch and re-verify the signed manifest at
     * download time. The client never supplies URL,
     * expected hash, expected size or signature URL.
     */
    let (
        manifest,
        outcome,
    ) =
        match discovery
            .fetch_validated_manifest_for_versions(
                &store,
                channel,
                &running_version,
                &update_agent_version,
            )
            .await
        {
            Ok(result) =>
                result,

            Err(error) => {
                return json_response(
                    StatusCode::BAD_GATEWAY,
                    json!({
                        "error":
                            "signed_manifest_refresh_failed",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    match outcome {
        DiscoveryOutcome::UpdateAvailable {
            ..
        } => {}

        DiscoveryOutcome::UpToDate => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "already_up_to_date"
                }),
            );
        }

        DiscoveryOutcome::Incompatible => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "update_incompatible"
                }),
            );
        }

        DiscoveryOutcome::ReplayRejected => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "manifest_replay_rejected"
                }),
            );
        }

        DiscoveryOutcome::Disabled => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "updates_disabled"
                }),
            );
        }
    }

    /*
     * Bind the user's explicit selection to the
     * newly verified current signed manifest.
     */
    if let Err(error) =
        validate_download_target(
            &target,
            &manifest,
        )
    {
        return json_response(
            StatusCode::CONFLICT,
            json!({
                "error":
                    "update_target_changed",

                "message":
                    error
            }),
        );
    }

    let runtime_root =
        Path::new(
            &state
                .settings
                .runtime
                .directory
        );

    let request =
        match package_download_request_from_manifest(
            &manifest,
            runtime_root,
        )
        {
            Ok(request) =>
                request,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "invalid_update_staging_path",

                        "message":
                            error
                    }),
                );
            }
        };

    let downloader =
        match PackageDownloadService::new()
        {
            Ok(service) =>
                service,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "download_initialization_failed",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    /*
     * Task 6 guarantees:
     * .partial -> exact size -> SHA256 ->
     * Ed25519 package signature -> atomic rename.
     */
    let staged =
        match downloader
            .download_and_stage(
                &request
            )
            .await
        {
            Ok(staged) =>
                staged,

            Err(error) => {
                return json_response(
                    StatusCode::BAD_GATEWAY,
                    json!({
                        "error":
                            "package_download_failed",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    json_response(
        StatusCode::CREATED,
        staged_download_response_payload(
            &target,
            &staged,
        ),
    )
}

#[derive(
    Debug,
    Deserialize,
    Serialize,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(deny_unknown_fields)]
pub struct VerifyStagedUpdateRequest {
    pub version: String,
    pub release_id: String,
    pub package_filename: String,
}

fn validate_verify_target(
    target: &VerifyStagedUpdateRequest,
    manifest: &ValidatedManifest,
) -> Result<(), String> {
    if target.version
        != manifest.version
        || target.release_id
            != manifest.release_id
        || target.package_filename
            != manifest.package_filename
    {
        return Err(
            concat!(
                "requested staged update identity ",
                "does not match the current signed manifest"
            )
            .to_string()
        );
    }

    Ok(())
}

async fn staged_package_from_signed_manifest(
    target: &VerifyStagedUpdateRequest,
    manifest: &ValidatedManifest,
    runtime_root: &Path,
) -> Result<StagedPackage, String> {
    validate_verify_target(
        target,
        manifest,
    )?;

    let staging =
        update_staging_directory(
            runtime_root
        )?;

    /*
     * Reject a symlinked staging directory.
     */
    let staging_metadata =
        fs::symlink_metadata(
            &staging
        )
        .await
        .map_err(
            |error| {
                format!(
                    "staging directory is unavailable: {error}"
                )
            }
        )?;

    if staging_metadata
        .file_type()
        .is_symlink()
        || !staging_metadata.is_dir()
    {
        return Err(
            "staging directory is not a real directory"
                .to_string()
        );
    }

    let path =
        staging.join(
            &manifest.package_filename
        );

    /*
     * symlink_metadata intentionally does not
     * follow the final staged package symlink.
     */
    let metadata =
        fs::symlink_metadata(
            &path
        )
        .await
        .map_err(
            |error| {
                format!(
                    "staged package is unavailable: {error}"
                )
            }
        )?;

    if metadata
        .file_type()
        .is_symlink()
        || !metadata.is_file()
    {
        return Err(
            "staged package must be a regular file"
                .to_string()
        );
    }

    let canonical_staging =
        fs::canonicalize(
            &staging
        )
        .await
        .map_err(
            |error| {
                format!(
                    "staging directory cannot be resolved: {error}"
                )
            }
        )?;

    let canonical_path =
        fs::canonicalize(
            &path
        )
        .await
        .map_err(
            |error| {
                format!(
                    "staged package cannot be resolved: {error}"
                )
            }
        )?;

    if canonical_path.parent()
        != Some(
            canonical_staging.as_path()
        )
    {
        return Err(
            "staged package escaped the staging directory"
                .to_string()
        );
    }

    let mut file =
        fs::File::open(
            &canonical_path
        )
        .await
        .map_err(
            |error| {
                format!(
                    "staged package cannot be opened: {error}"
                )
            }
        )?;

    let opened_metadata =
        file.metadata()
            .await
            .map_err(
                |error| {
                    format!(
                        "staged package metadata cannot be read: {error}"
                    )
                }
            )?;

    if !opened_metadata.is_file()
        || opened_metadata.len()
            != manifest.package_size
    {
        return Err(
            "staged package size does not match signed manifest"
                .to_string()
        );
    }

    let mut hasher =
        Sha256::new();

    let mut total =
        0_u64;

    let mut buffer =
        vec![
            0_u8;
            64 * 1024
        ];

    loop {
        let read =
            file.read(
                &mut buffer
            )
            .await
            .map_err(
                |error| {
                    format!(
                        "staged package cannot be read: {error}"
                    )
                }
            )?;

        if read == 0 {
            break;
        }

        total =
            total
                .checked_add(
                    read as u64
                )
                .ok_or_else(
                    || {
                        "staged package size overflow"
                            .to_string()
                    }
                )?;

        if total
            > manifest.package_size
        {
            return Err(
                "staged package exceeds signed size"
                    .to_string()
            );
        }

        hasher.update(
            &buffer[..read]
        );
    }

    if total
        != manifest.package_size
    {
        return Err(
            "staged package size changed during verification"
                .to_string()
        );
    }

    let sha256 =
        format!(
            "{:x}",
            hasher.finalize()
        );

    if sha256
        != manifest.package_sha256
    {
        return Err(
            "staged package SHA256 does not match signed manifest"
                .to_string()
        );
    }

    Ok(
        StagedPackage {
            path:
                canonical_path,

            size:
                total,

            sha256,
        }
    )
}

fn verified_agent_response_payload(
    target: &VerifyStagedUpdateRequest,
    verified: &VerifiedAgentPackage,
) -> Value {
    json!({
        "status":
            "verified",

        "version":
            verified.version,

        "release_id":
            target.release_id,

        "package_filename":
            verified
                .identity
                .filename,

        "size":
            verified
                .identity
                .size,

        "sha256":
            verified
                .identity
                .sha256
    })
}

#[derive(
    Debug,
    Deserialize,
    Serialize,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(deny_unknown_fields)]
pub struct InstallVerifiedUpdateRequest {
    pub operation_id: String,
    pub approved: bool,
}

fn explicit_install_approval(
    request:
        &InstallVerifiedUpdateRequest,
) -> Result<
    InstallApproval,
    String,
> {
    if !request.approved {
        return Err(
            "explicit install approval is required"
                .to_string()
        );
    }

    Ok(
        InstallApproval::Approved
    )
}

fn verified_operation_from_agent(
    operation_id: &str,
    target:
        &VerifyStagedUpdateRequest,
    verified:
        &VerifiedAgentPackage,
    updated_at: &str,
) -> Result<
    VerifiedUpdateOperationRecord,
    String,
> {
    if Uuid::parse_str(
        operation_id
    )
    .is_err()
    {
        return Err(
            "invalid update operation id"
                .to_string()
        );
    }

    /*
     * Reassert the exact 8C identity boundary before
     * turning Agent verification into install authority.
     */
    if verified.version
        != target.version
        || verified
            .identity
            .filename
            != target.package_filename
        || verified
            .identity
            .size
            == 0
        || verified
            .identity
            .sha256
            .len()
            != 64
        || !verified
            .identity
            .sha256
            .chars()
            .all(
                |value| {
                    value.is_ascii_digit()
                        || (
                            'a'..='f'
                        ).contains(
                            &value
                        )
                }
            )
    {
        return Err(
            "verified Agent identity is invalid"
                .to_string()
        );
    }

    Ok(
        VerifiedUpdateOperationRecord {
            operation_id:
                operation_id
                    .to_string(),

            target_version:
                target
                    .version
                    .clone(),

            release_id:
                target
                    .release_id
                    .clone(),

            package_filename:
                verified
                    .identity
                    .filename
                    .clone(),

            package_sha256:
                verified
                    .identity
                    .sha256
                    .clone(),

            package_size:
                verified
                    .identity
                    .size,

            status:
                UpdateOperationStatus
                    ::Verified,

            updated_at:
                updated_at
                    .to_string(),
        }
    )
}

fn verified_agent_package_from_operation(
    operation:
        &VerifiedUpdateOperationRecord,
) -> Result<
    VerifiedAgentPackage,
    String,
> {
    if operation.status
        != UpdateOperationStatus
            ::Installing
    {
        return Err(
            "update operation is not claimed for install"
                .to_string()
        );
    }

    let filename =
        operation
            .package_filename
            .as_str();

    if filename.is_empty()
        || Path::new(
            filename
        )
        .file_name()
        .and_then(
            |value| value.to_str()
        )
        != Some(filename)
        || !filename
            .ends_with(
                ".popkg"
            )
        || operation.package_size
            == 0
        || operation
            .package_sha256
            .len()
            != 64
        || !operation
            .package_sha256
            .chars()
            .all(
                |value| {
                    value.is_ascii_digit()
                        || (
                            'a'..='f'
                        ).contains(
                            &value
                        )
                }
            )
    {
        return Err(
            "claimed update package identity is invalid"
                .to_string()
        );
    }

    Ok(
        VerifiedAgentPackage {
            identity:
                AgentPackageIdentity {
                    filename:
                        operation
                            .package_filename
                            .clone(),

                    sha256:
                        operation
                            .package_sha256
                            .clone(),

                    size:
                        operation
                            .package_size,
                },

            version:
                operation
                    .target_version
                    .clone(),
        }
    )
}

fn verified_response_payload_with_operation(
    target:
        &VerifyStagedUpdateRequest,
    verified:
        &VerifiedAgentPackage,
    operation_id:
        &str,
) -> Value {
    json!({
        "status":
            "verified",

        "operation_id":
            operation_id,

        "version":
            verified.version,

        "release_id":
            target.release_id,

        "package_filename":
            verified
                .identity
                .filename,

        "size":
            verified
                .identity
                .size,

        "sha256":
            verified
                .identity
                .sha256
    })
}

async fn mark_update_operation_failed(
    store:
        &UpdateStateStore,
    operation:
        &VerifiedUpdateOperationRecord,
) {
    let _ =
        store
            .save_operation(
                UpdateOperationRecord {
                    operation_id:
                        operation
                            .operation_id
                            .clone(),

                    target_version:
                        operation
                            .target_version
                            .clone(),

                    status:
                        UpdateOperationStatus
                            ::Failed,

                    updated_at:
                        Utc::now()
                            .to_rfc3339(),
                }
            )
            .await;
}


pub async fn verify_staged_update(
    State(state):
        State<AppState>,
    headers:
        HeaderMap,
    Json(target):
        Json<VerifyStagedUpdateRequest>,
) -> Response {
    if let Err(response) =
        require_admin(
            &state,
            &headers,
        )
        .await
    {
        return response;
    }

    let store =
        match update_state_store(
            &state
        )
        .await
        {
            Ok(store) =>
                store,

            Err(response) =>
                return response,
        };

    let channel =
        match store
            .get_channel()
            .await
        {
            Ok(channel) =>
                channel,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "update_state_error",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    if channel
        == UpdateChannel::Disabled
    {
        return json_response(
            StatusCode::CONFLICT,
            json!({
                "error":
                    "updates_disabled"
            }),
        );
    }

    let running_version =
        state
            .settings
            .release
            .version
            .clone();

    /*
     * Read Agent version only for signed
     * compatibility evaluation.
     */
    let update_agent_version =
        match fetch_update_agent_version()
            .await
        {
            Ok(version) =>
                version,

            Err(response) =>
                return response,
        };

    let discovery =
        match UpdateDiscoveryService::new()
        {
            Ok(service) =>
                service,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "discovery_initialization_failed",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    /*
     * Re-fetch and verify the signed manifest again
     * immediately before trusting the staged package.
     */
    let (
        manifest,
        outcome,
    ) =
        match discovery
            .fetch_validated_manifest_for_versions(
                &store,
                channel,
                &running_version,
                &update_agent_version,
            )
            .await
        {
            Ok(result) =>
                result,

            Err(error) => {
                return json_response(
                    StatusCode::BAD_GATEWAY,
                    json!({
                        "error":
                            "signed_manifest_refresh_failed",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    match outcome {
        DiscoveryOutcome::UpdateAvailable {
            ..
        } => {}

        DiscoveryOutcome::UpToDate => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "already_up_to_date"
                }),
            );
        }

        DiscoveryOutcome::Incompatible => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "update_incompatible"
                }),
            );
        }

        DiscoveryOutcome::ReplayRejected => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "manifest_replay_rejected"
                }),
            );
        }

        DiscoveryOutcome::Disabled => {
            return json_response(
                StatusCode::CONFLICT,
                json!({
                    "error":
                        "updates_disabled"
                }),
            );
        }
    }

    if let Err(error) =
        validate_verify_target(
            &target,
            &manifest,
        )
    {
        return json_response(
            StatusCode::CONFLICT,
            json!({
                "error":
                    "update_target_changed",

                "message":
                    error
            }),
        );
    }

    let runtime_root =
        Path::new(
            &state
                .settings
                .runtime
                .directory
        );

    /*
     * Re-read the staged bytes from the server-owned
     * path. Client cannot supply path/hash/size.
     */
    let staged =
        match staged_package_from_signed_manifest(
            &target,
            &manifest,
            runtime_root,
        )
        .await
        {
            Ok(staged) =>
                staged,

            Err(error) => {
                return json_response(
                    StatusCode::CONFLICT,
                    json!({
                        "error":
                            "invalid_staged_package",

                        "message":
                            error
                    }),
                );
            }
        };

    let token =
        match read_update_token()
            .await
        {
            Ok(token) =>
                token,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "agent_configuration_error",

                        "message":
                            error
                    }),
                );
            }
        };

    let flow =
        UpdateAgentVerificationFlow::new(
            AGENT_BASE_URL
                .to_string(),

            Some(token),
        );

    /*
     * Task 7 performs:
     * upload -> exact filename/hash/size match ->
     * verify -> exact filename/hash/version match.
     *
     * It does NOT install.
     */
    let verified =
        match flow
            .upload_and_verify(
                &staged,
                &manifest.version,
            )
            .await
        {
            Ok(verified) =>
                verified,

            Err(error) => {
                return json_response(
                    StatusCode::BAD_GATEWAY,
                    json!({
                        "error":
                            "agent_package_verification_failed",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    let operation_id =
        Uuid::new_v4()
            .to_string();

    let operation =
        match verified_operation_from_agent(
            &operation_id,
            &target,
            &verified,
            &Utc::now()
                .to_rfc3339(),
        ) {
            Ok(operation) =>
                operation,

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "verified_operation_invalid",

                        "message":
                            error
                    }),
                );
            }
        };

    if let Err(error) =
        store
            .save_verified_operation(
                operation
            )
            .await
    {
        return json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({
                "error":
                    "verified_operation_persist_failed",

                "message":
                    error.to_string()
            }),
        );
    }

    json_response(
        StatusCode::OK,
        verified_response_payload_with_operation(
            &target,
            &verified,
            &operation_id,
        ),
    )
}

pub async fn install_verified_update(
    State(state):
        State<AppState>,
    headers:
        HeaderMap,
    Json(request):
        Json<InstallVerifiedUpdateRequest>,
) -> Response {
    if let Err(response) =
        require_admin(
            &state,
            &headers,
        )
        .await
    {
        return response;
    }

    /*
     * Explicit approval is checked before update-state
     * lookup/claim and before any Agent network request.
     */
    let approval =
        match explicit_install_approval(
            &request
        ) {
            Ok(approval) =>
                approval,

            Err(error) => {
                return json_response(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error":
                            "install_approval_required",

                        "message":
                            error
                    }),
                );
            }
        };

    let operation_id =
        request
            .operation_id
            .trim();

    if Uuid::parse_str(
        operation_id
    )
    .is_err()
    {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error":
                    "invalid_operation_id"
            }),
        );
    }

    let store =
        match update_state_store(
            &state
        )
        .await
        {
            Ok(store) =>
                store,

            Err(response) =>
                return response,
        };

    /*
     * Atomic verified -> installing transition.
     * A replay/concurrent request cannot claim the
     * same install authority twice.
     */
    let claimed =
        match store
            .claim_verified_operation_for_install(
                operation_id,
                &Utc::now()
                    .to_rfc3339(),
            )
            .await
        {
            Ok(
                Some(operation)
            ) =>
                operation,

            Ok(None) => {
                return json_response(
                    StatusCode::CONFLICT,
                    json!({
                        "error":
                            "verified_operation_unavailable",

                        "message":
                            "Operation is unknown, already used, or no longer verified."
                    }),
                );
            }

            Err(error) => {
                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "update_state_error",

                        "message":
                            error.to_string()
                    }),
                );
            }
        };

    /*
     * Reconstruct Agent identity exclusively from the
     * immutable DB record. No client filename/hash/size
     * participates in this step.
     */
    let verified =
        match verified_agent_package_from_operation(
            &claimed
        ) {
            Ok(verified) =>
                verified,

            Err(error) => {
                mark_update_operation_failed(
                    &store,
                    &claimed,
                )
                .await;

                return json_response(
                    StatusCode::CONFLICT,
                    json!({
                        "error":
                            "verified_operation_invalid",

                        "message":
                            error
                    }),
                );
            }
        };

    let token =
        match read_update_token()
            .await
        {
            Ok(token) =>
                token,

            Err(error) => {
                mark_update_operation_failed(
                    &store,
                    &claimed,
                )
                .await;

                return json_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error":
                            "agent_configuration_error",

                        "message":
                            error
                    }),
                );
            }
        };

    let flow =
        UpdateAgentVerificationFlow::new(
            AGENT_BASE_URL
                .to_string(),

            Some(token),
        );

    /*
     * Update Agent itself performs fresh package
     * verification again inside its install helper,
     * including SHA256 and package content validation.
     *
     * InstallApproval::Approved remains an additional
     * Task-7 network gate.
     */
    match flow
        .install_verified(
            &verified,
            approval,
        )
        .await
    {
        Ok(()) => {
            /*
             * Leave the operation in `installing`.
             * Installing the release restarts PhotoOS,
             * so this caller is not guaranteed to live
             * long enough to persist a terminal status.
             */
            json_response(
                StatusCode::ACCEPTED,
                json!({
                    "status":
                        "installing",

                    "operation_id":
                        claimed.operation_id,

                    "version":
                        claimed.target_version,

                    "release_id":
                        claimed.release_id,

                    "package_filename":
                        claimed.package_filename
                }),
            )
        }

        Err(error) => {
            mark_update_operation_failed(
                &store,
                &claimed,
            )
            .await;

            json_response(
                StatusCode::BAD_GATEWAY,
                json!({
                    "error":
                        "agent_install_failed",

                    "message":
                        error.to_string(),

                    "operation_id":
                        claimed.operation_id
                }),
            )
        }
    }
}


#[cfg(test)]
mod task8a_discovery_api_tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::{
        update_discovery::{
            DiscoveryOutcome,
            UpdateDiscoveryService,
        },
        update_state::{
            UpdateChannel,
            UpdateStateStore,
        },
    };

    use super::{
        check_for_updates,
        check_updates_with_service,
        discovery_response_payload,
        get_update_channel,
        parse_update_channel_name,
        persist_update_channel_name,
        set_update_channel,
    };

    async fn test_store() -> UpdateStateStore {
        let pool =
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect(
                    "sqlite::memory:"
                )
                .await
                .expect(
                    "create task8a database"
                );

        let store =
            UpdateStateStore::new(
                pool
            );

        store
            .ensure_schema()
            .await
            .expect(
                "initialize update schema"
            );

        store
    }

    #[test]
    fn task8a_channel_parser_is_strict() {
        assert_eq!(
            parse_update_channel_name(
                "stable"
            )
            .unwrap(),
            UpdateChannel::Stable
        );

        assert_eq!(
            parse_update_channel_name(
                "beta"
            )
            .unwrap(),
            UpdateChannel::Beta
        );

        assert_eq!(
            parse_update_channel_name(
                "disabled"
            )
            .unwrap(),
            UpdateChannel::Disabled
        );

        assert!(
            parse_update_channel_name(
                "Stable"
            )
            .is_err()
        );

        assert!(
            parse_update_channel_name(
                "nightly"
            )
            .is_err()
        );

        assert!(
            parse_update_channel_name(
                ""
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn task8a_channel_change_persists() {
        let store =
            test_store().await;

        let channel =
            persist_update_channel_name(
                &store,
                "beta",
            )
            .await
            .expect(
                "persist beta channel"
            );

        assert_eq!(
            channel,
            UpdateChannel::Beta
        );

        assert_eq!(
            store
                .get_channel()
                .await
                .expect(
                    "read channel"
                ),
            UpdateChannel::Beta
        );

        store.close().await;
    }

    #[tokio::test]
    async fn task8a_disabled_channel_check_never_needs_network() {
        let store =
            test_store().await;

        store
            .set_channel(
                UpdateChannel::Disabled
            )
            .await
            .expect(
                "disable update discovery"
            );

        let service =
            UpdateDiscoveryService
                ::new_for_test(
                    "http://127.0.0.1:1/manifest",
                    "http://127.0.0.1:1/manifest.sig",
                )
                .expect(
                    "test discovery service"
                );

        let (
            channel,
            outcome,
        ) =
            check_updates_with_service(
                &service,
                &store,
                "1.2.4",
                "1.0.0",
            )
            .await
            .expect(
                "disabled check"
            );

        assert_eq!(
            channel,
            UpdateChannel::Disabled
        );

        assert_eq!(
            outcome,
            DiscoveryOutcome::Disabled
        );

        store.close().await;
    }

    #[test]
    fn task8a_update_available_payload_has_no_install_side_effect() {
        let payload =
            discovery_response_payload(
                UpdateChannel::Beta,
                "1.2.2",
                "1.0.0",
                DiscoveryOutcome::UpdateAvailable {
                    version:
                        "1.2.3".to_string(),

                    release_id:
                        "1.2.3-test"
                            .to_string(),

                    package_filename:
                        "PhotoOS-test.popkg"
                            .to_string(),
                },
            );

        assert_eq!(
            payload["channel"],
            "beta"
        );

        assert_eq!(
            payload["running_version"],
            "1.2.2"
        );

        assert_eq!(
            payload[
                "update_agent_version"
            ],
            "1.0.0"
        );

        assert_eq!(
            payload["status"],
            "update_available"
        );

        assert_eq!(
            payload[
                "update_available"
            ],
            true
        );

        assert_eq!(
            payload["version"],
            "1.2.3"
        );

        assert_eq!(
            payload["release_id"],
            "1.2.3-test"
        );

        assert_eq!(
            payload[
                "package_filename"
            ],
            "PhotoOS-test.popkg"
        );

        assert!(
            payload
                .get("install")
                .is_none()
        );

        assert!(
            payload
                .get("installed")
                .is_none()
        );

        assert!(
            payload
                .get("downloaded")
                .is_none()
        );

        assert!(
            payload
                .get("download_url")
                .is_none()
        );
    }

    #[test]
    fn task8a_api_handler_symbols_exist() {
        /*
         * Compile-time API contract.
         *
         * These handlers must remain separate from
         * manual upload/verify/install handlers.
         */
        let _ =
            get_update_channel;

        let _ =
            set_update_channel;

        let _ =
            check_for_updates;
    }
}

#[cfg(test)]
mod task8b_download_api_tests {
    use std::path::{
        Path,
        PathBuf,
    };

    use crate::{
        update_discovery::ValidatedManifest,
        update_download::StagedPackage,
    };

    use super::{
        DownloadUpdateRequest,
        download_update,
        package_download_request_from_manifest,
        staged_download_response_payload,
        update_staging_directory,
        validate_download_target,
    };

    fn manifest() -> ValidatedManifest {
        ValidatedManifest {
            version:
                "1.2.4".to_string(),

            release_id:
                "1.2.4-test-release"
                    .to_string(),

            published_at:
                "2026-09-22T20:00:00Z"
                    .to_string(),

            source_commit:
                "a".repeat(40),

            package_filename:
                "PhotoOS-1.2.4-test.popkg"
                    .to_string(),

            package_size:
                123_456,

            package_sha256:
                "b".repeat(64),

            package_signature_url:
                concat!(
                    "https://github.com/",
                    "deniz3446/PhotoOS-Updates/",
                    "releases/download/v1.2.4/",
                    "PhotoOS-1.2.4-test.popkg.sig"
                )
                .to_string(),

            package_download_url:
                concat!(
                    "https://github.com/",
                    "deniz3446/PhotoOS-Updates/",
                    "releases/download/v1.2.4/",
                    "PhotoOS-1.2.4-test.popkg"
                )
                .to_string(),

            minimum_photoos_version:
                "1.2.1".to_string(),

            minimum_update_agent_version:
                "1.0.0".to_string(),

            release_notes:
                vec![],
        }
    }

    fn target() -> DownloadUpdateRequest {
        DownloadUpdateRequest {
            version:
                "1.2.4".to_string(),

            release_id:
                "1.2.4-test-release"
                    .to_string(),

            package_filename:
                "PhotoOS-1.2.4-test.popkg"
                    .to_string(),
        }
    }

    #[test]
    fn task8b_target_identity_must_match_signed_manifest_exactly() {
        let manifest =
            manifest();

        let exact =
            target();

        validate_download_target(
            &exact,
            &manifest,
        )
        .expect(
            "exact signed identity must match"
        );

        let mut wrong_version =
            exact.clone();

        wrong_version.version =
            "1.2.5".to_string();

        assert!(
            validate_download_target(
                &wrong_version,
                &manifest,
            )
            .is_err()
        );

        let mut wrong_release =
            exact.clone();

        wrong_release.release_id =
            "different-release"
                .to_string();

        assert!(
            validate_download_target(
                &wrong_release,
                &manifest,
            )
            .is_err()
        );

        let mut wrong_filename =
            exact;

        wrong_filename.package_filename =
            "PhotoOS-other.popkg"
                .to_string();

        assert!(
            validate_download_target(
                &wrong_filename,
                &manifest,
            )
            .is_err()
        );
    }

    #[test]
    fn task8b_client_target_contains_no_url_hash_or_size_fields() {
        let request =
            serde_json::to_value(
                target()
            )
            .expect(
                "serialize download target"
            );

        assert_eq!(
            request["version"],
            "1.2.4"
        );

        assert_eq!(
            request["release_id"],
            "1.2.4-test-release"
        );

        assert_eq!(
            request["package_filename"],
            "PhotoOS-1.2.4-test.popkg"
        );

        for forbidden in [
            "download_url",
            "signature_url",
            "sha256",
            "size",
            "expected_size",
            "expected_sha256",
            "staging_directory",
        ] {
            assert!(
                request
                    .get(forbidden)
                    .is_none(),
                "client must not control {forbidden}"
            );
        }
    }

    #[test]
    fn task8b_staging_directory_is_beneath_runtime_root() {
        let runtime =
            Path::new(
                "/var/lib/photoos/runtime"
            );

        let staging =
            update_staging_directory(
                runtime
            )
            .expect(
                "valid absolute runtime root"
            );

        assert_eq!(
            staging,
            PathBuf::from(
                "/var/lib/photoos/runtime/updates/staging"
            )
        );

        assert!(
            staging.starts_with(
                runtime
            )
        );
    }

    #[test]
    fn task8b_relative_or_empty_runtime_root_is_rejected() {
        assert!(
            update_staging_directory(
                Path::new("")
            )
            .is_err()
        );

        assert!(
            update_staging_directory(
                Path::new(
                    "relative/runtime"
                )
            )
            .is_err()
        );
    }

    #[test]
    fn task8b_downloader_request_is_derived_only_from_signed_manifest() {
        let manifest =
            manifest();

        let request =
            package_download_request_from_manifest(
                &manifest,
                Path::new(
                    "/var/lib/photoos/runtime"
                ),
            )
            .expect(
                "build downloader request"
            );

        assert_eq!(
            request.filename,
            manifest.package_filename
        );

        assert_eq!(
            request.download_url,
            manifest.package_download_url
        );

        assert_eq!(
            request.signature_url,
            manifest.package_signature_url
        );

        assert_eq!(
            request.expected_size,
            manifest.package_size
        );

        assert_eq!(
            request.expected_sha256,
            manifest.package_sha256
        );

        assert_eq!(
            request.staging_directory,
            PathBuf::from(
                "/var/lib/photoos/runtime/updates/staging"
            )
        );
    }

    #[test]
    fn task8b_staged_response_has_no_agent_or_install_side_effect_contract() {
        let staged =
            StagedPackage {
                path:
                    PathBuf::from(
                        concat!(
                            "/var/lib/photoos/runtime/",
                            "updates/staging/",
                            "PhotoOS-1.2.4-test.popkg"
                        )
                    ),

                size:
                    123_456,

                sha256:
                    "b".repeat(64),
            };

        let payload =
            staged_download_response_payload(
                &target(),
                &staged,
            );

        assert_eq!(
            payload["status"],
            "staged"
        );

        assert_eq!(
            payload["version"],
            "1.2.4"
        );

        assert_eq!(
            payload["release_id"],
            "1.2.4-test-release"
        );

        assert_eq!(
            payload["package_filename"],
            "PhotoOS-1.2.4-test.popkg"
        );

        assert_eq!(
            payload["size"],
            123_456
        );

        assert_eq!(
            payload["sha256"],
            "b".repeat(64)
        );

        /*
         * Do not expose a server filesystem path.
         */
        assert!(
            payload
                .get("path")
                .is_none()
        );

        /*
         * Staging is not Agent verification
         * and definitely not installation.
         */
        for forbidden in [
            "installed",
            "install",
            "install_started",
            "agent_verified",
            "uploaded_to_agent",
        ] {
            assert!(
                payload
                    .get(forbidden)
                    .is_none()
            );
        }

        /*
         * Compile-time API contract.
         */
        let _ =
            download_update;
    }
}

#[cfg(test)]
mod task8c_agent_verify_api_tests {
    use std::{
        path::{
            Path,
            PathBuf,
        },
        time::{
            SystemTime,
            UNIX_EPOCH,
        },
    };

    use sha2::{
        Digest,
        Sha256,
    };

    use crate::{
        update_agent_flow::{
            AgentPackageIdentity,
            VerifiedAgentPackage,
        },
        update_discovery::ValidatedManifest,
    };

    use super::{
        VerifyStagedUpdateRequest,
        staged_package_from_signed_manifest,
        validate_verify_target,
        verified_agent_response_payload,
        verify_staged_update,
    };

    fn unique_runtime(
        label: &str,
    ) -> PathBuf {
        let stamp =
            SystemTime::now()
                .duration_since(
                    UNIX_EPOCH
                )
                .expect(
                    "clock"
                )
                .as_nanos();

        std::env::temp_dir()
            .join(
                format!(
                    "photoos-task8c-{label}-{}-{stamp}",
                    std::process::id()
                )
            )
    }

    fn sha256(
        bytes: &[u8],
    ) -> String {
        format!(
            "{:x}",
            Sha256::digest(
                bytes
            )
        )
    }

    fn manifest_for(
        bytes: &[u8],
    ) -> ValidatedManifest {
        ValidatedManifest {
            version:
                "1.2.4".to_string(),

            release_id:
                "1.2.4-test-release"
                    .to_string(),

            published_at:
                "2026-09-22T20:00:00Z"
                    .to_string(),

            source_commit:
                "a".repeat(40),

            package_filename:
                "PhotoOS-1.2.4-test.popkg"
                    .to_string(),

            package_size:
                bytes.len() as u64,

            package_sha256:
                sha256(bytes),

            package_signature_url:
                concat!(
                    "https://github.com/",
                    "deniz3446/PhotoOS-Updates/",
                    "releases/download/v1.2.4/",
                    "PhotoOS-1.2.4-test.popkg.sig"
                )
                .to_string(),

            package_download_url:
                concat!(
                    "https://github.com/",
                    "deniz3446/PhotoOS-Updates/",
                    "releases/download/v1.2.4/",
                    "PhotoOS-1.2.4-test.popkg"
                )
                .to_string(),

            minimum_photoos_version:
                "1.2.1".to_string(),

            minimum_update_agent_version:
                "1.0.0".to_string(),

            release_notes:
                vec![],
        }
    }

    fn target(
    ) -> VerifyStagedUpdateRequest {
        VerifyStagedUpdateRequest {
            version:
                "1.2.4".to_string(),

            release_id:
                "1.2.4-test-release"
                    .to_string(),

            package_filename:
                "PhotoOS-1.2.4-test.popkg"
                    .to_string(),
        }
    }

    #[test]
    fn task8c_client_cannot_control_path_hash_or_size() {
        let value =
            serde_json::to_value(
                target()
            )
            .expect(
                "serialize verify target"
            );

        assert_eq!(
            value["version"],
            "1.2.4"
        );

        assert_eq!(
            value["release_id"],
            "1.2.4-test-release"
        );

        assert_eq!(
            value[
                "package_filename"
            ],
            "PhotoOS-1.2.4-test.popkg"
        );

        for forbidden in [
            "path",
            "staged_path",
            "sha256",
            "size",
            "download_url",
            "signature_url",
            "token",
            "install",
            "approved",
        ] {
            assert!(
                value
                    .get(forbidden)
                    .is_none(),
                "client must not control {forbidden}"
            );
        }
    }

    #[test]
    fn task8c_target_identity_must_match_signed_manifest() {
        let bytes =
            b"signed-package";

        let manifest =
            manifest_for(
                bytes
            );

        let exact =
            target();

        validate_verify_target(
            &exact,
            &manifest,
        )
        .expect(
            "exact identity"
        );

        let mut wrong =
            exact.clone();

        wrong.package_filename =
            "other.popkg"
                .to_string();

        assert!(
            validate_verify_target(
                &wrong,
                &manifest,
            )
            .is_err()
        );

        let mut wrong =
            target();

        wrong.version =
            "1.2.5".to_string();

        assert!(
            validate_verify_target(
                &wrong,
                &manifest,
            )
            .is_err()
        );

        let mut wrong =
            target();

        wrong.release_id =
            "other-release"
                .to_string();

        assert!(
            validate_verify_target(
                &wrong,
                &manifest,
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn task8c_staged_package_is_rehashed_from_server_path() {
        let bytes =
            b"photoos-verified-stage";

        let runtime =
            unique_runtime(
                "rehash"
            );

        let staging =
            runtime
                .join("updates")
                .join("staging");

        tokio::fs::create_dir_all(
            &staging
        )
        .await
        .expect(
            "create staging"
        );

        let manifest =
            manifest_for(
                bytes
            );

        let path =
            staging.join(
                &manifest
                    .package_filename
            );

        tokio::fs::write(
            &path,
            bytes,
        )
        .await
        .expect(
            "write staged package"
        );

        let staged =
            staged_package_from_signed_manifest(
                &target(),
                &manifest,
                &runtime,
            )
            .await
            .expect(
                "revalidate staged package"
            );

        assert_eq!(
            staged.path,
            path
        );

        assert_eq!(
            staged.size,
            bytes.len() as u64
        );

        assert_eq!(
            staged.sha256,
            sha256(bytes)
        );

        let _ =
            tokio::fs::remove_dir_all(
                &runtime
            )
            .await;
    }

    #[tokio::test]
    async fn task8c_tampered_staged_package_is_rejected() {
        let signed_bytes =
            b"signed-content";

        let tampered_bytes =
            b"tampered-content";

        let runtime =
            unique_runtime(
                "tampered"
            );

        let staging =
            runtime
                .join("updates")
                .join("staging");

        tokio::fs::create_dir_all(
            &staging
        )
        .await
        .expect(
            "create staging"
        );

        let manifest =
            manifest_for(
                signed_bytes
            );

        let path =
            staging.join(
                &manifest
                    .package_filename
            );

        tokio::fs::write(
            &path,
            tampered_bytes,
        )
        .await
        .expect(
            "write tampered package"
        );

        assert!(
            staged_package_from_signed_manifest(
                &target(),
                &manifest,
                &runtime,
            )
            .await
            .is_err()
        );

        let _ =
            tokio::fs::remove_dir_all(
                &runtime
            )
            .await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn task8c_staged_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let bytes =
            b"signed-content";

        let runtime =
            unique_runtime(
                "symlink"
            );

        let staging =
            runtime
                .join("updates")
                .join("staging");

        tokio::fs::create_dir_all(
            &staging
        )
        .await
        .expect(
            "create staging"
        );

        let manifest =
            manifest_for(
                bytes
            );

        let outside =
            runtime.join(
                "outside.popkg"
            );

        tokio::fs::write(
            &outside,
            bytes,
        )
        .await
        .expect(
            "write outside package"
        );

        let staged_path =
            staging.join(
                &manifest
                    .package_filename
            );

        symlink(
            &outside,
            &staged_path,
        )
        .expect(
            "create symlink"
        );

        assert!(
            staged_package_from_signed_manifest(
                &target(),
                &manifest,
                &runtime,
            )
            .await
            .is_err()
        );

        let _ =
            tokio::fs::remove_dir_all(
                &runtime
            )
            .await;
    }

    #[test]
    fn task8c_verified_response_has_no_install_contract() {
        let verified =
            VerifiedAgentPackage {
                identity:
                    AgentPackageIdentity {
                        filename:
                            "PhotoOS-1.2.4-test.popkg"
                                .to_string(),

                        sha256:
                            "b".repeat(64),

                        size:
                            123_456,
                    },

                version:
                    "1.2.4".to_string(),
            };

        let payload =
            verified_agent_response_payload(
                &target(),
                &verified,
            );

        assert_eq!(
            payload["status"],
            "verified"
        );

        assert_eq!(
            payload["version"],
            "1.2.4"
        );

        assert_eq!(
            payload[
                "package_filename"
            ],
            "PhotoOS-1.2.4-test.popkg"
        );

        assert_eq!(
            payload["sha256"],
            "b".repeat(64)
        );

        assert_eq!(
            payload["size"],
            123_456
        );

        /*
         * Filesystem location and install controls
         * must not be exposed by this endpoint.
         */
        for forbidden in [
            "path",
            "installed",
            "install",
            "install_started",
            "approved",
        ] {
            assert!(
                payload
                    .get(forbidden)
                    .is_none()
            );
        }

        /*
         * Compile-time production handler contract.
         */
        let _ =
            verify_staged_update;
    }
}

#[cfg(test)]
mod task8d2_install_verified_api_tests {
    use serde_json::{
        json,
        to_value,
    };

    use crate::{
        update_agent_flow::{
            AgentPackageIdentity,
            InstallApproval,
            VerifiedAgentPackage,
        },
        update_state::{
            UpdateOperationStatus,
            VerifiedUpdateOperationRecord,
        },
    };

    use super::{
        InstallVerifiedUpdateRequest,
        VerifyStagedUpdateRequest,
        explicit_install_approval,
        install_verified_update,
        verified_agent_package_from_operation,
        verified_operation_from_agent,
        verified_response_payload_with_operation,
    };

    fn target(
    ) -> VerifyStagedUpdateRequest {
        VerifyStagedUpdateRequest {
            version:
                "1.2.4"
                    .to_string(),

            release_id:
                "1.2.4-test-release"
                    .to_string(),

            package_filename:
                "PhotoOS-1.2.4-test.popkg"
                    .to_string(),
        }
    }

    fn verified(
    ) -> VerifiedAgentPackage {
        VerifiedAgentPackage {
            identity:
                AgentPackageIdentity {
                    filename:
                        "PhotoOS-1.2.4-test.popkg"
                            .to_string(),

                    sha256:
                        "b".repeat(64),

                    size:
                        123_456,
                },

            version:
                "1.2.4"
                    .to_string(),
        }
    }

    #[test]
    fn task8d2_install_request_contains_only_operation_id_and_explicit_approval() {
        let request =
            InstallVerifiedUpdateRequest {
                operation_id:
                    "op-123"
                        .to_string(),

                approved:
                    true,
            };

        let value =
            to_value(
                &request
            )
            .expect(
                "serialize install request"
            );

        assert_eq!(
            value["operation_id"],
            "op-123"
        );

        assert_eq!(
            value["approved"],
            true
        );

        for forbidden in [
            "filename",
            "version",
            "release_id",
            "sha256",
            "size",
            "path",
            "token",
        ] {
            assert!(
                value
                    .get(forbidden)
                    .is_none(),
                "client must not control {forbidden}"
            );
        }

        /*
         * Unknown identity fields are rejected rather
         * than silently ignored.
         */
        assert!(
            serde_json::from_value::<
                InstallVerifiedUpdateRequest
            >(
                json!({
                    "operation_id":
                        "op-123",

                    "approved":
                        true,

                    "filename":
                        "evil.popkg"
                })
            )
            .is_err()
        );
    }

    #[test]
    fn task8d2_false_approval_is_rejected_before_install_authority() {
        let denied =
            InstallVerifiedUpdateRequest {
                operation_id:
                    "op-123"
                        .to_string(),

                approved:
                    false,
            };

        assert!(
            explicit_install_approval(
                &denied
            )
            .is_err()
        );

        let approved =
            InstallVerifiedUpdateRequest {
                operation_id:
                    "op-123"
                        .to_string(),

                approved:
                    true,
            };

        assert_eq!(
            explicit_install_approval(
                &approved
            )
            .expect(
                "explicit approval"
            ),
            InstallApproval::Approved
        );
    }

    #[test]
    fn task8d2_successful_8c_verification_creates_bound_operation_identity() {
        let record =
            verified_operation_from_agent(
                "550e8400-e29b-41d4-a716-446655440000",
                &target(),
                &verified(),
                "2026-09-22T20:00:00+00:00",
            )
            .expect(
                "build verified operation"
            );

        assert_eq!(
            record.operation_id,
            "550e8400-e29b-41d4-a716-446655440000"
        );

        assert_eq!(
            record.target_version,
            "1.2.4"
        );

        assert_eq!(
            record.release_id,
            "1.2.4-test-release"
        );

        assert_eq!(
            record.package_filename,
            "PhotoOS-1.2.4-test.popkg"
        );

        assert_eq!(
            record.package_sha256,
            "b".repeat(64)
        );

        assert_eq!(
            record.package_size,
            123_456
        );

        assert_eq!(
            record.status,
            UpdateOperationStatus::Verified
        );
    }

    #[test]
    fn task8d2_install_identity_is_reconstructed_only_from_claimed_db_record() {
        let operation =
            VerifiedUpdateOperationRecord {
                operation_id:
                    "op-123"
                        .to_string(),

                target_version:
                    "1.2.4"
                        .to_string(),

                release_id:
                    "1.2.4-test-release"
                        .to_string(),

                package_filename:
                    "PhotoOS-1.2.4-test.popkg"
                        .to_string(),

                package_sha256:
                    "b".repeat(64),

                package_size:
                    123_456,

                status:
                    UpdateOperationStatus
                        ::Installing,

                updated_at:
                    "2026-09-22T20:01:00+00:00"
                        .to_string(),
            };

        let package =
            verified_agent_package_from_operation(
                &operation
            )
            .expect(
                "reconstruct verified package"
            );

        assert_eq!(
            package,
            verified()
        );
    }

    #[test]
    fn task8d2_verify_response_returns_opaque_operation_id_but_no_install_flag() {
        let payload =
            verified_response_payload_with_operation(
                &target(),
                &verified(),
                "550e8400-e29b-41d4-a716-446655440000",
            );

        assert_eq!(
            payload["status"],
            "verified"
        );

        assert_eq!(
            payload["operation_id"],
            "550e8400-e29b-41d4-a716-446655440000"
        );

        assert_eq!(
            payload["version"],
            "1.2.4"
        );

        for forbidden in [
            "approved",
            "install",
            "installed",
            "path",
            "token",
        ] {
            assert!(
                payload
                    .get(forbidden)
                    .is_none()
            );
        }

        /*
         * Compile-time production handler contract.
         */
        let _ =
            install_verified_update;
    }
}
