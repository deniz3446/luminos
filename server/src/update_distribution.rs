use std::{
    error::Error,
    fmt,
};

use reqwest::Url;

use crate::update_state::UpdateChannel;

const STABLE_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/stable.json";

const STABLE_SIGNATURE_URL: &str =
    "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/stable.json.sig";

const BETA_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/beta.json";

const BETA_SIGNATURE_URL: &str =
    "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/beta.json.sig";

const RELEASE_HOST: &str =
    "github.com";

const RELEASE_ASSET_HOST: &str =
    "release-assets.githubusercontent.com";

const OWNER: &str =
    "deniz3446";

const REPOSITORY: &str =
    "PhotoOS-Updates";

pub const MAX_REDIRECTS: usize = 3;

pub const MAX_MANIFEST_BYTES: u64 =
    256 * 1024;

pub const MAX_SIGNATURE_BYTES: u64 =
    4 * 1024;

pub const MAX_PACKAGE_BYTES: u64 =
    512 * 1024 * 1024;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
pub enum DistributionPolicyError {
    InvalidUrl,
    InvalidScheme,
    InvalidHost,
    InvalidCredentials,
    InvalidPort,
    InvalidPath,
    InvalidFilename,
    QueryNotAllowed,
    FragmentNotAllowed,
    SizeOutOfRange,
}

impl fmt::Display for DistributionPolicyError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let message = match self {
            Self::InvalidUrl =>
                "invalid update distribution URL",

            Self::InvalidScheme =>
                "update distribution URL must use HTTPS",

            Self::InvalidHost =>
                "update distribution URL host is not allowed",

            Self::InvalidCredentials =>
                "credentials are not allowed in update distribution URLs",

            Self::InvalidPort =>
                "explicit non-default ports are not allowed",

            Self::InvalidPath =>
                "update distribution URL path is not allowed",

            Self::InvalidFilename =>
                "update distribution filename is not safe",

            Self::QueryNotAllowed =>
                "query parameters are not allowed for this update URL",

            Self::FragmentNotAllowed =>
                "URL fragments are not allowed for update distribution",

            Self::SizeOutOfRange =>
                "update distribution response size is outside the allowed range",
        };

        write!(f, "{message}")
    }
}

impl Error for DistributionPolicyError {}

pub fn channel_manifest_url(
    channel: UpdateChannel,
) -> Option<&'static str> {
    match channel {
        UpdateChannel::Stable =>
            Some(STABLE_MANIFEST_URL),

        UpdateChannel::Beta =>
            Some(BETA_MANIFEST_URL),

        UpdateChannel::Disabled =>
            None,
    }
}

pub fn channel_signature_url(
    channel: UpdateChannel,
) -> Option<&'static str> {
    match channel {
        UpdateChannel::Stable =>
            Some(STABLE_SIGNATURE_URL),

        UpdateChannel::Beta =>
            Some(BETA_SIGNATURE_URL),

        UpdateChannel::Disabled =>
            None,
    }
}

fn parse_https_url(
    raw: &str,
) -> Result<Url, DistributionPolicyError> {
    let url =
        Url::parse(raw)
            .map_err(
                |_| DistributionPolicyError::InvalidUrl
            )?;

    if url.scheme() != "https" {
        return Err(
            DistributionPolicyError::InvalidScheme
        );
    }

    if !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(
            DistributionPolicyError::InvalidCredentials
        );
    }

    if url.port().is_some() {
        return Err(
            DistributionPolicyError::InvalidPort
        );
    }

    if url.fragment().is_some() {
        return Err(
            DistributionPolicyError::FragmentNotAllowed
        );
    }

    Ok(url)
}

fn validate_leaf_filename(
    filename: &str,
) -> Result<(), DistributionPolicyError> {
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains('?')
        || filename.contains('#')
        || filename.contains('%')
    {
        return Err(
            DistributionPolicyError::InvalidFilename
        );
    }

    if !filename.ends_with(".popkg")
        && !filename.ends_with(".popkg.sig")
    {
        return Err(
            DistributionPolicyError::InvalidFilename
        );
    }

    Ok(())
}

fn has_release_path_escape(
    raw: &str,
) -> bool {
    let lower = raw.to_ascii_lowercase();

    lower.contains("/../")
        || lower.ends_with("/..")
        || lower.contains("%2e")
        || lower.contains("%2f")
        || lower.contains("%5c")
}

fn official_release_path(
    url: &Url,
) -> bool {
    let Some(segments) =
        url.path_segments()
    else {
        return false;
    };

    let segments:
        Vec<&str> = segments.collect();

    segments.len() == 6
        && segments[0] == OWNER
        && segments[1] == REPOSITORY
        && segments[2] == "releases"
        && segments[3] == "download"
        && !segments[4].is_empty()
        && segments[4] != "."
        && segments[4] != ".."
        && !segments[5].is_empty()
}

pub fn validate_release_asset_url(
    raw: &str,
    expected_filename: &str,
) -> Result<(), DistributionPolicyError> {
    validate_leaf_filename(
        expected_filename,
    )?;

    if has_release_path_escape(raw) {
        return Err(
            DistributionPolicyError::InvalidPath
        );
    }

    let url = parse_https_url(raw)?;

    if url.host_str() != Some(RELEASE_HOST) {
        return Err(
            DistributionPolicyError::InvalidHost
        );
    }

    if url.query().is_some() {
        return Err(
            DistributionPolicyError::QueryNotAllowed
        );
    }

    if !official_release_path(&url) {
        return Err(
            DistributionPolicyError::InvalidPath
        );
    }

    let actual_filename =
        url.path_segments()
            .and_then(|mut segments| {
                segments.next_back()
            })
            .ok_or(
                DistributionPolicyError::InvalidPath
            )?;

    if actual_filename != expected_filename {
        return Err(
            DistributionPolicyError::InvalidFilename
        );
    }

    Ok(())
}

pub fn validate_redirect_target(
    raw: &str,
) -> Result<(), DistributionPolicyError> {
    let url = parse_https_url(raw)?;

    match url.host_str() {
        Some(RELEASE_HOST) => {
            if has_release_path_escape(raw) {
                return Err(
                    DistributionPolicyError::InvalidPath
                );
            }

            if url.query().is_some() {
                return Err(
                    DistributionPolicyError::QueryNotAllowed
                );
            }

            if !official_release_path(&url) {
                return Err(
                    DistributionPolicyError::InvalidPath
                );
            }

            Ok(())
        }

        Some(RELEASE_ASSET_HOST) => {
            if !url
                .path()
                .starts_with(
                    "/github-production-release-asset/"
                )
            {
                return Err(
                    DistributionPolicyError::InvalidPath
                );
            }

            Ok(())
        }

        _ => Err(
            DistributionPolicyError::InvalidHost
        ),
    }
}

pub fn can_follow_redirect(
    redirects_already_followed: usize,
) -> bool {
    redirects_already_followed
        < MAX_REDIRECTS
}

fn validate_positive_bounded_size(
    size: u64,
    maximum: u64,
) -> Result<(), DistributionPolicyError> {
    if size == 0 || size > maximum {
        return Err(
            DistributionPolicyError::SizeOutOfRange
        );
    }

    Ok(())
}

pub fn validate_manifest_response_size(
    size: u64,
) -> Result<(), DistributionPolicyError> {
    validate_positive_bounded_size(
        size,
        MAX_MANIFEST_BYTES,
    )
}

pub fn validate_signature_response_size(
    size: u64,
) -> Result<(), DistributionPolicyError> {
    validate_positive_bounded_size(
        size,
        MAX_SIGNATURE_BYTES,
    )
}

pub fn validate_package_size(
    size: u64,
) -> Result<(), DistributionPolicyError> {
    validate_positive_bounded_size(
        size,
        MAX_PACKAGE_BYTES,
    )
}


#[cfg(test)]
mod tests {
    use crate::update_state::UpdateChannel;

    use super::{
        can_follow_redirect,
        channel_manifest_url,
        channel_signature_url,
        validate_manifest_response_size,
        validate_package_size,
        validate_release_asset_url,
        validate_redirect_target,
        validate_signature_response_size,
        MAX_MANIFEST_BYTES,
        MAX_PACKAGE_BYTES,
        MAX_REDIRECTS,
        MAX_SIGNATURE_BYTES,
    };

    #[test]
    fn channel_urls_are_fixed_and_disabled_has_no_remote_url() {
        assert_eq!(
            channel_manifest_url(
                UpdateChannel::Stable
            ),
            Some(
                "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/stable.json"
            )
        );

        assert_eq!(
            channel_signature_url(
                UpdateChannel::Stable
            ),
            Some(
                "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/stable.json.sig"
            )
        );

        assert_eq!(
            channel_manifest_url(
                UpdateChannel::Beta
            ),
            Some(
                "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/beta.json"
            )
        );

        assert_eq!(
            channel_signature_url(
                UpdateChannel::Beta
            ),
            Some(
                "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/beta.json.sig"
            )
        );

        assert_eq!(
            channel_manifest_url(
                UpdateChannel::Disabled
            ),
            None
        );

        assert_eq!(
            channel_signature_url(
                UpdateChannel::Disabled
            ),
            None
        );
    }

    #[test]
    fn release_asset_url_requires_exact_https_repo_and_filename() {
        let filename =
            "PhotoOS-1.2.4-deadbeef.popkg";

        let valid = format!(
            "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}"
        );

        validate_release_asset_url(
            &valid,
            filename,
        )
        .expect(
            "official GitHub release asset must be accepted"
        );

        let signature =
            format!("{filename}.sig");

        let valid_signature = format!(
            "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{signature}"
        );

        validate_release_asset_url(
            &valid_signature,
            &signature,
        )
        .expect(
            "official detached signature URL must be accepted"
        );

        let invalid = [
            format!(
                "http://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}"
            ),
            format!(
                "https://evil.example/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}"
            ),
            format!(
                "https://github.com/attacker/PhotoOS-Updates/releases/download/v1.2.4/{filename}"
            ),
            format!(
                "https://github.com/deniz3446/OtherRepo/releases/download/v1.2.4/{filename}"
            ),
            format!(
                "https://attacker@github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}"
            ),
            format!(
                "https://github.com:444/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}?download=1"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}#fragment"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/{filename}.evil"
            ),
        ];

        for url in invalid {
            assert!(
                validate_release_asset_url(
                    &url,
                    filename,
                )
                .is_err(),
                "must reject {url}"
            );
        }
    }

    #[test]
    fn release_asset_url_rejects_path_traversal_and_encoded_separators() {
        let filename =
            "PhotoOS-1.2.4-deadbeef.popkg";

        let invalid = [
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/../v1.2.4/{filename}"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/%2e%2e/v1.2.4/{filename}"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/%2e%2e/{filename}"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4%2fextra/{filename}"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/%2F{filename}"
            ),
            format!(
                "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/%5c{filename}"
            ),
        ];

        for url in invalid {
            assert!(
                validate_release_asset_url(
                    &url,
                    filename,
                )
                .is_err(),
                "must reject traversal-like URL {url}"
            );
        }
    }

    #[test]
    fn redirect_policy_is_bounded_and_host_allowlisted() {
        assert_eq!(
            MAX_REDIRECTS,
            3
        );

        assert!(
            can_follow_redirect(0)
        );

        assert!(
            can_follow_redirect(1)
        );

        assert!(
            can_follow_redirect(2)
        );

        assert!(
            !can_follow_redirect(3)
        );

        validate_redirect_target(
            "https://github.com/deniz3446/PhotoOS-Updates/releases/download/v1.2.4/PhotoOS-1.2.4-deadbeef.popkg"
        )
        .expect(
            "official github redirect target"
        );

        validate_redirect_target(
            "https://release-assets.githubusercontent.com/github-production-release-asset/123456/example?sp=r&sv=1&sig=abc"
        )
        .expect(
            "GitHub release asset CDN redirect"
        );

        let invalid = [
            "http://release-assets.githubusercontent.com/github-production-release-asset/123456/example",
            "https://release-assets.githubusercontent.com.evil.example/example",
            "https://evil.example/example",
            "https://attacker@release-assets.githubusercontent.com/example",
            "https://release-assets.githubusercontent.com:444/example",
            "https://raw.githubusercontent.com/deniz3446/PhotoOS-Updates/main/channels/stable.json",
        ];

        for url in invalid {
            assert!(
                validate_redirect_target(url)
                    .is_err(),
                "must reject redirect target {url}"
            );
        }
    }

    #[test]
    fn manifest_and_signature_response_sizes_are_bounded() {
        assert!(
            MAX_MANIFEST_BYTES > 0
        );

        assert!(
            MAX_SIGNATURE_BYTES >= 64
        );

        validate_manifest_response_size(1)
            .expect("small manifest");

        validate_manifest_response_size(
            MAX_MANIFEST_BYTES
        )
        .expect("manifest at maximum");

        assert!(
            validate_manifest_response_size(
                MAX_MANIFEST_BYTES + 1
            )
            .is_err()
        );

        validate_signature_response_size(64)
            .expect("Ed25519 signature");

        validate_signature_response_size(
            MAX_SIGNATURE_BYTES
        )
        .expect("signature response at maximum");

        assert!(
            validate_signature_response_size(
                MAX_SIGNATURE_BYTES + 1
            )
            .is_err()
        );
    }

    #[test]
    fn package_size_must_be_positive_and_within_agent_limit() {
        assert_eq!(
            MAX_PACKAGE_BYTES,
            512 * 1024 * 1024
        );

        assert!(
            validate_package_size(0)
                .is_err()
        );

        validate_package_size(1)
            .expect("minimum positive size");

        validate_package_size(
            7_434_622
        )
        .expect(
            "published PhotoOS 1.2.3 package size"
        );

        validate_package_size(
            MAX_PACKAGE_BYTES
        )
        .expect("maximum package size");

        assert!(
            validate_package_size(
                MAX_PACKAGE_BYTES + 1
            )
            .is_err()
        );
    }
}
