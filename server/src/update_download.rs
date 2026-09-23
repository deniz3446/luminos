use std::{
    error::Error,
    fmt,
    path::{
        Path,
        PathBuf,
    },
    time::Duration,
};

use reqwest::{
    Client,
    Response,
    header::LOCATION,
    redirect::Policy,
};

use sha2::{
    Digest,
    Sha256,
};

use tokio::{
    fs::{
        self,
        OpenOptions,
    },
    io::AsyncWriteExt,
};

use crate::{
    update_distribution::{
        MAX_PACKAGE_BYTES,
        MAX_REDIRECTS,
        MAX_SIGNATURE_BYTES,
        can_follow_redirect,
        validate_package_size,
        validate_redirect_target,
        validate_release_asset_url,
    },
    update_trust::verify_detached_signature,
};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct PackageDownloadRequest {
    pub download_url: String,
    pub signature_url: String,
    pub filename: String,
    pub expected_size: u64,
    pub expected_sha256: String,
    pub staging_directory: PathBuf,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct StagedPackage {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub enum DownloadError {
    InvalidRequest,
    Http(reqwest::Error),
    HttpStatus(u16),
    InvalidRedirect,
    TooManyRedirects,
    Io(std::io::Error),
    SizeMismatch,
    HashMismatch,
    InvalidSignature,
    SignatureTooLarge,
}

impl fmt::Display for DownloadError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::InvalidRequest =>
                write!(
                    f,
                    "invalid package download request"
                ),

            Self::Http(error) =>
                write!(
                    f,
                    "package download HTTP error: {error}"
                ),

            Self::HttpStatus(status) =>
                write!(
                    f,
                    "package download returned HTTP status {status}"
                ),

            Self::InvalidRedirect =>
                write!(
                    f,
                    "package download redirect is not allowed"
                ),

            Self::TooManyRedirects =>
                write!(
                    f,
                    "package download exceeded redirect limit"
                ),

            Self::Io(error) =>
                write!(
                    f,
                    "package staging I/O error: {error}"
                ),

            Self::SizeMismatch =>
                write!(
                    f,
                    "downloaded package size does not match signed manifest"
                ),

            Self::HashMismatch =>
                write!(
                    f,
                    "downloaded package SHA256 does not match signed manifest"
                ),

            Self::InvalidSignature =>
                write!(
                    f,
                    "downloaded package signature is invalid"
                ),

            Self::SignatureTooLarge =>
                write!(
                    f,
                    "package signature response exceeds allowed size"
                ),
        }
    }
}

impl Error for DownloadError {
    fn source(
        &self,
    ) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http(error) =>
                Some(error),

            Self::Io(error) =>
                Some(error),

            _ => None,
        }
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(
        value: reqwest::Error,
    ) -> Self {
        Self::Http(value)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(
        value: std::io::Error,
    ) -> Self {
        Self::Io(value)
    }
}

pub struct PackageDownloadService {
    client: Client,

    #[cfg(test)]
    allow_test_http: bool,
}

impl PackageDownloadService {
    pub fn new(
    ) -> Result<Self, DownloadError> {
        let client =
            Client::builder()
                .redirect(
                    Policy::none()
                )
                .timeout(
                    Duration::from_secs(600)
                )
                .build()?;

        Ok(
            Self {
                client,

                #[cfg(test)]
                allow_test_http: false,
            }
        )
    }

    #[cfg(test)]
    pub fn new_for_test(
    ) -> Result<Self, DownloadError> {
        let client =
            Client::builder()
                .redirect(
                    Policy::none()
                )
                .timeout(
                    Duration::from_secs(30)
                )
                .build()?;

        Ok(
            Self {
                client,
                allow_test_http: true,
            }
        )
    }

    fn is_test_transport(
        &self,
    ) -> bool {
        #[cfg(test)]
        {
            self.allow_test_http
        }

        #[cfg(not(test))]
        {
            false
        }
    }

    fn validate_filename(
        filename: &str,
    ) -> Result<(), DownloadError> {
        if filename.is_empty()
            || filename == "."
            || filename == ".."
            || filename.contains('/')
            || filename.contains('\\')
            || filename.contains('%')
            || filename.contains('?')
            || filename.contains('#')
            || !filename.ends_with(".popkg")
        {
            return Err(
                DownloadError::InvalidRequest
            );
        }

        Ok(())
    }

    fn valid_sha256(
        digest: &str,
    ) -> bool {
        digest.len() == 64
            && digest
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

    fn validate_request(
        &self,
        request: &PackageDownloadRequest,
    ) -> Result<(), DownloadError> {
        Self::validate_filename(
            &request.filename
        )?;

        if !Self::valid_sha256(
            &request.expected_sha256
        ) {
            return Err(
                DownloadError::InvalidRequest
            );
        }

        validate_package_size(
            request.expected_size
        )
        .map_err(
            |_| DownloadError::InvalidRequest
        )?;

        if request.expected_size
            > MAX_PACKAGE_BYTES
        {
            return Err(
                DownloadError::InvalidRequest
            );
        }

        if !self.is_test_transport() {
            validate_release_asset_url(
                &request.download_url,
                &request.filename,
            )
            .map_err(
                |_| DownloadError::InvalidRequest
            )?;

            let signature_filename =
                format!(
                    "{}.sig",
                    request.filename
                );

            validate_release_asset_url(
                &request.signature_url,
                &signature_filename,
            )
            .map_err(
                |_| DownloadError::InvalidRequest
            )?;
        }

        Ok(())
    }

    async fn response_with_redirects(
        &self,
        initial_url: &str,
    ) -> Result<Response, DownloadError> {
        let mut url =
            initial_url.to_string();

        let mut followed:
            usize = 0;

        loop {
            let response =
                self.client
                    .get(&url)
                    .send()
                    .await?;

            if !response
                .status()
                .is_redirection()
            {
                return Ok(response);
            }

            if !can_follow_redirect(
                followed
            ) {
                return Err(
                    DownloadError::TooManyRedirects
                );
            }

            let location =
                response
                    .headers()
                    .get(LOCATION)
                    .ok_or(
                        DownloadError::InvalidRedirect
                    )?
                    .to_str()
                    .map_err(
                        |_| DownloadError::InvalidRedirect
                    )?;

            let next =
                response
                    .url()
                    .join(location)
                    .map_err(
                        |_| DownloadError::InvalidRedirect
                    )?;

            if !self.is_test_transport() {
                validate_redirect_target(
                    next.as_str()
                )
                .map_err(
                    |_| DownloadError::InvalidRedirect
                )?;
            }

            followed += 1;

            if followed
                > MAX_REDIRECTS
            {
                return Err(
                    DownloadError::TooManyRedirects
                );
            }

            url =
                next.to_string();
        }
    }

    async fn fetch_signature(
        &self,
        url: &str,
    ) -> Result<Vec<u8>, DownloadError> {
        let mut response =
            self.response_with_redirects(
                url
            )
            .await?;

        if !response
            .status()
            .is_success()
        {
            return Err(
                DownloadError::HttpStatus(
                    response
                        .status()
                        .as_u16()
                )
            );
        }

        let mut signature =
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
                        DownloadError::SignatureTooLarge
                    )?;

            if total
                > MAX_SIGNATURE_BYTES
            {
                return Err(
                    DownloadError::SignatureTooLarge
                );
            }

            signature
                .extend_from_slice(
                    &chunk
                );
        }

        if signature.is_empty() {
            return Err(
                DownloadError::InvalidSignature
            );
        }

        Ok(signature)
    }

    async fn cleanup_partial(
        path: &Path,
    ) {
        match fs::remove_file(
            path
        )
        .await
        {
            Ok(()) => {}

            Err(error)
                if error.kind()
                    == std::io::ErrorKind::NotFound =>
            {}

            Err(_) => {}
        }
    }

    async fn download_to_partial(
        &self,
        request: &PackageDownloadRequest,
        partial_path: &Path,
    ) -> Result<
        (u64, String),
        DownloadError,
    > {
        let mut response =
            self.response_with_redirects(
                &request.download_url
            )
            .await?;

        if !response
            .status()
            .is_success()
        {
            return Err(
                DownloadError::HttpStatus(
                    response
                        .status()
                        .as_u16()
                )
            );
        }

        Self::cleanup_partial(
            partial_path
        )
        .await;

        let mut file =
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(
                    partial_path
                )
                .await?;

        let mut hasher =
            Sha256::new();

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
                        DownloadError::SizeMismatch
                    )?;

            if total
                > request.expected_size
                || total
                    > MAX_PACKAGE_BYTES
            {
                return Err(
                    DownloadError::SizeMismatch
                );
            }

            file
                .write_all(
                    &chunk
                )
                .await?;

            hasher.update(
                &chunk
            );
        }

        file.flush().await?;
        file.sync_all().await?;

        if total
            != request.expected_size
        {
            return Err(
                DownloadError::SizeMismatch
            );
        }

        let digest =
            format!(
                "{:x}",
                hasher.finalize()
            );

        Ok(
            (
                total,
                digest,
            )
        )
    }

    pub async fn download_and_stage(
        &self,
        request: &PackageDownloadRequest,
    ) -> Result<
        StagedPackage,
        DownloadError,
    > {
        self.validate_request(
            request
        )?;

        fs::create_dir_all(
            &request.staging_directory
        )
        .await?;

        let final_path =
            request
                .staging_directory
                .join(
                    &request.filename
                );

        let partial_path =
            request
                .staging_directory
                .join(
                    format!(
                        "{}.partial",
                        request.filename
                    )
                );

        Self::cleanup_partial(
            &partial_path
        )
        .await;

        /*
         * Fetch the detached signature first, but do not
         * trust it yet. Validation happens only after the
         * exact package bytes have been streamed to the
         * non-installable .partial path.
         *
         * This also keeps all failure paths away from the
         * final installable filename.
         */
        let signature =
            match self
                .fetch_signature(
                    &request.signature_url
                )
                .await
            {
                Ok(value) => value,

                Err(error) => {
                    Self::cleanup_partial(
                        &partial_path
                    )
                    .await;

                    return Err(error);
                }
            };

        let (
            actual_size,
            actual_sha256,
        ) =
            match self
                .download_to_partial(
                    request,
                    &partial_path,
                )
                .await
            {
                Ok(value) => value,

                Err(error) => {
                    Self::cleanup_partial(
                        &partial_path
                    )
                    .await;

                    return Err(error);
                }
            };

        if actual_sha256
            != request.expected_sha256
        {
            Self::cleanup_partial(
                &partial_path
            )
            .await;

            return Err(
                DownloadError::HashMismatch
            );
        }

        /*
         * Standard Ed25519 verification in the existing
         * trust module is one-shot over the exact message.
         * The network transfer and SHA256 verification are
         * streamed; only after those pass do we read the
         * completed .partial file for detached signature
         * verification.
         */
        let package_bytes =
            match fs::read(
                &partial_path
            )
            .await
            {
                Ok(value) => value,

                Err(error) => {
                    Self::cleanup_partial(
                        &partial_path
                    )
                    .await;

                    return Err(
                        DownloadError::Io(
                            error
                        )
                    );
                }
            };

        if package_bytes.len()
            as u64
            != actual_size
        {
            Self::cleanup_partial(
                &partial_path
            )
            .await;

            return Err(
                DownloadError::SizeMismatch
            );
        }

        if verify_detached_signature(
            &package_bytes,
            &signature,
        )
        .is_err()
        {
            Self::cleanup_partial(
                &partial_path
            )
            .await;

            return Err(
                DownloadError::InvalidSignature
            );
        }

        /*
         * No final/installable path is touched until:
         *   1) exact size passed,
         *   2) SHA256 passed,
         *   3) Ed25519 signature passed.
         */
        if let Err(error) =
            fs::rename(
                &partial_path,
                &final_path,
            )
            .await
        {
            Self::cleanup_partial(
                &partial_path
            )
            .await;

            return Err(
                DownloadError::Io(
                    error
                )
            );
        }

        Ok(
            StagedPackage {
                path:
                    final_path,

                size:
                    actual_size,

                sha256:
                    actual_sha256,
            }
        )
    }
}


#[cfg(test)]
mod tests {
    use std::{
        path::{
            Path,
            PathBuf,
        },
        sync::Arc,
    };

    use sha2::{
        Digest,
        Sha256,
    };

    use tokio::{
        fs,
        io::{
            AsyncReadExt,
            AsyncWriteExt,
        },
        net::TcpListener,
        task::JoinHandle,
    };

    use uuid::Uuid;

    use super::{
        DownloadError,
        PackageDownloadRequest,
        PackageDownloadService,
        StagedPackage,
    };

    const SIGNED_BYTES: &[u8] = include_bytes!(
        "../test-fixtures/update-discovery-1.2.3/beta.json"
    );

    const VALID_SIGNATURE: &[u8] = include_bytes!(
        "../test-fixtures/update-discovery-1.2.3/beta.json.sig"
    );

    fn sha256_hex(
        bytes: &[u8],
    ) -> String {
        format!(
            "{:x}",
            Sha256::digest(bytes)
        )
    }

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(
            format!(
                "photoos-task6-{}",
                Uuid::new_v4()
            )
        )
    }

    fn request(
        base: &str,
        directory: &Path,
    ) -> PackageDownloadRequest {
        PackageDownloadRequest {
            download_url:
                format!(
                    "{base}/package"
                ),

            signature_url:
                format!(
                    "{base}/package.sig"
                ),

            filename:
                "PhotoOS-test-signed.popkg"
                    .to_string(),

            expected_size:
                SIGNED_BYTES.len()
                    as u64,

            expected_sha256:
                sha256_hex(
                    SIGNED_BYTES
                ),

            staging_directory:
                directory
                    .to_path_buf(),
        }
    }

    async fn spawn_server(
        package: Vec<u8>,
        signature: Vec<u8>,
    ) -> (
        String,
        JoinHandle<()>,
    ) {
        let listener =
            TcpListener::bind(
                "127.0.0.1:0"
            )
            .await
            .expect(
                "bind fixture server"
            );

        let address =
            listener
                .local_addr()
                .expect(
                    "fixture address"
                );

        let package =
            Arc::new(package);

        let signature =
            Arc::new(signature);

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
                                    "accept request"
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
                                    "read request"
                                );

                        let request =
                            String::from_utf8_lossy(
                                &request[..read]
                            );

                        let body: &[u8] =
                            if request
                                .starts_with(
                                    "GET /package.sig "
                                )
                            {
                                signature
                                    .as_slice()
                            } else {
                                package
                                    .as_slice()
                            };

                        let header =
                            format!(
                                concat!(
                                    "HTTP/1.1 200 OK\r\n",
                                    "Content-Length: {length}\r\n",
                                    "Content-Type: application/octet-stream\r\n",
                                    "Connection: close\r\n",
                                    "\r\n"
                                ),
                                length = body.len(),
                            );

                        socket
                            .write_all(
                                header.as_bytes()
                            )
                            .await
                            .expect(
                                "write header"
                            );

                        socket
                            .write_all(body)
                            .await
                            .expect(
                                "write body"
                            );

                        socket
                            .shutdown()
                            .await
                            .expect(
                                "shutdown socket"
                            );
                    }
                }
            );

        (
            format!(
                "http://{address}"
            ),
            handle,
        )
    }

    async fn assert_absent(
        path: &Path,
    ) {
        assert!(
            fs::metadata(path)
                .await
                .is_err(),
            "path must not exist: {}",
            path.display()
        );
    }

    #[tokio::test]
    async fn valid_signed_stream_is_atomically_finalized() {
        let directory =
            temp_dir();

        fs::create_dir_all(
            &directory
        )
        .await
        .expect(
            "create staging dir"
        );

        let (
            base,
            server,
        ) =
            spawn_server(
                SIGNED_BYTES.to_vec(),
                VALID_SIGNATURE.to_vec(),
            )
            .await;

        let request =
            request(
                &base,
                &directory,
            );

        let partial =
            directory.join(
                "PhotoOS-test-signed.popkg.partial"
            );

        let final_path =
            directory.join(
                "PhotoOS-test-signed.popkg"
            );

        let service =
            PackageDownloadService
                ::new_for_test()
                .expect(
                    "download service"
                );

        let staged =
            service
                .download_and_stage(
                    &request
                )
                .await
                .expect(
                    "valid signed stage"
                );

        assert_eq!(
            staged,
            StagedPackage {
                path:
                    final_path.clone(),

                size:
                    SIGNED_BYTES.len()
                        as u64,

                sha256:
                    sha256_hex(
                        SIGNED_BYTES
                    ),
            }
        );

        assert_absent(
            &partial
        )
        .await;

        assert_eq!(
            fs::read(
                &final_path
            )
            .await
            .expect(
                "read finalized package"
            ),
            SIGNED_BYTES
        );

        server
            .await
            .expect(
                "fixture server"
            );

        let _ =
            fs::remove_dir_all(
                &directory
            )
            .await;
    }

    #[tokio::test]
    async fn wrong_hash_never_creates_installable_final_file() {
        let directory =
            temp_dir();

        fs::create_dir_all(
            &directory
        )
        .await
        .expect(
            "create staging dir"
        );

        let (
            base,
            server,
        ) =
            spawn_server(
                SIGNED_BYTES.to_vec(),
                VALID_SIGNATURE.to_vec(),
            )
            .await;

        let mut request =
            request(
                &base,
                &directory,
            );

        request.expected_sha256 =
            "0".repeat(64);

        let service =
            PackageDownloadService
                ::new_for_test()
                .expect(
                    "download service"
                );

        let result =
            service
                .download_and_stage(
                    &request
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    DownloadError::HashMismatch
                )
            ),
            "unexpected result: {result:?}"
        );

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg.partial"
            )
        )
        .await;

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg"
            )
        )
        .await;

        server
            .await
            .expect(
                "fixture server"
            );

        let _ =
            fs::remove_dir_all(
                &directory
            )
            .await;
    }

    #[tokio::test]
    async fn invalid_signature_removes_partial_and_never_finalizes() {
        let directory =
            temp_dir();

        fs::create_dir_all(
            &directory
        )
        .await
        .expect(
            "create staging dir"
        );

        let (
            base,
            server,
        ) =
            spawn_server(
                SIGNED_BYTES.to_vec(),
                vec![0u8; 64],
            )
            .await;

        let request =
            request(
                &base,
                &directory,
            );

        let service =
            PackageDownloadService
                ::new_for_test()
                .expect(
                    "download service"
                );

        let result =
            service
                .download_and_stage(
                    &request
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    DownloadError::InvalidSignature
                )
            ),
            "unexpected result: {result:?}"
        );

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg.partial"
            )
        )
        .await;

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg"
            )
        )
        .await;

        server
            .await
            .expect(
                "fixture server"
            );

        let _ =
            fs::remove_dir_all(
                &directory
            )
            .await;
    }

    #[tokio::test]
    async fn truncated_download_is_rejected_by_exact_size() {
        let directory =
            temp_dir();

        fs::create_dir_all(
            &directory
        )
        .await
        .expect(
            "create staging dir"
        );

        let truncated =
            SIGNED_BYTES[
                ..SIGNED_BYTES.len() - 1
            ]
            .to_vec();

        let (
            base,
            server,
        ) =
            spawn_server(
                truncated,
                VALID_SIGNATURE.to_vec(),
            )
            .await;

        let request =
            request(
                &base,
                &directory,
            );

        let service =
            PackageDownloadService
                ::new_for_test()
                .expect(
                    "download service"
                );

        let result =
            service
                .download_and_stage(
                    &request
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    DownloadError::SizeMismatch
                )
            ),
            "unexpected result: {result:?}"
        );

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg.partial"
            )
        )
        .await;

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg"
            )
        )
        .await;

        server
            .await
            .expect(
                "fixture server"
            );

        let _ =
            fs::remove_dir_all(
                &directory
            )
            .await;
    }

    #[tokio::test]
    async fn oversized_stream_is_stopped_and_partial_is_removed() {
        let directory =
            temp_dir();

        fs::create_dir_all(
            &directory
        )
        .await
        .expect(
            "create staging dir"
        );

        let mut oversized =
            SIGNED_BYTES.to_vec();

        oversized.push(0);

        let (
            base,
            server,
        ) =
            spawn_server(
                oversized,
                VALID_SIGNATURE.to_vec(),
            )
            .await;

        let request =
            request(
                &base,
                &directory,
            );

        let service =
            PackageDownloadService
                ::new_for_test()
                .expect(
                    "download service"
                );

        let result =
            service
                .download_and_stage(
                    &request
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    DownloadError::SizeMismatch
                )
            ),
            "unexpected result: {result:?}"
        );

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg.partial"
            )
        )
        .await;

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg"
            )
        )
        .await;

        server
            .await
            .expect(
                "fixture server"
            );

        let _ =
            fs::remove_dir_all(
                &directory
            )
            .await;
    }

    #[tokio::test]
    async fn failed_replacement_does_not_destroy_existing_final_package() {
        let directory =
            temp_dir();

        fs::create_dir_all(
            &directory
        )
        .await
        .expect(
            "create staging dir"
        );

        let final_path =
            directory.join(
                "PhotoOS-test-signed.popkg"
            );

        let existing =
            b"existing verified package";

        fs::write(
            &final_path,
            existing,
        )
        .await
        .expect(
            "seed existing final"
        );

        let (
            base,
            server,
        ) =
            spawn_server(
                SIGNED_BYTES.to_vec(),
                vec![0u8; 64],
            )
            .await;

        let request =
            request(
                &base,
                &directory,
            );

        let service =
            PackageDownloadService
                ::new_for_test()
                .expect(
                    "download service"
                );

        let result =
            service
                .download_and_stage(
                    &request
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    DownloadError::InvalidSignature
                )
            )
        );

        assert_eq!(
            fs::read(
                &final_path
            )
            .await
            .expect(
                "existing final survives"
            ),
            existing
        );

        assert_absent(
            &directory.join(
                "PhotoOS-test-signed.popkg.partial"
            )
        )
        .await;

        server
            .await
            .expect(
                "fixture server"
            );

        let _ =
            fs::remove_dir_all(
                &directory
            )
            .await;
    }
}
