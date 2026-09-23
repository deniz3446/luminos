use std::{
    error::Error,
    fmt,
};

use serde_json::{
    Value,
    json,
};

use crate::{
    update_agent::{
        AgentTransport,
        AgentTransportError,
    },
    update_download::StagedPackage,
};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct AgentPackageIdentity {
    pub filename: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub struct VerifiedAgentPackage {
    pub identity: AgentPackageIdentity,
    pub version: String,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
pub enum InstallApproval {
    NotApproved,
    Approved,
}

#[derive(Debug)]
pub enum AgentFlowError {
    InvalidStagedPackage,
    Transport(String),
    UploadRejected(u16),
    UploadResponseInvalid,
    UploadIdentityMismatch,
    VerifyRejected,
    VerifyIdentityMismatch,
    InstallNotApproved,
    InstallRejected(u16),
    InstallResponseInvalid,
}

impl fmt::Display for AgentFlowError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::InvalidStagedPackage =>
                write!(
                    f,
                    "staged package identity is invalid"
                ),

            Self::Transport(message) =>
                write!(
                    f,
                    "Update Agent transport failed: {message}"
                ),

            Self::UploadRejected(status) =>
                write!(
                    f,
                    "Update Agent rejected package upload with HTTP {status}"
                ),

            Self::UploadResponseInvalid =>
                write!(
                    f,
                    "Update Agent upload response has an invalid identity"
                ),

            Self::UploadIdentityMismatch =>
                write!(
                    f,
                    "Update Agent upload identity does not match staged package"
                ),

            Self::VerifyRejected =>
                write!(
                    f,
                    "Update Agent package verification was rejected"
                ),

            Self::VerifyIdentityMismatch =>
                write!(
                    f,
                    "Update Agent verification identity does not match staged package"
                ),

            Self::InstallNotApproved =>
                write!(
                    f,
                    "package installation requires explicit approval"
                ),

            Self::InstallRejected(status) =>
                write!(
                    f,
                    "Update Agent rejected install request with HTTP {status}"
                ),

            Self::InstallResponseInvalid =>
                write!(
                    f,
                    "Update Agent install response is invalid"
                ),
        }
    }
}

impl Error for AgentFlowError {}

impl From<AgentTransportError> for AgentFlowError {
    fn from(
        value: AgentTransportError,
    ) -> Self {
        Self::Transport(
            value.to_string()
        )
    }
}

pub struct UpdateAgentVerificationFlow {
    transport: AgentTransport,
}

impl UpdateAgentVerificationFlow {
    pub fn new(
        base_url: String,
        token: Option<String>,
    ) -> Self {
        Self {
            transport:
                AgentTransport::new(
                    base_url,
                    token,
                ),
        }
    }

    fn staged_filename(
        staged: &StagedPackage,
    ) -> Result<
        String,
        AgentFlowError,
    > {
        let filename =
            staged
                .path
                .file_name()
                .and_then(
                    |value| value.to_str()
                )
                .ok_or(
                    AgentFlowError::InvalidStagedPackage
                )?;

        if filename.is_empty()
            || filename == "."
            || filename == ".."
            || filename.contains('/')
            || filename.contains('\\')
            || !filename.ends_with(".popkg")
        {
            return Err(
                AgentFlowError::InvalidStagedPackage
            );
        }

        if staged.size == 0
            || !Self::valid_sha256(
                &staged.sha256
            )
        {
            return Err(
                AgentFlowError::InvalidStagedPackage
            );
        }

        Ok(
            filename.to_string()
        )
    }

    fn valid_sha256(
        value: &str,
    ) -> bool {
        value.len() == 64
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

    fn successful_status(
        status: u16,
    ) -> bool {
        (
            200..300
        )
        .contains(
            &status
        )
    }

    fn body_string(
        body: &Value,
        key: &str,
    ) -> Option<String> {
        body.get(key)
            .and_then(
                Value::as_str
            )
            .map(
                str::to_string
            )
    }

    fn body_u64(
        body: &Value,
        key: &str,
    ) -> Option<u64> {
        body.get(key)
            .and_then(
                Value::as_u64
            )
    }

    fn upload_succeeded(
        body: &Value,
    ) -> bool {
        body.get("success")
            .and_then(
                Value::as_bool
            )
            .unwrap_or(false)
            || body
                .get("uploaded")
                .and_then(
                    Value::as_bool
                )
                .unwrap_or(false)
    }

    fn parse_upload_identity(
        body: &Value,
    ) -> Result<
        AgentPackageIdentity,
        AgentFlowError,
    > {
        if !Self::upload_succeeded(
            body
        ) {
            return Err(
                AgentFlowError::UploadResponseInvalid
            );
        }

        let filename =
            Self::body_string(
                body,
                "filename",
            )
            .ok_or(
                AgentFlowError::UploadResponseInvalid
            )?;

        let sha256 =
            Self::body_string(
                body,
                "sha256",
            )
            .ok_or(
                AgentFlowError::UploadResponseInvalid
            )?;

        let size =
            Self::body_u64(
                body,
                "size_bytes",
            )
            .ok_or(
                AgentFlowError::UploadResponseInvalid
            )?;

        if filename.is_empty()
            || !Self::valid_sha256(
                &sha256
            )
            || size == 0
        {
            return Err(
                AgentFlowError::UploadResponseInvalid
            );
        }

        Ok(
            AgentPackageIdentity {
                filename,
                sha256,
                size,
            }
        )
    }

    pub async fn upload_and_verify(
        &self,
        staged: &StagedPackage,
        expected_version: &str,
    ) -> Result<
        VerifiedAgentPackage,
        AgentFlowError,
    > {
        let filename =
            Self::staged_filename(
                staged
            )?;

        let upload =
            self.transport
                .upload_file(
                    "/upload-package",
                    &filename,
                    &staged.path,
                )
                .await?;

        if !Self::successful_status(
            upload.status
        ) {
            return Err(
                AgentFlowError::UploadRejected(
                    upload.status
                )
            );
        }

        let uploaded_identity =
            Self::parse_upload_identity(
                &upload.body
            )?;

        let expected_identity =
            AgentPackageIdentity {
                filename:
                    filename.clone(),

                sha256:
                    staged.sha256.clone(),

                size:
                    staged.size,
            };

        if uploaded_identity
            != expected_identity
        {
            /*
             * Critical boundary:
             * verify is never called when the exact
             * uploaded filename/hash/size differs from
             * the staged package identity.
             */
            return Err(
                AgentFlowError::UploadIdentityMismatch
            );
        }

        let verify =
            self.transport
                .post_json(
                    "/verify-package",
                    &json!({
                        "filename": filename,
                    }),
                )
                .await?;

        if !Self::successful_status(
            verify.status
        ) {
            return Err(
                AgentFlowError::VerifyRejected
            );
        }

        let valid =
            verify.body
                .get("valid")
                .and_then(
                    Value::as_bool
                )
                .unwrap_or(false);

        if !valid {
            return Err(
                AgentFlowError::VerifyRejected
            );
        }

        let verified_filename =
            Self::body_string(
                &verify.body,
                "filename",
            )
            .ok_or(
                AgentFlowError::VerifyIdentityMismatch
            )?;

        let verified_sha256 =
            Self::body_string(
                &verify.body,
                "package_sha256",
            )
            .ok_or(
                AgentFlowError::VerifyIdentityMismatch
            )?;

        let verified_version =
            Self::body_string(
                &verify.body,
                "version",
            )
            .ok_or(
                AgentFlowError::VerifyIdentityMismatch
            )?;

        if verified_filename
            != expected_identity.filename
            || verified_sha256
                != expected_identity.sha256
            || verified_version
                != expected_version
        {
            return Err(
                AgentFlowError::VerifyIdentityMismatch
            );
        }

        Ok(
            VerifiedAgentPackage {
                identity:
                    expected_identity,

                version:
                    verified_version,
            }
        )
    }

    pub async fn install_verified(
        &self,
        verified:
            &VerifiedAgentPackage,
        approval:
            InstallApproval,
    ) -> Result<
        (),
        AgentFlowError,
    > {
        /*
         * This gate is intentionally before any
         * network request. Discovery, download,
         * staging and verification cannot install.
         */
        if approval
            != InstallApproval::Approved
        {
            return Err(
                AgentFlowError::InstallNotApproved
            );
        }

        let response =
            self.transport
                .post_json(
                    "/install-package",
                    &json!({
                        "filename":
                            verified
                                .identity
                                .filename,
                    }),
                )
                .await?;

        if !Self::successful_status(
            response.status
        ) {
            return Err(
                AgentFlowError::InstallRejected(
                    response.status
                )
            );
        }

        /*
         * Existing Agent variants return different
         * success keys while beginning installation
         * asynchronously. HTTP 2xx is the transport
         * acceptance boundary; if a boolean acceptance
         * field exists and is explicitly false, reject it.
         */
        for key in [
            "accepted",
            "success",
            "started",
        ] {
            if let Some(false) =
                response
                    .body
                    .get(key)
                    .and_then(
                        Value::as_bool
                    )
            {
                return Err(
                    AgentFlowError::InstallResponseInvalid
                );
            }
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{
            SystemTime,
            UNIX_EPOCH,
        },
    };

    use sha2::{
        Digest,
        Sha256,
    };

    use tokio::{
        io::{
            AsyncReadExt,
            AsyncWriteExt,
        },
        net::TcpListener,
        sync::oneshot,
        task::JoinHandle,
    };

    use crate::update_download::StagedPackage;

    use super::{
        AgentFlowError,
        AgentPackageIdentity,
        InstallApproval,
        UpdateAgentVerificationFlow,
        VerifiedAgentPackage,
    };

    fn sha256_hex(
        bytes: &[u8],
    ) -> String {
        format!(
            "{:x}",
            Sha256::digest(bytes)
        )
    }

    fn temp_package(
        bytes: &[u8],
    ) -> StagedPackage {
        let stamp =
            SystemTime::now()
                .duration_since(
                    UNIX_EPOCH
                )
                .unwrap()
                .as_nanos();

        let path =
            std::env::temp_dir().join(
                format!(
                    "PhotoOS-task7-{stamp}.popkg"
                )
            );

        std::fs::write(
            &path,
            bytes,
        )
        .expect(
            "write staged fixture"
        );

        StagedPackage {
            path,
            size:
                bytes.len()
                    as u64,
            sha256:
                sha256_hex(bytes),
        }
    }

    fn filename(
        staged: &StagedPackage,
    ) -> String {
        staged
            .path
            .file_name()
            .and_then(
                |value| value.to_str()
            )
            .expect(
                "staged filename"
            )
            .to_string()
    }

    fn http_reason(
        status: u16,
    ) -> &'static str {
        match status {
            200 => "OK",
            201 => "Created",
            202 => "Accepted",
            400 => "Bad Request",
            409 => "Conflict",
            _ => "Test",
        }
    }

    fn content_length(
        headers: &[u8],
    ) -> usize {
        let text =
            String::from_utf8_lossy(
                headers
            );

        text.lines()
            .find_map(
                |line| {
                    let (
                        name,
                        value,
                    ) =
                        line.split_once(':')?;

                    if name
                        .eq_ignore_ascii_case(
                            "content-length"
                        )
                    {
                        value
                            .trim()
                            .parse::<usize>()
                            .ok()
                    } else {
                        None
                    }
                }
            )
            .unwrap_or(0)
    }

    async fn read_request(
        socket:
            &mut tokio::net::TcpStream,
    ) -> Vec<u8> {
        let mut all =
            Vec::<u8>::new();

        let header_end =
            loop {
                let mut chunk =
                    [0u8; 4096];

                let read =
                    socket
                        .read(
                            &mut chunk
                        )
                        .await
                        .expect(
                            "read request"
                        );

                if read == 0 {
                    panic!(
                        "connection closed before headers"
                    );
                }

                all.extend_from_slice(
                    &chunk[..read]
                );

                if let Some(index) =
                    all
                        .windows(4)
                        .position(
                            |window| {
                                window
                                    == b"\r\n\r\n"
                            }
                        )
                {
                    break index + 4;
                }
            };

        let body_length =
            content_length(
                &all[..header_end]
            );

        let expected =
            header_end
                + body_length;

        while all.len()
            < expected
        {
            let mut chunk =
                [0u8; 4096];

            let read =
                socket
                    .read(
                        &mut chunk
                    )
                    .await
                    .expect(
                        "read request body"
                    );

            if read == 0 {
                break;
            }

            all.extend_from_slice(
                &chunk[..read]
            );
        }

        all
    }

    async fn spawn_sequence(
        responses:
            Vec<(
                u16,
                String,
            )>,
    ) -> (
        String,
        oneshot::Receiver<
            Vec<Vec<u8>>
        >,
        JoinHandle<()>,
    ) {
        let listener =
            TcpListener::bind(
                "127.0.0.1:0"
            )
            .await
            .expect(
                "bind local agent stub"
            );

        let address =
            listener
                .local_addr()
                .expect(
                    "stub address"
                );

        let (
            tx,
            rx,
        ) =
            oneshot::channel();

        let handle =
            tokio::spawn(
                async move {
                    let mut requests =
                        Vec::new();

                    for (
                        status,
                        body,
                    ) in responses
                    {
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

                        let request =
                            read_request(
                                &mut socket
                            )
                            .await;

                        requests.push(
                            request
                        );

                        let response =
                            format!(
                                concat!(
                                    "HTTP/1.1 {status} {reason}\r\n",
                                    "Content-Type: application/json\r\n",
                                    "Content-Length: {length}\r\n",
                                    "Connection: close\r\n",
                                    "\r\n",
                                    "{body}"
                                ),
                                status = status,
                                reason =
                                    http_reason(
                                        status
                                    ),
                                length =
                                    body.as_bytes()
                                        .len(),
                                body = body,
                            );

                        socket
                            .write_all(
                                response
                                    .as_bytes()
                            )
                            .await
                            .expect(
                                "write stub response"
                            );

                        socket
                            .shutdown()
                            .await
                            .expect(
                                "shutdown stub"
                            );
                    }

                    let _ =
                        tx.send(
                            requests
                        );
                }
            );

        (
            format!(
                "http://{address}"
            ),
            rx,
            handle,
        )
    }

    fn request_text(
        request: &[u8],
    ) -> String {
        String::from_utf8_lossy(
            request
        )
        .to_string()
    }

    #[tokio::test]
    async fn exact_upload_and_verify_identity_is_accepted_without_install() {
        let staged =
            temp_package(
                b"PHOTOOS-TASK7-IDENTITY"
            );

        let name =
            filename(
                &staged
            );

        let upload =
            format!(
                concat!(
                    "{{",
                    "\"success\":true,",
                    "\"filename\":\"{}\",",
                    "\"sha256\":\"{}\",",
                    "\"size_bytes\":{}",
                    "}}"
                ),
                name,
                staged.sha256,
                staged.size,
            );

        let verify =
            format!(
                concat!(
                    "{{",
                    "\"valid\":true,",
                    "\"version\":\"1.2.4\",",
                    "\"filename\":\"{}\",",
                    "\"package_sha256\":\"{}\"",
                    "}}"
                ),
                name,
                staged.sha256,
            );

        let (
            base,
            requests_rx,
            server,
        ) =
            spawn_sequence(
                vec![
                    (
                        201,
                        upload,
                    ),
                    (
                        200,
                        verify,
                    ),
                ]
            )
            .await;

        let flow =
            UpdateAgentVerificationFlow
                ::new(
                    base,
                    Some(
                        "a".repeat(64)
                    ),
                );

        let verified =
            flow
                .upload_and_verify(
                    &staged,
                    "1.2.4",
                )
                .await
                .expect(
                    "exact identity must verify"
                );

        assert_eq!(
            verified,
            VerifiedAgentPackage {
                identity:
                    AgentPackageIdentity {
                        filename:
                            name.clone(),
                        sha256:
                            staged
                                .sha256
                                .clone(),
                        size:
                            staged.size,
                    },
                version:
                    "1.2.4"
                        .to_string(),
            }
        );

        let requests =
            requests_rx
                .await
                .expect(
                    "recorded requests"
                );

        assert_eq!(
            requests.len(),
            2,
            "verification flow must not install"
        );

        let first =
            request_text(
                &requests[0]
            );

        let second =
            request_text(
                &requests[1]
            );

        assert!(
            first.starts_with(
                "POST /upload-package HTTP/1.1"
            )
        );

        assert!(
            second.starts_with(
                "POST /verify-package HTTP/1.1"
            )
        );

        assert!(
            !first.contains(
                "/install-package"
            )
        );

        assert!(
            !second.contains(
                "/install-package"
            )
        );

        server
            .await
            .expect(
                "stub server"
            );

        std::fs::remove_file(
            &staged.path
        )
        .unwrap();
    }

    #[tokio::test]
    async fn upload_identity_mismatch_stops_before_verify() {
        let staged =
            temp_package(
                b"PHOTOOS-TASK7-UPLOAD"
            );

        let name =
            filename(
                &staged
            );

        let upload =
            format!(
                concat!(
                    "{{",
                    "\"success\":true,",
                    "\"filename\":\"{}\",",
                    "\"sha256\":\"{}\",",
                    "\"size_bytes\":{}",
                    "}}"
                ),
                name,
                "0".repeat(64),
                staged.size + 1,
            );

        let (
            base,
            requests_rx,
            server,
        ) =
            spawn_sequence(
                vec![
                    (
                        201,
                        upload,
                    ),
                ]
            )
            .await;

        let flow =
            UpdateAgentVerificationFlow
                ::new(
                    base,
                    None,
                );

        let result =
            flow
                .upload_and_verify(
                    &staged,
                    "1.2.4",
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    AgentFlowError
                        ::UploadIdentityMismatch
                )
            ),
            "unexpected result: {result:?}"
        );

        let requests =
            requests_rx
                .await
                .unwrap();

        assert_eq!(
            requests.len(),
            1,
            "verify must not run after bad upload identity"
        );

        server.await.unwrap();

        std::fs::remove_file(
            &staged.path
        )
        .unwrap();
    }

    #[tokio::test]
    async fn verify_identity_mismatch_is_rejected() {
        let staged =
            temp_package(
                b"PHOTOOS-TASK7-VERIFY"
            );

        let name =
            filename(
                &staged
            );

        let upload =
            format!(
                concat!(
                    "{{",
                    "\"success\":true,",
                    "\"filename\":\"{}\",",
                    "\"sha256\":\"{}\",",
                    "\"size_bytes\":{}",
                    "}}"
                ),
                name,
                staged.sha256,
                staged.size,
            );

        let verify =
            format!(
                concat!(
                    "{{",
                    "\"valid\":true,",
                    "\"version\":\"1.2.4\",",
                    "\"filename\":\"{}\",",
                    "\"package_sha256\":\"{}\"",
                    "}}"
                ),
                name,
                "f".repeat(64),
            );

        let (
            base,
            _requests_rx,
            server,
        ) =
            spawn_sequence(
                vec![
                    (
                        201,
                        upload,
                    ),
                    (
                        200,
                        verify,
                    ),
                ]
            )
            .await;

        let flow =
            UpdateAgentVerificationFlow
                ::new(
                    base,
                    None,
                );

        let result =
            flow
                .upload_and_verify(
                    &staged,
                    "1.2.4",
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    AgentFlowError
                        ::VerifyIdentityMismatch
                )
            ),
            "unexpected result: {result:?}"
        );

        server.await.unwrap();

        std::fs::remove_file(
            &staged.path
        )
        .unwrap();
    }

    #[tokio::test]
    async fn verify_valid_false_is_rejected() {
        let staged =
            temp_package(
                b"PHOTOOS-TASK7-INVALID"
            );

        let name =
            filename(
                &staged
            );

        let upload =
            format!(
                concat!(
                    "{{",
                    "\"success\":true,",
                    "\"filename\":\"{}\",",
                    "\"sha256\":\"{}\",",
                    "\"size_bytes\":{}",
                    "}}"
                ),
                name,
                staged.sha256,
                staged.size,
            );

        let verify =
            format!(
                concat!(
                    "{{",
                    "\"valid\":false,",
                    "\"version\":\"1.2.4\",",
                    "\"filename\":\"{}\",",
                    "\"package_sha256\":\"{}\"",
                    "}}"
                ),
                name,
                staged.sha256,
            );

        let (
            base,
            _requests_rx,
            server,
        ) =
            spawn_sequence(
                vec![
                    (
                        201,
                        upload,
                    ),
                    (
                        200,
                        verify,
                    ),
                ]
            )
            .await;

        let flow =
            UpdateAgentVerificationFlow
                ::new(
                    base,
                    None,
                );

        let result =
            flow
                .upload_and_verify(
                    &staged,
                    "1.2.4",
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    AgentFlowError
                        ::VerifyRejected
                )
            ),
            "unexpected result: {result:?}"
        );

        server.await.unwrap();

        std::fs::remove_file(
            &staged.path
        )
        .unwrap();
    }

    #[tokio::test]
    async fn install_without_explicit_approval_is_rejected_before_network() {
        let verified =
            VerifiedAgentPackage {
                identity:
                    AgentPackageIdentity {
                        filename:
                            "PhotoOS-test.popkg"
                                .to_string(),

                        sha256:
                            "a".repeat(64),

                        size:
                            123,
                    },

                version:
                    "1.2.4"
                        .to_string(),
            };

        let flow =
            UpdateAgentVerificationFlow
                ::new(
                    "http://127.0.0.1:1"
                        .to_string(),
                    None,
                );

        let result =
            flow
                .install_verified(
                    &verified,
                    InstallApproval::NotApproved,
                )
                .await;

        assert!(
            matches!(
                result,
                Err(
                    AgentFlowError
                        ::InstallNotApproved
                )
            ),
            "must fail before any network request: {result:?}"
        );
    }

    #[tokio::test]
    async fn approved_install_is_a_separate_explicit_request() {
        let verified =
            VerifiedAgentPackage {
                identity:
                    AgentPackageIdentity {
                        filename:
                            "PhotoOS-test.popkg"
                                .to_string(),

                        sha256:
                            "b".repeat(64),

                        size:
                            456,
                    },

                version:
                    "1.2.4"
                        .to_string(),
            };

        let (
            base,
            requests_rx,
            server,
        ) =
            spawn_sequence(
                vec![
                    (
                        202,
                        r#"{"accepted":true}"#
                            .to_string(),
                    ),
                ]
            )
            .await;

        let flow =
            UpdateAgentVerificationFlow
                ::new(
                    base,
                    Some(
                        "c".repeat(64)
                    ),
                );

        flow
            .install_verified(
                &verified,
                InstallApproval::Approved,
            )
            .await
            .expect(
                "approved local stub install request"
            );

        let requests =
            requests_rx
                .await
                .unwrap();

        assert_eq!(
            requests.len(),
            1
        );

        let request =
            request_text(
                &requests[0]
            );

        assert!(
            request.starts_with(
                "POST /install-package HTTP/1.1"
            )
        );

        assert!(
            request.contains(
                r#"{"filename":"PhotoOS-test.popkg"}"#
            )
        );

        server.await.unwrap();
    }
}
