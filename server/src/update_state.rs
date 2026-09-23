use std::{
    error::Error,
    fmt,
    path::Path,
};

use chrono::DateTime;
use sqlx::SqlitePool;

const UPDATE_STATE_SCHEMA: &str = include_str!(
    "../migrations/20260922200500_update_discovery_state.sql"
);

const VERIFIED_OPERATION_SCHEMA: &str = include_str!(
    "../migrations/20260922213000_update_verified_operation_identity.sql"
);

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
pub enum UpdateChannel {
    Stable,
    Beta,
    Disabled,
}

impl UpdateChannel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
            Self::Disabled => "disabled",
        }
    }

    fn from_db(
        value: &str,
    ) -> Result<Self, UpdateStateError> {
        match value {
            "stable" => Ok(Self::Stable),
            "beta" => Ok(Self::Beta),
            "disabled" => Ok(Self::Disabled),
            other => Err(
                UpdateStateError::InvalidChannel(
                    other.to_string(),
                ),
            ),
        }
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct ManifestReplayCandidate {
    pub channel: UpdateChannel,
    pub version: String,
    pub published_at: String,
    pub manifest_sha256: String,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
pub enum ReplayDecision {
    Accepted,
    AlreadySeen,
    RejectedRegression,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
pub enum UpdateOperationStatus {
    Downloading,
    Verifying,
    Verified,
    Installing,
    Succeeded,
    Failed,
}

impl UpdateOperationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Verified => "verified",
            Self::Installing => "installing",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn from_db(
        value: &str,
    ) -> Result<Self, UpdateStateError> {
        match value {
            "downloading" => Ok(Self::Downloading),
            "verifying" => Ok(Self::Verifying),
            "verified" => Ok(Self::Verified),
            "installing" => Ok(Self::Installing),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            other => Err(
                UpdateStateError::InvalidOperationStatus(
                    other.to_string(),
                ),
            ),
        }
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct UpdateOperationRecord {
    pub operation_id: String,
    pub target_version: String,
    pub status: UpdateOperationStatus,
    pub updated_at: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct VerifiedUpdateOperationRecord {
    pub operation_id: String,
    pub target_version: String,
    pub release_id: String,
    pub package_filename: String,
    pub package_sha256: String,
    pub package_size: u64,
    pub status: UpdateOperationStatus,
    pub updated_at: String,
}

#[derive(Debug)]
pub enum UpdateStateError {
    Sqlx(sqlx::Error),
    InvalidChannel(String),
    InvalidOperationStatus(String),
    InvalidVersion(String),
    InvalidTimestamp(String),
    InvalidDigest,
    InvalidOperationId,
    InvalidReleaseId,
    InvalidPackageFilename,
    InvalidPackageSize,
}

impl fmt::Display for UpdateStateError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Sqlx(error) => {
                write!(
                    f,
                    "update-state database error: {error}"
                )
            }

            Self::InvalidChannel(value) => {
                write!(
                    f,
                    "invalid update channel: {value}"
                )
            }

            Self::InvalidOperationStatus(value) => {
                write!(
                    f,
                    "invalid update operation status: {value}"
                )
            }

            Self::InvalidVersion(value) => {
                write!(
                    f,
                    "invalid PhotoOS semantic version: {value}"
                )
            }

            Self::InvalidTimestamp(value) => {
                write!(
                    f,
                    "invalid update timestamp: {value}"
                )
            }

            Self::InvalidDigest => {
                write!(
                    f,
                    "manifest SHA256 must be 64 lowercase hexadecimal characters"
                )
            }

            Self::InvalidOperationId => {
                write!(
                    f,
                    "update operation id must not be empty"
                )
            }

            Self::InvalidReleaseId => {
                write!(
                    f,
                    "invalid update release id"
                )
            }

            Self::InvalidPackageFilename => {
                write!(
                    f,
                    "invalid update package filename"
                )
            }

            Self::InvalidPackageSize => {
                write!(
                    f,
                    "invalid update package size"
                )
            }
}
    }
}

impl Error for UpdateStateError {
    fn source(
        &self,
    ) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlx(error) => Some(error),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for UpdateStateError {
    fn from(
        value: sqlx::Error,
    ) -> Self {
        Self::Sqlx(value)
    }
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
) -> Result<NumericVersion, UpdateStateError> {
    let parts: Vec<&str> =
        value.split('.').collect();

    if parts.len() != 3 {
        return Err(
            UpdateStateError::InvalidVersion(
                value.to_string(),
            ),
        );
    }

    fn part(
        raw: &str,
        whole: &str,
    ) -> Result<u64, UpdateStateError> {
        if raw.is_empty()
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_digit())
            || (
                raw.len() > 1
                && raw.starts_with('0')
            )
        {
            return Err(
                UpdateStateError::InvalidVersion(
                    whole.to_string(),
                ),
            );
        }

        raw.parse::<u64>().map_err(
            |_| {
                UpdateStateError::InvalidVersion(
                    whole.to_string(),
                )
            },
        )
    }

    Ok(
        NumericVersion {
            major: part(
                parts[0],
                value,
            )?,
            minor: part(
                parts[1],
                value,
            )?,
            patch: part(
                parts[2],
                value,
            )?,
        },
    )
}

fn parse_timestamp(
    value: &str,
) -> Result<
    DateTime<chrono::FixedOffset>,
    UpdateStateError,
> {
    DateTime::parse_from_rfc3339(value)
        .map_err(
            |_| {
                UpdateStateError::InvalidTimestamp(
                    value.to_string(),
                )
            },
        )
}

fn validate_digest(
    value: &str,
) -> Result<(), UpdateStateError> {
    if value.len() != 64
        || !value.bytes().all(
            |byte| {
                byte.is_ascii_digit()
                    || (b'a'..=b'f').contains(&byte)
            },
        )
    {
        return Err(
            UpdateStateError::InvalidDigest,
        );
    }

    Ok(())
}

fn validate_release_id(
    value: &str,
) -> Result<(), UpdateStateError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.contains('/')
        || value.contains('\\')
        || value.contains("..")
    {
        return Err(
            UpdateStateError::InvalidReleaseId
        );
    }

    Ok(())
}

fn validate_package_filename(
    value: &str,
) -> Result<(), UpdateStateError> {
    if value.trim().is_empty()
        || value != value.trim()
        || Path::new(value)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(value)
        || !value.ends_with(".popkg")
    {
        return Err(
            UpdateStateError::InvalidPackageFilename
        );
    }

    Ok(())
}

fn package_size_for_db(
    value: u64,
) -> Result<i64, UpdateStateError> {
    if value == 0 {
        return Err(
            UpdateStateError::InvalidPackageSize
        );
    }

    i64::try_from(value)
        .map_err(
            |_| UpdateStateError::InvalidPackageSize
        )
}

#[derive(Clone)]
pub struct UpdateStateStore {
    pool: SqlitePool,
}

impl UpdateStateStore {
    pub fn new(
        pool: SqlitePool,
    ) -> Self {
        Self { pool }
    }

    pub async fn ensure_schema(
        &self,
    ) -> Result<(), UpdateStateError> {
        sqlx::raw_sql(
            UPDATE_STATE_SCHEMA,
        )
        .execute(&self.pool)
        .await?;

        sqlx::raw_sql(
            VERIFIED_OPERATION_SCHEMA,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_channel(
        &self,
    ) -> Result<
        UpdateChannel,
        UpdateStateError,
    > {
        let channel =
            sqlx::query_scalar::<_, String>(
                r#"
                SELECT channel
                FROM photoos_update_preferences
                WHERE singleton = 1
                "#,
            )
            .fetch_optional(&self.pool)
            .await?;

        match channel {
            Some(value) => {
                UpdateChannel::from_db(
                    &value,
                )
            }

            None => Ok(
                UpdateChannel::Stable,
            ),
        }
    }

    pub async fn set_channel(
        &self,
        channel: UpdateChannel,
    ) -> Result<(), UpdateStateError> {
        sqlx::query(
            r#"
            INSERT INTO photoos_update_preferences (
                singleton,
                channel,
                updated_at
            )
            VALUES (
                1,
                ?1,
                CURRENT_TIMESTAMP
            )
            ON CONFLICT(singleton)
            DO UPDATE SET
                channel = excluded.channel,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(
            channel.as_str(),
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn observe_manifest(
        &self,
        candidate: ManifestReplayCandidate,
    ) -> Result<
        ReplayDecision,
        UpdateStateError,
    > {
        if candidate.channel
            == UpdateChannel::Disabled
        {
            return Err(
                UpdateStateError::InvalidChannel(
                    "disabled".to_string(),
                ),
            );
        }

        let candidate_version =
            parse_version(
                &candidate.version,
            )?;

        let candidate_published =
            parse_timestamp(
                &candidate.published_at,
            )?;

        validate_digest(
            &candidate.manifest_sha256,
        )?;

        let previous =
            sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                ),
            >(
                r#"
                SELECT
                    version,
                    published_at,
                    manifest_sha256
                FROM photoos_update_manifest_replay
                WHERE channel = ?1
                "#,
            )
            .bind(
                candidate.channel.as_str(),
            )
            .fetch_optional(&self.pool)
            .await?;

        if let Some(
            (
                old_version,
                old_published_at,
                old_digest,
            ),
        ) = previous
        {
            if old_digest
                == candidate.manifest_sha256
            {
                return Ok(
                    ReplayDecision::AlreadySeen,
                );
            }

            let old_version =
                parse_version(
                    &old_version,
                )?;

            let old_published =
                parse_timestamp(
                    &old_published_at,
                )?;

            if candidate_version
                <= old_version
                || candidate_published
                    < old_published
            {
                return Ok(
                    ReplayDecision::RejectedRegression,
                );
            }
        }

        sqlx::query(
            r#"
            INSERT INTO photoos_update_manifest_replay (
                channel,
                version,
                published_at,
                manifest_sha256,
                observed_at
            )
            VALUES (
                ?1,
                ?2,
                ?3,
                ?4,
                CURRENT_TIMESTAMP
            )
            ON CONFLICT(channel)
            DO UPDATE SET
                version = excluded.version,
                published_at = excluded.published_at,
                manifest_sha256 = excluded.manifest_sha256,
                observed_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(
            candidate.channel.as_str(),
        )
        .bind(
            &candidate.version,
        )
        .bind(
            &candidate.published_at,
        )
        .bind(
            &candidate.manifest_sha256,
        )
        .execute(&self.pool)
        .await?;

        Ok(
            ReplayDecision::Accepted,
        )
    }

    pub async fn save_operation(
        &self,
        operation: UpdateOperationRecord,
    ) -> Result<(), UpdateStateError> {
        if operation.operation_id.trim().is_empty() {
            return Err(
                UpdateStateError::InvalidOperationId,
            );
        }

        parse_version(
            &operation.target_version,
        )?;

        parse_timestamp(
            &operation.updated_at,
        )?;

        sqlx::query(
            r#"
            INSERT INTO photoos_update_operations (
                operation_id,
                target_version,
                status,
                updated_at
            )
            VALUES (
                ?1,
                ?2,
                ?3,
                ?4
            )
            ON CONFLICT(operation_id)
            DO UPDATE SET
                target_version = excluded.target_version,
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(
            &operation.operation_id,
        )
        .bind(
            &operation.target_version,
        )
        .bind(
            operation.status.as_str(),
        )
        .bind(
            &operation.updated_at,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_operation(
        &self,
        operation_id: &str,
    ) -> Result<
        Option<UpdateOperationRecord>,
        UpdateStateError,
    > {
        let row =
            sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                ),
            >(
                r#"
                SELECT
                    target_version,
                    status,
                    updated_at
                FROM photoos_update_operations
                WHERE operation_id = ?1
                "#,
            )
            .bind(operation_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(
                (
                    target_version,
                    status,
                    updated_at,
                ),
            ) => {
                Ok(
                    Some(
                        UpdateOperationRecord {
                            operation_id:
                                operation_id
                                    .to_string(),
                            target_version,
                            status:
                                UpdateOperationStatus
                                    ::from_db(
                                        &status,
                                    )?,
                            updated_at,
                        },
                    ),
                )
            }

            None => Ok(None),
        }
    }

    pub async fn save_verified_operation(
        &self,
        operation:
            VerifiedUpdateOperationRecord,
    ) -> Result<(), UpdateStateError> {
        if operation.operation_id.trim().is_empty() {
            return Err(
                UpdateStateError::InvalidOperationId
            );
        }

        if operation.status
            != UpdateOperationStatus::Verified
        {
            return Err(
                UpdateStateError::InvalidOperationStatus(
                    operation.status
                        .as_str()
                        .to_string(),
                )
            );
        }

        parse_version(
            &operation.target_version
        )?;

        parse_timestamp(
            &operation.updated_at
        )?;

        validate_release_id(
            &operation.release_id
        )?;

        validate_package_filename(
            &operation.package_filename
        )?;

        validate_digest(
            &operation.package_sha256
        )?;

        let package_size =
            package_size_for_db(
                operation.package_size
            )?;

        let mut transaction =
            self.pool.begin().await?;

        /*
         * Plain INSERT is intentional:
         * a verified operation id is single-use and
         * its identity cannot be rewritten.
         */
        sqlx::query(
            r#"
            INSERT INTO photoos_update_operations (
                operation_id,
                target_version,
                status,
                updated_at
            )
            VALUES (
                ?1,
                ?2,
                ?3,
                ?4
            )
            "#,
        )
        .bind(&operation.operation_id)
        .bind(&operation.target_version)
        .bind(operation.status.as_str())
        .bind(&operation.updated_at)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO photoos_update_verified_operations (
                operation_id,
                release_id,
                package_filename,
                package_sha256,
                package_size
            )
            VALUES (
                ?1,
                ?2,
                ?3,
                ?4,
                ?5
            )
            "#,
        )
        .bind(&operation.operation_id)
        .bind(&operation.release_id)
        .bind(&operation.package_filename)
        .bind(&operation.package_sha256)
        .bind(package_size)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;

        Ok(())
    }

    pub async fn get_verified_operation(
        &self,
        operation_id: &str,
    ) -> Result<
        Option<VerifiedUpdateOperationRecord>,
        UpdateStateError,
    > {
        if operation_id.trim().is_empty() {
            return Err(
                UpdateStateError::InvalidOperationId
            );
        }

        let row =
            sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    i64,
                    String,
                    String,
                ),
            >(
                r#"
                SELECT
                    o.target_version,
                    v.release_id,
                    v.package_filename,
                    v.package_sha256,
                    v.package_size,
                    o.status,
                    o.updated_at
                FROM photoos_update_verified_operations v
                INNER JOIN photoos_update_operations o
                    ON o.operation_id = v.operation_id
                WHERE v.operation_id = ?1
                  AND o.status = 'verified'
                "#,
            )
            .bind(operation_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(
            (
                target_version,
                release_id,
                package_filename,
                package_sha256,
                package_size,
                status,
                updated_at,
            )
        ) = row
        else {
            return Ok(None);
        };

        parse_version(
            &target_version
        )?;

        validate_release_id(
            &release_id
        )?;

        validate_package_filename(
            &package_filename
        )?;

        validate_digest(
            &package_sha256
        )?;

        parse_timestamp(
            &updated_at
        )?;

        let package_size =
            u64::try_from(package_size)
                .map_err(
                    |_| UpdateStateError
                        ::InvalidPackageSize
                )?;

        if package_size == 0 {
            return Err(
                UpdateStateError::InvalidPackageSize
            );
        }

        Ok(
            Some(
                VerifiedUpdateOperationRecord {
                    operation_id:
                        operation_id.to_string(),

                    target_version,
                    release_id,
                    package_filename,
                    package_sha256,
                    package_size,

                    status:
                        UpdateOperationStatus
                            ::from_db(&status)?,

                    updated_at,
                }
            )
        )
    }

    pub async fn claim_verified_operation_for_install(
        &self,
        operation_id: &str,
        updated_at: &str,
    ) -> Result<
        Option<VerifiedUpdateOperationRecord>,
        UpdateStateError,
    > {
        if operation_id.trim().is_empty() {
            return Err(
                UpdateStateError::InvalidOperationId
            );
        }

        parse_timestamp(
            updated_at
        )?;

        let mut transaction =
            self.pool
                .begin()
                .await?;

        /*
         * Atomic single-use install authority.
         *
         * Only one caller can change the row from
         * verified -> installing. Any later/replayed
         * install request observes rows_affected == 0.
         */
        let result =
            sqlx::query(
                r#"
                UPDATE photoos_update_operations
                SET
                    status = 'installing',
                    updated_at = ?1
                WHERE operation_id = ?2
                  AND status = 'verified'
                "#,
            )
            .bind(
                updated_at
            )
            .bind(
                operation_id
            )
            .execute(
                &mut *transaction
            )
            .await?;

        if result.rows_affected() != 1 {
            transaction
                .rollback()
                .await?;

            return Ok(None);
        }

        /*
         * Read the immutable verified package identity
         * from the same transaction after the claim.
         */
        let row =
            sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    i64,
                    String,
                    String,
                ),
            >(
                r#"
                SELECT
                    o.target_version,
                    v.release_id,
                    v.package_filename,
                    v.package_sha256,
                    v.package_size,
                    o.status,
                    o.updated_at
                FROM photoos_update_verified_operations v
                INNER JOIN photoos_update_operations o
                    ON o.operation_id = v.operation_id
                WHERE v.operation_id = ?1
                  AND o.status = 'installing'
                "#,
            )
            .bind(
                operation_id
            )
            .fetch_optional(
                &mut *transaction
            )
            .await?;

        let Some(
            (
                target_version,
                release_id,
                package_filename,
                package_sha256,
                package_size,
                status,
                stored_updated_at,
            )
        ) = row
        else {
            transaction
                .rollback()
                .await?;

            return Ok(None);
        };

        parse_version(
            &target_version
        )?;

        validate_release_id(
            &release_id
        )?;

        validate_package_filename(
            &package_filename
        )?;

        validate_digest(
            &package_sha256
        )?;

        parse_timestamp(
            &stored_updated_at
        )?;

        let package_size =
            u64::try_from(
                package_size
            )
            .map_err(
                |_| {
                    UpdateStateError
                        ::InvalidPackageSize
                }
            )?;

        if package_size == 0 {
            return Err(
                UpdateStateError
                    ::InvalidPackageSize
            );
        }

        let status =
            UpdateOperationStatus
                ::from_db(
                    &status
                )?;

        if status
            != UpdateOperationStatus
                ::Installing
        {
            return Err(
                UpdateStateError
                    ::InvalidOperationStatus(
                        status
                            .as_str()
                            .to_string(),
                    )
            );
        }

        transaction
            .commit()
            .await?;

        Ok(
            Some(
                VerifiedUpdateOperationRecord {
                    operation_id:
                        operation_id
                            .to_string(),

                    target_version,

                    release_id,

                    package_filename,

                    package_sha256,

                    package_size,

                    status,

                    updated_at:
                        stored_updated_at,
                }
            )
        )
    }

    pub async fn close(
        &self,
    ) {
        self.pool.close().await;
    }
}


#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        str::FromStr,
    };

    use sqlx::{
        sqlite::SqliteConnectOptions,
        SqlitePool,
    };
    use uuid::Uuid;

    use super::{
        ManifestReplayCandidate,
        ReplayDecision,
        UpdateChannel,
        UpdateOperationRecord,
        UpdateOperationStatus,
        UpdateStateStore,
    };

    async fn memory_store() -> UpdateStateStore {
        let pool = SqlitePool::connect(
            "sqlite::memory:",
        )
        .await
        .expect("memory sqlite");

        let store = UpdateStateStore::new(pool);

        store
            .ensure_schema()
            .await
            .expect("update-state schema");

        store
    }

    async fn open_file_store(
        path: &PathBuf,
    ) -> UpdateStateStore {
        let url = format!(
            "sqlite://{}",
            path.display()
        );

        let options =
            SqliteConnectOptions::from_str(&url)
                .expect("sqlite options")
                .create_if_missing(true);

        let pool =
            SqlitePool::connect_with(options)
                .await
                .expect("file sqlite");

        let store = UpdateStateStore::new(pool);

        store
            .ensure_schema()
            .await
            .expect("update-state schema");

        store
    }

    fn candidate(
        channel: UpdateChannel,
        version: &str,
        published_at: &str,
        digest_byte: char,
    ) -> ManifestReplayCandidate {
        ManifestReplayCandidate {
            channel,
            version: version.to_string(),
            published_at: published_at.to_string(),
            manifest_sha256:
                digest_byte
                    .to_string()
                    .repeat(64),
        }
    }

    #[tokio::test]
    async fn channel_defaults_to_stable_and_persists() {
        let store = memory_store().await;

        assert_eq!(
            store
                .get_channel()
                .await
                .expect("default channel"),
            UpdateChannel::Stable
        );

        store
            .set_channel(UpdateChannel::Beta)
            .await
            .expect("set beta");

        assert_eq!(
            store
                .get_channel()
                .await
                .expect("persisted channel"),
            UpdateChannel::Beta
        );
    }

    #[tokio::test]
    async fn replay_state_is_scoped_per_channel() {
        let store = memory_store().await;

        let stable =
            candidate(
                UpdateChannel::Stable,
                "1.2.4",
                "2026-09-22T18:00:00Z",
                'a',
            );

        let beta =
            candidate(
                UpdateChannel::Beta,
                "1.2.3",
                "2026-09-22T17:00:00Z",
                'b',
            );

        assert_eq!(
            store
                .observe_manifest(stable)
                .await
                .expect("stable observation"),
            ReplayDecision::Accepted
        );

        assert_eq!(
            store
                .observe_manifest(beta)
                .await
                .expect("beta observation"),
            ReplayDecision::Accepted
        );
    }

    #[tokio::test]
    async fn replay_regressions_are_rejected() {
        let store = memory_store().await;

        let accepted =
            candidate(
                UpdateChannel::Stable,
                "1.2.4",
                "2026-09-22T19:00:00Z",
                'c',
            );

        assert_eq!(
            store
                .observe_manifest(accepted)
                .await
                .expect("initial observation"),
            ReplayDecision::Accepted
        );

        let older_version =
            candidate(
                UpdateChannel::Stable,
                "1.2.3",
                "2026-09-22T20:00:00Z",
                'd',
            );

        assert_eq!(
            store
                .observe_manifest(older_version)
                .await
                .expect("older version"),
            ReplayDecision::RejectedRegression
        );

        let older_publication =
            candidate(
                UpdateChannel::Stable,
                "1.2.4",
                "2026-09-22T18:59:59Z",
                'e',
            );

        assert_eq!(
            store
                .observe_manifest(older_publication)
                .await
                .expect("older publication"),
            ReplayDecision::RejectedRegression
        );
    }

    #[tokio::test]
    async fn same_manifest_digest_is_idempotent() {
        let store = memory_store().await;

        let first =
            candidate(
                UpdateChannel::Stable,
                "1.2.4",
                "2026-09-22T19:30:00Z",
                'f',
            );

        assert_eq!(
            store
                .observe_manifest(first.clone())
                .await
                .expect("first observation"),
            ReplayDecision::Accepted
        );

        assert_eq!(
            store
                .observe_manifest(first)
                .await
                .expect("repeat observation"),
            ReplayDecision::AlreadySeen
        );
    }

    #[tokio::test]
    async fn operation_state_survives_store_reopen() {
        let path =
            std::env::temp_dir().join(
                format!(
                    "photoos-update-state-{}.db",
                    Uuid::new_v4()
                )
            );

        let operation =
            UpdateOperationRecord {
                operation_id:
                    "task3-op-001".to_string(),
                target_version:
                    "1.2.4".to_string(),
                status:
                    UpdateOperationStatus::Downloading,
                updated_at:
                    "2026-09-22T19:45:00Z"
                        .to_string(),
            };

        {
            let store =
                open_file_store(&path).await;

            store
                .save_operation(
                    operation.clone()
                )
                .await
                .expect("save operation");

            store.close().await;
        }

        {
            let reopened =
                open_file_store(&path).await;

            let loaded =
                reopened
                    .get_operation(
                        "task3-op-001"
                    )
                    .await
                    .expect("load operation")
                    .expect(
                        "operation must persist"
                    );

            assert_eq!(
                loaded,
                operation
            );

            reopened.close().await;
        }

        let _ =
            std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod task8d1_verified_operation_identity_tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::{
        UpdateOperationRecord,
        UpdateOperationStatus,
        UpdateStateStore,
        VerifiedUpdateOperationRecord,
    };

    async fn test_store(
    ) -> UpdateStateStore {
        let pool =
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect(
                    "sqlite::memory:"
                )
                .await
                .expect(
                    "create task8d1 database"
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

    fn verified_record(
    ) -> VerifiedUpdateOperationRecord {
        VerifiedUpdateOperationRecord {
            operation_id:
                "task8d1-operation"
                    .to_string(),

            target_version:
                "1.2.4".to_string(),

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
                UpdateOperationStatus::Verified,

            updated_at:
                "2026-09-22T20:00:00+00:00"
                    .to_string(),
        }
    }

    #[tokio::test]
    async fn task8d1_verified_operation_round_trips_exact_package_identity() {
        let store =
            test_store().await;

        let expected =
            verified_record();

        store
            .save_verified_operation(
                expected.clone()
            )
            .await
            .expect(
                "save verified operation"
            );

        let actual =
            store
                .get_verified_operation(
                    &expected.operation_id
                )
                .await
                .expect(
                    "read verified operation"
                )
                .expect(
                    "verified operation exists"
                );

        assert_eq!(
            actual,
            expected
        );

        store.close().await;
    }

    #[tokio::test]
    async fn task8d1_plain_operation_cannot_be_promoted_to_verified_identity() {
        let store =
            test_store().await;

        store
            .save_operation(
                UpdateOperationRecord {
                    operation_id:
                        "legacy-operation"
                            .to_string(),

                    target_version:
                        "1.2.4"
                            .to_string(),

                    status:
                        UpdateOperationStatus
                            ::Verified,

                    updated_at:
                        "2026-09-22T20:00:00+00:00"
                            .to_string(),
                }
            )
            .await
            .expect(
                "save legacy operation"
            );

        /*
         * Status=verified alone is not enough.
         * Install must require a bound package
         * identity created by the 8C verification
         * path.
         */
        assert!(
            store
                .get_verified_operation(
                    "legacy-operation"
                )
                .await
                .expect(
                    "query verified identity"
                )
                .is_none()
        );

        store.close().await;
    }

    #[tokio::test]
    async fn task8d1_invalid_package_identity_is_rejected() {
        let store =
            test_store().await;

        let mut invalid_hash =
            verified_record();

        invalid_hash.operation_id =
            "invalid-hash"
                .to_string();

        invalid_hash.package_sha256 =
            "not-a-sha256"
                .to_string();

        assert!(
            store
                .save_verified_operation(
                    invalid_hash
                )
                .await
                .is_err()
        );

        let mut invalid_size =
            verified_record();

        invalid_size.operation_id =
            "invalid-size"
                .to_string();

        invalid_size.package_size =
            0;

        assert!(
            store
                .save_verified_operation(
                    invalid_size
                )
                .await
                .is_err()
        );

        let mut invalid_filename =
            verified_record();

        invalid_filename.operation_id =
            "invalid-filename"
                .to_string();

        invalid_filename
            .package_filename =
            "../evil.popkg"
                .to_string();

        assert!(
            store
                .save_verified_operation(
                    invalid_filename
                )
                .await
                .is_err()
        );

        store.close().await;
    }
}

#[cfg(test)]
mod task8d2_install_claim_state_tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::{
        UpdateOperationRecord,
        UpdateOperationStatus,
        UpdateStateStore,
        VerifiedUpdateOperationRecord,
    };

    async fn test_store(
    ) -> UpdateStateStore {
        let pool =
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect(
                    "sqlite::memory:"
                )
                .await
                .expect(
                    "create task8d2 database"
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

    fn verified_record(
        operation_id: &str,
    ) -> VerifiedUpdateOperationRecord {
        VerifiedUpdateOperationRecord {
            operation_id:
                operation_id
                    .to_string(),

            target_version:
                "1.2.4".to_string(),

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
                UpdateOperationStatus::Verified,

            updated_at:
                "2026-09-22T20:00:00+00:00"
                    .to_string(),
        }
    }

    #[tokio::test]
    async fn task8d2_verified_operation_can_be_claimed_only_once_for_install() {
        let store =
            test_store().await;

        let original =
            verified_record(
                "task8d2-single-use"
            );

        store
            .save_verified_operation(
                original.clone()
            )
            .await
            .expect(
                "save verified operation"
            );

        let claimed =
            store
                .claim_verified_operation_for_install(
                    &original.operation_id,
                    "2026-09-22T20:01:00+00:00",
                )
                .await
                .expect(
                    "claim verified operation"
                )
                .expect(
                    "first claim succeeds"
                );

        assert_eq!(
            claimed.operation_id,
            original.operation_id
        );

        assert_eq!(
            claimed.target_version,
            original.target_version
        );

        assert_eq!(
            claimed.release_id,
            original.release_id
        );

        assert_eq!(
            claimed.package_filename,
            original.package_filename
        );

        assert_eq!(
            claimed.package_sha256,
            original.package_sha256
        );

        assert_eq!(
            claimed.package_size,
            original.package_size
        );

        assert_eq!(
            claimed.status,
            UpdateOperationStatus::Installing
        );

        /*
         * Once claimed, it must no longer be returned
         * by the verified-only lookup.
         */
        assert!(
            store
                .get_verified_operation(
                    &original.operation_id
                )
                .await
                .expect(
                    "read verified identity"
                )
                .is_none()
        );

        let base =
            store
                .get_operation(
                    &original.operation_id
                )
                .await
                .expect(
                    "read operation"
                )
                .expect(
                    "operation exists"
                );

        assert_eq!(
            base.status,
            UpdateOperationStatus::Installing
        );

        /*
         * Second install attempt must lose the atomic
         * verified -> installing claim race.
         */
        assert!(
            store
                .claim_verified_operation_for_install(
                    &original.operation_id,
                    "2026-09-22T20:02:00+00:00",
                )
                .await
                .expect(
                    "second claim query"
                )
                .is_none()
        );

        store.close().await;
    }

    #[tokio::test]
    async fn task8d2_plain_or_nonverified_operation_cannot_be_claimed() {
        let store =
            test_store().await;

        store
            .save_operation(
                UpdateOperationRecord {
                    operation_id:
                        "task8d2-not-verified"
                            .to_string(),

                    target_version:
                        "1.2.4"
                            .to_string(),

                    status:
                        UpdateOperationStatus
                            ::Downloading,

                    updated_at:
                        "2026-09-22T20:00:00+00:00"
                            .to_string(),
                }
            )
            .await
            .expect(
                "save nonverified operation"
            );

        assert!(
            store
                .claim_verified_operation_for_install(
                    "task8d2-not-verified",
                    "2026-09-22T20:01:00+00:00",
                )
                .await
                .expect(
                    "claim query"
                )
                .is_none()
        );

        store.close().await;
    }
}
