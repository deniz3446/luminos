use std::{
    error::Error,
    fmt,
    time::Duration,
};

use chrono::DateTime;

use reqwest::{
    Client,
    redirect::Policy,
};

use serde::Deserialize;
use serde_json::Value;

use sha2::{
    Digest,
    Sha256,
};

use crate::{
    update_distribution::{
        channel_manifest_url,
        channel_signature_url,
        validate_manifest_response_size,
        validate_package_size,
        validate_release_asset_url,
        validate_signature_response_size,
        MAX_MANIFEST_BYTES,
        MAX_SIGNATURE_BYTES,
    },
    update_state::{
        ManifestReplayCandidate,
        ReplayDecision,
        UpdateChannel,
        UpdateStateError,
        UpdateStateStore,
    },
    update_trust::verify_detached_signature,
};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub enum DiscoveryOutcome {
    Disabled,

    UpToDate,

    Incompatible,

    ReplayRejected,

    UpdateAvailable {
        version: String,
        release_id: String,
        package_filename: String,
    },
}

#[derive(Debug)]
pub enum DiscoveryError {
    InvalidSignature,
    InvalidJson,
    InvalidManifest(&'static str),
    Http(reqwest::Error),
    HttpStatus(u16),
    ResponseTooLarge,
    State(UpdateStateError),
}

impl fmt::Display for DiscoveryError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::InvalidSignature => {
                write!(
                    f,
                    "update manifest signature is invalid"
                )
            }

            Self::InvalidJson => {
                write!(
                    f,
                    "signed update manifest is not valid JSON"
                )
            }

            Self::InvalidManifest(reason) => {
                write!(
                    f,
                    "invalid signed update manifest: {reason}"
                )
            }

            Self::Http(error) => {
                write!(
                    f,
                    "update discovery HTTP error: {error}"
                )
            }

            Self::HttpStatus(status) => {
                write!(
                    f,
                    "update discovery returned HTTP status {status}"
                )
            }

            Self::ResponseTooLarge => {
                write!(
                    f,
                    "update discovery response exceeded the allowed size"
                )
            }

            Self::State(error) => {
                write!(
                    f,
                    "update discovery state error: {error}"
                )
            }
        }
    }
}

impl Error for DiscoveryError {
    fn source(
        &self,
    ) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http(error) =>
                Some(error),

            Self::State(error) =>
                Some(error),

            _ => None,
        }
    }
}

impl From<reqwest::Error> for DiscoveryError {
    fn from(
        value: reqwest::Error,
    ) -> Self {
        Self::Http(value)
    }
}

impl From<UpdateStateError> for DiscoveryError {
    fn from(
        value: UpdateStateError,
    ) -> Self {
        Self::State(value)
    }
}

#[derive(Debug, Deserialize)]
struct ManifestDocument {
    schema_version: u64,
    product: String,
    channel: String,
    version: String,
    release_id: String,
    published_at: String,
    source_commit: String,
    package: ManifestPackage,
    minimums: ManifestMinimums,

    #[serde(default)]
    release_notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ManifestPackage {
    filename: String,
    size: u64,
    sha256: String,
    signature_url: String,
    download_url: String,
}

#[derive(Debug, Deserialize)]
struct ManifestMinimums {
    photoos_version: String,
    update_agent_version: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct ValidatedManifest {
    pub version: String,
    pub release_id: String,
    pub published_at: String,
    pub source_commit: String,

    pub package_filename: String,
    pub package_size: u64,
    pub package_sha256: String,
    pub package_signature_url: String,
    pub package_download_url: String,

    pub minimum_photoos_version: String,
    pub minimum_update_agent_version: String,

    pub release_notes: Vec<String>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
struct NumericVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

fn parse_version(
    value: &str,
) -> Result<NumericVersion, DiscoveryError> {
    let mut parts =
        value.split('.');

    let major =
        parts.next();

    let minor =
        parts.next();

    let patch =
        parts.next();

    if major.is_none()
        || minor.is_none()
        || patch.is_none()
        || parts.next().is_some()
    {
        return Err(
            DiscoveryError::InvalidManifest(
                "version must be major.minor.patch"
            )
        );
    }

    fn parse_part(
        raw: &str,
    ) -> Option<u64> {
        if raw.is_empty() {
            return None;
        }

        if raw.len() > 1
            && raw.starts_with('0')
        {
            return None;
        }

        if !raw
            .bytes()
            .all(
                |byte| byte.is_ascii_digit()
            )
        {
            return None;
        }

        raw.parse::<u64>().ok()
    }

    let major =
        parse_part(
            major.unwrap()
        )
        .ok_or(
            DiscoveryError::InvalidManifest(
                "invalid major version"
            )
        )?;

    let minor =
        parse_part(
            minor.unwrap()
        )
        .ok_or(
            DiscoveryError::InvalidManifest(
                "invalid minor version"
            )
        )?;

    let patch =
        parse_part(
            patch.unwrap()
        )
        .ok_or(
            DiscoveryError::InvalidManifest(
                "invalid patch version"
            )
        )?;

    Ok(
        NumericVersion {
            major,
            minor,
            patch,
        }
    )
}

fn valid_lower_hex(
    value: &str,
    expected_len: usize,
) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(
                |byte| {
                    byte.is_ascii_digit()
                        || (
                            b'a'..=b'f'
                        )
                        .contains(
                            &byte
                        )
                }
            )
}

fn valid_release_id(
    version: &str,
    release_id: &str,
) -> bool {
    if release_id.len() > 160 {
        return false;
    }

    let prefix =
        format!(
            "{version}-"
        );

    release_id
        .starts_with(
            &prefix
        )
        && release_id
            .bytes()
            .all(
                |byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(
                            byte,
                            b'.'
                                | b'-'
                                | b'_'
                        )
                }
            )
}

fn expected_channel_name(
    channel: UpdateChannel,
) -> Result<
    &'static str,
    DiscoveryError,
> {
    match channel {
        UpdateChannel::Stable =>
            Ok("stable"),

        UpdateChannel::Beta =>
            Ok("beta"),

        UpdateChannel::Disabled =>
            Err(
                DiscoveryError::InvalidManifest(
                    "disabled channel has no remote manifest"
                )
            ),
    }
}

pub fn validate_manifest_value(
    value: &Value,
    expected_channel: UpdateChannel,
) -> Result<
    ValidatedManifest,
    DiscoveryError,
> {
    let manifest:
        ManifestDocument =
        serde_json::from_value(
            value.clone()
        )
        .map_err(
            |_| DiscoveryError::InvalidJson
        )?;

    if manifest.schema_version != 1 {
        return Err(
            DiscoveryError::InvalidManifest(
                "unsupported schema_version"
            )
        );
    }

    if manifest.product != "PhotoOS" {
        return Err(
            DiscoveryError::InvalidManifest(
                "product must be PhotoOS"
            )
        );
    }

    let expected_channel_name =
        expected_channel_name(
            expected_channel
        )?;

    if manifest.channel
        != expected_channel_name
    {
        return Err(
            DiscoveryError::InvalidManifest(
                "manifest channel mismatch"
            )
        );
    }

    parse_version(
        &manifest.version
    )?;

    if !valid_release_id(
        &manifest.version,
        &manifest.release_id,
    ) {
        return Err(
            DiscoveryError::InvalidManifest(
                "invalid release_id"
            )
        );
    }

    DateTime::parse_from_rfc3339(
        &manifest.published_at
    )
    .map_err(
        |_| {
            DiscoveryError::InvalidManifest(
                "invalid published_at"
            )
        }
    )?;

    if !valid_lower_hex(
        &manifest.source_commit,
        40,
    ) {
        return Err(
            DiscoveryError::InvalidManifest(
                "source_commit must be 40 lowercase hex characters"
            )
        );
    }

    if !valid_lower_hex(
        &manifest.package.sha256,
        64,
    ) {
        return Err(
            DiscoveryError::InvalidManifest(
                "package SHA256 must be 64 lowercase hex characters"
            )
        );
    }

    validate_package_size(
        manifest.package.size
    )
    .map_err(
        |_| {
            DiscoveryError::InvalidManifest(
                "package size is outside the allowed range"
            )
        }
    )?;

    validate_release_asset_url(
        &manifest.package.download_url,
        &manifest.package.filename,
    )
    .map_err(
        |_| {
            DiscoveryError::InvalidManifest(
                "package download URL is not allowed"
            )
        }
    )?;

    let signature_filename =
        format!(
            "{}.sig",
            manifest.package.filename
        );

    validate_release_asset_url(
        &manifest.package.signature_url,
        &signature_filename,
    )
    .map_err(
        |_| {
            DiscoveryError::InvalidManifest(
                "package signature URL is not allowed"
            )
        }
    )?;

    parse_version(
        &manifest
            .minimums
            .photoos_version
    )?;

    parse_version(
        &manifest
            .minimums
            .update_agent_version
    )?;

    Ok(
        ValidatedManifest {
            version:
                manifest.version,

            release_id:
                manifest.release_id,

            published_at:
                manifest.published_at,

            source_commit:
                manifest.source_commit,

            package_filename:
                manifest.package.filename,

            package_size:
                manifest.package.size,

            package_sha256:
                manifest.package.sha256,

            package_signature_url:
                manifest.package.signature_url,

            package_download_url:
                manifest.package.download_url,

            minimum_photoos_version:
                manifest
                    .minimums
                    .photoos_version,

            minimum_update_agent_version:
                manifest
                    .minimums
                    .update_agent_version,

            release_notes:
                manifest.release_notes,
        }
    )
}

fn verify_and_parse_manifest(
    expected_channel: UpdateChannel,
    manifest_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<
    ValidatedManifest,
    DiscoveryError,
> {
    validate_manifest_response_size(
        manifest_bytes.len() as u64
    )
    .map_err(
        |_| DiscoveryError::ResponseTooLarge
    )?;

    validate_signature_response_size(
        signature_bytes.len() as u64
    )
    .map_err(
        |_| DiscoveryError::ResponseTooLarge
    )?;

    verify_detached_signature(
        manifest_bytes,
        signature_bytes,
    )
    .map_err(
        |_| DiscoveryError::InvalidSignature
    )?;

    let value:
        Value =
        serde_json::from_slice(
            manifest_bytes
        )
        .map_err(
            |_| DiscoveryError::InvalidJson
        )?;

    validate_manifest_value(
        &value,
        expected_channel,
    )
}

fn outcome_for_manifest_for_versions(
    running_version: &str,
    update_agent_version: &str,
    manifest: &ValidatedManifest,
) -> Result<DiscoveryOutcome, DiscoveryError> {
    let running =
        parse_version(running_version)?;

    let minimum_photoos =
        parse_version(
            &manifest.minimum_photoos_version
        )?;

    if running < minimum_photoos {
        return Ok(
            DiscoveryOutcome::Incompatible
        );
    }

    let update_agent =
        parse_version(
            update_agent_version
        )?;

    let minimum_agent =
        parse_version(
            &manifest.minimum_update_agent_version
        )?;

    if update_agent < minimum_agent {
        return Ok(
            DiscoveryOutcome::Incompatible
        );
    }

    let offered =
        parse_version(
            &manifest.version
        )?;

    if offered <= running {
        return Ok(
            DiscoveryOutcome::UpToDate
        );
    }

    Ok(
        DiscoveryOutcome::UpdateAvailable {
            version:
                manifest.version.clone(),

            release_id:
                manifest.release_id.clone(),

            package_filename:
                manifest.package_filename.clone(),
        }
    )
}

fn outcome_for_manifest(
    running_version: &str,
    manifest: &ValidatedManifest,
) -> Result<DiscoveryOutcome, DiscoveryError> {
    /*
     * Legacy Task 5 callers did not know the local
     * Update Agent version. Preserve their original
     * behavior while Task 8A production callers use
     * the explicit two-version path.
     */
    outcome_for_manifest_for_versions(
        running_version,
        &manifest.minimum_update_agent_version,
        manifest,
    )
}

pub fn evaluate_signed_manifest(
    expected_channel: UpdateChannel,
    running_version: &str,
    manifest_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<
    DiscoveryOutcome,
    DiscoveryError,
> {
    let manifest =
        verify_and_parse_manifest(
            expected_channel,
            manifest_bytes,
            signature_bytes,
        )?;

    outcome_for_manifest(
        running_version,
        &manifest,
    )
}


pub fn evaluate_signed_manifest_for_versions(
    expected_channel: UpdateChannel,
    running_version: &str,
    update_agent_version: &str,
    manifest_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<DiscoveryOutcome, DiscoveryError> {
    let manifest =
        verify_and_parse_manifest(
            expected_channel,
            manifest_bytes,
            signature_bytes,
        )?;

    outcome_for_manifest_for_versions(
        running_version,
        update_agent_version,
        &manifest,
    )
}

pub async fn evaluate_signed_manifest_with_state(
    store: &UpdateStateStore,
    expected_channel: UpdateChannel,
    running_version: &str,
    manifest_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<
    DiscoveryOutcome,
    DiscoveryError,
> {
    let manifest =
        verify_and_parse_manifest(
            expected_channel,
            manifest_bytes,
            signature_bytes,
        )?;

    let digest =
        format!(
            "{:x}",
            Sha256::digest(
                manifest_bytes
            )
        );

    let decision =
        store
            .observe_manifest(
                ManifestReplayCandidate {
                    channel:
                        expected_channel,

                    version:
                        manifest
                            .version
                            .clone(),

                    published_at:
                        manifest
                            .published_at
                            .clone(),

                    manifest_sha256:
                        digest,
                }
            )
            .await?;

    match decision {
        ReplayDecision::RejectedRegression => {
            Ok(
                DiscoveryOutcome::ReplayRejected
            )
        }

        ReplayDecision::Accepted
        | ReplayDecision::AlreadySeen => {
            outcome_for_manifest(
                running_version,
                &manifest,
            )
        }
    }
}

pub async fn evaluate_signed_manifest_with_state_for_versions(
    store: &UpdateStateStore,
    expected_channel: UpdateChannel,
    running_version: &str,
    update_agent_version: &str,
    manifest_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<DiscoveryOutcome, DiscoveryError> {
    let manifest =
        verify_and_parse_manifest(
            expected_channel,
            manifest_bytes,
            signature_bytes,
        )?;

    let digest =
        format!(
            "{:x}",
            Sha256::digest(manifest_bytes)
        );

    let decision =
        store
            .observe_manifest(
                ManifestReplayCandidate {
                    channel:
                        expected_channel,

                    version:
                        manifest.version.clone(),

                    published_at:
                        manifest.published_at.clone(),

                    manifest_sha256:
                        digest,
                }
            )
            .await?;

    match decision {
        ReplayDecision::RejectedRegression => {
            Ok(
                DiscoveryOutcome::ReplayRejected
            )
        }

        ReplayDecision::Accepted
        | ReplayDecision::AlreadySeen => {
            outcome_for_manifest_for_versions(
                running_version,
                update_agent_version,
                &manifest,
            )
        }
    }
}

pub struct UpdateDiscoveryService {
    client: Client,

    test_urls:
        Option<(
            String,
            String,
        )>,
}

impl UpdateDiscoveryService {
    pub fn new(
    ) -> Result<Self, DiscoveryError> {
        let client =
            Client::builder()
                .redirect(
                    Policy::none()
                )
                .timeout(
                    Duration::from_secs(12)
                )
                .build()?;

        Ok(
            Self {
                client,
                test_urls: None,
            }
        )
    }

    #[cfg(test)]
    pub fn new_for_test(
        manifest_url: impl Into<String>,
        signature_url: impl Into<String>,
    ) -> Result<Self, DiscoveryError> {
        let client =
            Client::builder()
                .redirect(
                    Policy::none()
                )
                .timeout(
                    Duration::from_secs(5)
                )
                .build()?;

        Ok(
            Self {
                client,

                test_urls:
                    Some(
                        (
                            manifest_url.into(),
                            signature_url.into(),
                        )
                    ),
            }
        )
    }

    fn urls(
        &self,
        channel: UpdateChannel,
    ) -> Result<
        (String, String),
        DiscoveryError,
    > {
        if let Some(
            (
                manifest,
                signature,
            )
        ) = &self.test_urls
        {
            return Ok(
                (
                    manifest.clone(),
                    signature.clone(),
                )
            );
        }

        let manifest =
            channel_manifest_url(
                channel
            )
            .ok_or(
                DiscoveryError::InvalidManifest(
                    "disabled channel has no manifest URL"
                )
            )?;

        let signature =
            channel_signature_url(
                channel
            )
            .ok_or(
                DiscoveryError::InvalidManifest(
                    "disabled channel has no signature URL"
                )
            )?;

        Ok(
            (
                manifest.to_string(),
                signature.to_string(),
            )
        )
    }

    async fn fetch_bounded(
        &self,
        url: &str,
        maximum: u64,
    ) -> Result<
        Vec<u8>,
        DiscoveryError,
    > {
        let mut response =
            self.client
                .get(url)
                .send()
                .await?;

        if response
            .status()
            .is_redirection()
        {
            return Err(
                DiscoveryError::HttpStatus(
                    response
                        .status()
                        .as_u16()
                )
            );
        }

        if !response
            .status()
            .is_success()
        {
            return Err(
                DiscoveryError::HttpStatus(
                    response
                        .status()
                        .as_u16()
                )
            );
        }

        if let Some(length) =
            response.content_length()
        {
            if length == 0
                || length > maximum
            {
                return Err(
                    DiscoveryError::ResponseTooLarge
                );
            }
        }

        let mut body =
            Vec::new();

        let mut total:
            u64 = 0;

        while let Some(chunk) =
            response.chunk().await?
        {
            total =
                total
                    .checked_add(
                        chunk.len() as u64
                    )
                    .ok_or(
                        DiscoveryError::ResponseTooLarge
                    )?;

            if total > maximum {
                return Err(
                    DiscoveryError::ResponseTooLarge
                );
            }

            body.extend_from_slice(
                &chunk
            );
        }

        if total == 0 {
            return Err(
                DiscoveryError::ResponseTooLarge
            );
        }

        Ok(body)
    }

    pub async fn check_channel(
        &self,
        store: &UpdateStateStore,
        channel: UpdateChannel,
        running_version: &str,
    ) -> Result<
        DiscoveryOutcome,
        DiscoveryError,
    > {
        if channel
            == UpdateChannel::Disabled
        {
            return Ok(
                DiscoveryOutcome::Disabled
            );
        }

        let (
            manifest_url,
            signature_url,
        ) =
            self.urls(
                channel
            )?;

        let manifest =
            self.fetch_bounded(
                &manifest_url,
                MAX_MANIFEST_BYTES,
            )
            .await?;

        let signature =
            self.fetch_bounded(
                &signature_url,
                MAX_SIGNATURE_BYTES,
            )
            .await?;

        evaluate_signed_manifest_with_state(
            store,
            channel,
            running_version,
            &manifest,
            &signature,
        )
        .await
    }

    pub async fn fetch_validated_manifest_for_versions(
        &self,
        store: &UpdateStateStore,
        channel: UpdateChannel,
        running_version: &str,
        update_agent_version: &str,
    ) -> Result<
        (
            ValidatedManifest,
            DiscoveryOutcome,
        ),
        DiscoveryError,
    > {
        if channel
            == UpdateChannel::Disabled
        {
            return Err(
                DiscoveryError::InvalidManifest(
                    "disabled channel cannot download packages"
                )
            );
        }

        let (
            manifest_url,
            signature_url,
        ) =
            self.urls(channel)?;

        let manifest_bytes =
            self.fetch_bounded(
                &manifest_url,
                MAX_MANIFEST_BYTES,
            )
            .await?;

        let signature_bytes =
            self.fetch_bounded(
                &signature_url,
                MAX_SIGNATURE_BYTES,
            )
            .await?;

        /*
         * Signature verification occurs inside
         * verify_and_parse_manifest before JSON parse.
         */
        let manifest =
            verify_and_parse_manifest(
                channel,
                &manifest_bytes,
                &signature_bytes,
            )?;

        let digest =
            format!(
                "{:x}",
                Sha256::digest(
                    &manifest_bytes
                )
            );

        let decision =
            store
                .observe_manifest(
                    ManifestReplayCandidate {
                        channel,

                        version:
                            manifest.version.clone(),

                        published_at:
                            manifest
                                .published_at
                                .clone(),

                        manifest_sha256:
                            digest,
                    }
                )
                .await?;

        let outcome =
            match decision {
                ReplayDecision::RejectedRegression => {
                    DiscoveryOutcome::ReplayRejected
                }

                ReplayDecision::Accepted
                | ReplayDecision::AlreadySeen => {
                    outcome_for_manifest_for_versions(
                        running_version,
                        update_agent_version,
                        &manifest,
                    )?
                }
            };

        Ok(
            (
                manifest,
                outcome,
            )
        )
    }

    pub async fn check_channel_for_versions(
        &self,
        store: &UpdateStateStore,
        channel: UpdateChannel,
        running_version: &str,
        update_agent_version: &str,
    ) -> Result<
        DiscoveryOutcome,
        DiscoveryError,
    > {
        if channel
            == UpdateChannel::Disabled
        {
            return Ok(
                DiscoveryOutcome::Disabled
            );
        }

        let (
            _manifest,
            outcome,
        ) =
            self.fetch_validated_manifest_for_versions(
                store,
                channel,
                running_version,
                update_agent_version,
            )
            .await?;

        Ok(outcome)
    }

}


#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    };

    use serde_json::{
        Value,
        json,
    };

    use sqlx::SqlitePool;

    use tokio::{
        io::{
            AsyncReadExt,
            AsyncWriteExt,
        },
        net::TcpListener,
        task::JoinHandle,
    };

    use crate::{
        update_state::{
            ManifestReplayCandidate,
            ReplayDecision,
            UpdateChannel,
            UpdateStateStore,
        },
    };

    use super::{
        evaluate_signed_manifest,
        evaluate_signed_manifest_with_state,
        validate_manifest_value,
        DiscoveryError,
        DiscoveryOutcome,
        UpdateDiscoveryService,
    };

    const BETA_MANIFEST: &[u8] = include_bytes!(
        "../test-fixtures/update-discovery-1.2.3/beta.json"
    );

    const BETA_SIGNATURE: &[u8] = include_bytes!(
        "../test-fixtures/update-discovery-1.2.3/beta.json.sig"
    );

    async fn memory_store() -> UpdateStateStore {
        let pool =
            SqlitePool::connect(
                "sqlite::memory:",
            )
            .await
            .expect("memory sqlite");

        let store =
            UpdateStateStore::new(pool);

        store
            .ensure_schema()
            .await
            .expect(
                "update-state schema"
            );

        store
    }

    async fn spawn_fixture_server(
    ) -> (
        String,
        Arc<AtomicUsize>,
        JoinHandle<()>,
    ) {
        let listener =
            TcpListener::bind(
                "127.0.0.1:0",
            )
            .await
            .expect(
                "bind local fixture server"
            );

        let address =
            listener
                .local_addr()
                .expect(
                    "fixture server address"
                );

        let manifest =
            BETA_MANIFEST.to_vec();

        let signature =
            BETA_SIGNATURE.to_vec();

        let requests =
            Arc::new(
                AtomicUsize::new(0)
            );

        let counter =
            Arc::clone(&requests);

        let handle =
            tokio::spawn(
                async move {
                    for _ in 0..2 {
                        let (
                            mut socket,
                            _,
                        ) =
                            listener
                                .accept()
                                .await
                                .expect(
                                    "accept fixture request"
                                );

                        counter.fetch_add(
                            1,
                            Ordering::SeqCst,
                        );

                        let mut request =
                            vec![0u8; 8192];

                        let read =
                            socket
                                .read(
                                    &mut request
                                )
                                .await
                                .expect(
                                    "read fixture request"
                                );

                        let request =
                            String::from_utf8_lossy(
                                &request[..read]
                            );

                        let body: &[u8] =
                            if request
                                .starts_with(
                                    "GET /manifest.sig "
                                )
                            {
                                &signature
                            } else if request
                                .starts_with(
                                    "GET /manifest "
                                )
                            {
                                &manifest
                            } else {
                                b"not found"
                            };

                        let status =
                            if request
                                .starts_with(
                                    "GET /manifest "
                                )
                                || request
                                    .starts_with(
                                        "GET /manifest.sig "
                                    )
                            {
                                "200 OK"
                            } else {
                                "404 Not Found"
                            };

                        let header =
                            format!(
                                concat!(
                                    "HTTP/1.1 {status}\r\n",
                                    "Content-Length: {length}\r\n",
                                    "Content-Type: application/octet-stream\r\n",
                                    "Connection: close\r\n",
                                    "\r\n"
                                ),
                                status = status,
                                length = body.len(),
                            );

                        socket
                            .write_all(
                                header.as_bytes()
                            )
                            .await
                            .expect(
                                "write fixture header"
                            );

                        socket
                            .write_all(body)
                            .await
                            .expect(
                                "write fixture body"
                            );

                        socket
                            .shutdown()
                            .await
                            .expect(
                                "close fixture socket"
                            );
                    }
                },
            );

        (
            format!(
                "http://{address}"
            ),
            requests,
            handle,
        )
    }

    #[test]
    fn published_signed_beta_fixture_is_update_available() {
        let outcome =
            evaluate_signed_manifest(
                UpdateChannel::Beta,
                "1.2.2",
                BETA_MANIFEST,
                BETA_SIGNATURE,
            )
            .expect(
                "published signed beta fixture"
            );

        match outcome {
            DiscoveryOutcome::UpdateAvailable {
                version,
                release_id,
                package_filename,
            } => {
                assert_eq!(
                    version,
                    "1.2.3"
                );

                assert_eq!(
                    release_id,
                    "1.2.3-login-redesign-4756c28b753f"
                );

                assert_eq!(
                    package_filename,
                    "PhotoOS-1.2.3-4756c28b753f.popkg"
                );
            }

            other => {
                panic!(
                    "expected UpdateAvailable, got {other:?}"
                );
            }
        }
    }

    #[test]
    fn invalid_signature_is_rejected_before_json_parse() {
        let invalid_json =
            b"{ definitely not valid json";

        let invalid_signature =
            [0u8; 64];

        let result =
            evaluate_signed_manifest(
                UpdateChannel::Beta,
                "1.2.2",
                invalid_json,
                &invalid_signature,
            );

        assert!(
            matches!(
                result,
                Err(
                    DiscoveryError::InvalidSignature
                )
            ),
            "signature verification must happen before JSON parsing: {result:?}"
        );
    }

    #[test]
    fn manifest_contract_rejects_invalid_identity_hash_size_and_urls() {
        let base: Value =
            serde_json::from_slice(
                BETA_MANIFEST
            )
            .expect(
                "published fixture JSON"
            );

        validate_manifest_value(
            &base,
            UpdateChannel::Beta,
        )
        .expect(
            "published fixture contract"
        );

        let mut wrong_product =
            base.clone();

        wrong_product["product"] =
            json!("NotPhotoOS");

        assert!(
            validate_manifest_value(
                &wrong_product,
                UpdateChannel::Beta,
            )
            .is_err()
        );

        let mut wrong_channel =
            base.clone();

        wrong_channel["channel"] =
            json!("stable");

        assert!(
            validate_manifest_value(
                &wrong_channel,
                UpdateChannel::Beta,
            )
            .is_err()
        );

        let mut wrong_hash =
            base.clone();

        wrong_hash["package"]["sha256"] =
            json!("ABC123");

        assert!(
            validate_manifest_value(
                &wrong_hash,
                UpdateChannel::Beta,
            )
            .is_err()
        );

        let mut zero_size =
            base.clone();

        zero_size["package"]["size"] =
            json!(0);

        assert!(
            validate_manifest_value(
                &zero_size,
                UpdateChannel::Beta,
            )
            .is_err()
        );

        let mut evil_download =
            base.clone();

        evil_download
            ["package"]
            ["download_url"] =
            json!(
                "https://evil.example/PhotoOS-1.2.3-4756c28b753f.popkg"
            );

        assert!(
            validate_manifest_value(
                &evil_download,
                UpdateChannel::Beta,
            )
            .is_err()
        );

        let mut evil_signature =
            base.clone();

        evil_signature
            ["package"]
            ["signature_url"] =
            json!(
                "https://evil.example/PhotoOS-1.2.3-4756c28b753f.popkg.sig"
            );

        assert!(
            validate_manifest_value(
                &evil_signature,
                UpdateChannel::Beta,
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn disabled_channel_short_circuits_before_network() {
        let store =
            memory_store().await;

        let service =
            UpdateDiscoveryService
                ::new_for_test(
                    "http://127.0.0.1:1/manifest",
                    "http://127.0.0.1:1/manifest.sig",
                )
                .expect(
                    "test discovery service"
                );

        let outcome =
            service
                .check_channel(
                    &store,
                    UpdateChannel::Disabled,
                    "1.2.2",
                )
                .await
                .expect(
                    "disabled discovery"
                );

        assert_eq!(
            outcome,
            DiscoveryOutcome::Disabled
        );

        store.close().await;
    }

    #[tokio::test]
    async fn local_http_fixture_exercises_signed_online_discovery() {
        let (
            base,
            requests,
            server,
        ) =
            spawn_fixture_server().await;

        let store =
            memory_store().await;

        let service =
            UpdateDiscoveryService
                ::new_for_test(
                    format!(
                        "{base}/manifest"
                    ),
                    format!(
                        "{base}/manifest.sig"
                    ),
                )
                .expect(
                    "local discovery service"
                );

        let outcome =
            service
                .check_channel(
                    &store,
                    UpdateChannel::Beta,
                    "1.2.2",
                )
                .await
                .expect(
                    "signed online discovery"
                );

        assert!(
            matches!(
                outcome,
                DiscoveryOutcome::UpdateAvailable {
                    ref version,
                    ..
                } if version == "1.2.3"
            )
        );

        assert_eq!(
            requests.load(
                Ordering::SeqCst
            ),
            2
        );

        server
            .await
            .expect(
                "fixture server task"
            );

        store.close().await;
    }

    #[tokio::test]
    async fn replay_regression_is_rejected_after_signature_validation() {
        let store =
            memory_store().await;

        let higher =
            ManifestReplayCandidate {
                channel:
                    UpdateChannel::Beta,

                version:
                    "1.2.4".to_string(),

                published_at:
                    "2026-09-23T00:00:00Z"
                        .to_string(),

                manifest_sha256:
                    "a".repeat(64),
            };

        assert_eq!(
            store
                .observe_manifest(
                    higher
                )
                .await
                .expect(
                    "seed higher replay state"
                ),
            ReplayDecision::Accepted
        );

        let outcome =
            evaluate_signed_manifest_with_state(
                &store,
                UpdateChannel::Beta,
                "1.2.2",
                BETA_MANIFEST,
                BETA_SIGNATURE,
            )
            .await
            .expect(
                "signed replay evaluation"
            );

        assert_eq!(
            outcome,
            DiscoveryOutcome::ReplayRejected
        );

        store.close().await;
    }
}

#[cfg(test)]
mod task8a_agent_minimum_tests {
    use crate::update_state::UpdateChannel;

    use super::{
        DiscoveryOutcome,
        evaluate_signed_manifest_for_versions,
    };

    const BETA_MANIFEST: &[u8] =
        include_bytes!(
            "../test-fixtures/update-discovery-1.2.3/beta.json"
        );

    const BETA_SIGNATURE: &[u8] =
        include_bytes!(
            "../test-fixtures/update-discovery-1.2.3/beta.json.sig"
        );

    #[test]
    fn task8a_agent_below_signed_minimum_is_incompatible() {
        /*
         * The published 1.2.3 beta fixture requires
         * Update Agent >= 1.0.0.
         *
         * PhotoOS 1.2.2 itself is compatible, so this
         * failure must be caused by Agent 0.9.0.
         */
        let outcome =
            evaluate_signed_manifest_for_versions(
                UpdateChannel::Beta,
                "1.2.2",
                "0.9.0",
                BETA_MANIFEST,
                BETA_SIGNATURE,
            )
            .expect(
                "valid signed manifest"
            );

        assert_eq!(
            outcome,
            DiscoveryOutcome::Incompatible
        );
    }
}
