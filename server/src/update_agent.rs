use std::{
    error::Error,
    fmt,
    path::Path,
    time::Duration,
};

use reqwest::{
    Client,
    RequestBuilder,
    header::{CONTENT_LENGTH, CONTENT_TYPE},
};
use serde_json::Value;
use tokio_util::io::ReaderStream;

const UPDATE_TOKEN_HEADER: &str = "X-PhotoOS-Update-Token";
const UPDATE_FILENAME_HEADER: &str = "X-PhotoOS-Filename";

#[derive(Debug)]
pub struct AgentResponse {
    pub status: u16,
    pub body: Value,
}

#[derive(Debug)]
pub enum AgentTransportError {
    Request {
        message: String,
    },
    File {
        message: String,
    },
    InvalidJson {
        status: u16,
        message: String,
    },
}

impl fmt::Display for AgentTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request { message } => {
                write!(f, "Update Agent request failed: {message}")
            }
            Self::File { message } => {
                write!(f, "Update package file error: {message}")
            }
            Self::InvalidJson { status, message } => {
                write!(
                    f,
                    "Update Agent returned invalid JSON with HTTP {status}: {message}"
                )
            }
        }
    }
}

impl Error for AgentTransportError {}

#[derive(Clone)]
pub struct AgentTransport {
    base_url: String,
    token: Option<String>,
    client: Client,
}

impl AgentTransport {
    pub fn new(base_url: String, token: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
            client: Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url,
            path.trim_start_matches('/')
        )
    }

    fn authenticated(&self, request: RequestBuilder) -> RequestBuilder {
        match self.token.as_deref() {
            Some(token) => request.header(UPDATE_TOKEN_HEADER, token),
            None => request,
        }
    }

    async fn decode(
        response: reqwest::Response,
    ) -> Result<AgentResponse, AgentTransportError> {
        let status = response.status().as_u16();

        let body = response
            .json::<Value>()
            .await
            .map_err(|error| AgentTransportError::InvalidJson {
                status,
                message: error.to_string(),
            })?;

        Ok(AgentResponse { status, body })
    }

    pub async fn get_json(
        &self,
        path: &str,
    ) -> Result<AgentResponse, AgentTransportError> {
        let response = self
            .client
            .get(self.url(path))
            .timeout(Duration::from_secs(12))
            .send()
            .await
            .map_err(|error| AgentTransportError::Request {
                message: error.to_string(),
            })?;

        Self::decode(response).await
    }

    pub async fn post_json(
        &self,
        path: &str,
        payload: &Value,
    ) -> Result<AgentResponse, AgentTransportError> {
        let request = self
            .client
            .post(self.url(path))
            .json(payload)
            .timeout(Duration::from_secs(120));

        let response = self
            .authenticated(request)
            .send()
            .await
            .map_err(|error| AgentTransportError::Request {
                message: error.to_string(),
            })?;

        Self::decode(response).await
    }

    pub async fn upload_file(
        &self,
        path: &str,
        filename: &str,
        package_path: &Path,
    ) -> Result<AgentResponse, AgentTransportError> {
        let file = tokio::fs::File::open(package_path)
            .await
            .map_err(|error| AgentTransportError::File {
                message: format!(
                    "{}: {error}",
                    package_path.display()
                ),
            })?;

        let size = file
            .metadata()
            .await
            .map_err(|error| AgentTransportError::File {
                message: format!(
                    "{}: {error}",
                    package_path.display()
                ),
            })?
            .len();

        let stream = ReaderStream::new(file);
        let body = reqwest::Body::wrap_stream(stream);

        let request = self
            .client
            .post(self.url(path))
            .header(UPDATE_FILENAME_HEADER, filename)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, size.to_string())
            .body(body)
            .timeout(Duration::from_secs(600));

        let response = self
            .authenticated(request)
            .send()
            .await
            .map_err(|error| AgentTransportError::Request {
                message: error.to_string(),
            })?;

        Self::decode(response).await
    }
}


#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    use super::{AgentTransport, AgentTransportError};

    fn header_end(bytes: &[u8]) -> Option<usize> {
        bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
    }

    fn content_length(headers: &[u8]) -> usize {
        let text = String::from_utf8_lossy(headers);

        text.lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;

                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    async fn spawn_stub(
        status: u16,
        body: &'static str,
    ) -> (String, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (request_tx, request_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];

            loop {
                let read = socket.read(&mut chunk).await.unwrap();

                if read == 0 {
                    break;
                }

                request.extend_from_slice(&chunk[..read]);

                if let Some(end) = header_end(&request) {
                    let expected =
                        end + content_length(&request[..end]);

                    if request.len() >= expected {
                        break;
                    }
                }
            }

            let _ = request_tx.send(request);

            let status_text = match status {
                200 => "OK",
                201 => "Created",
                202 => "Accepted",
                409 => "Conflict",
                502 => "Bad Gateway",
                _ => "Test",
            };

            let response = format!(
                "HTTP/1.1 {status} {status_text}\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {body}",
                body.as_bytes().len()
            );

            socket.write_all(response.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
        });

        (format!("http://{address}"), request_rx)
    }

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "photoos-agent-test-{}-{stamp}-{name}",
            std::process::id()
        ));

        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[tokio::test]
    async fn typed_get_preserves_status_and_json_body() {
        let (base_url, request_rx) =
            spawn_stub(202, r#"{"ok":true,"mode":"test"}"#).await;

        let transport = AgentTransport::new(base_url, None);

        let response = transport
            .get_json("/status")
            .await
            .expect("typed GET must succeed");

        assert_eq!(response.status, 202);
        assert_eq!(response.body["ok"], true);
        assert_eq!(response.body["mode"], "test");

        let request =
            String::from_utf8(request_rx.await.unwrap()).unwrap();

        assert!(request.starts_with("GET /status HTTP/1.1"));
    }

    #[tokio::test]
    async fn typed_post_sends_token_and_json_body() {
        let (base_url, request_rx) =
            spawn_stub(200, r#"{"valid":true}"#).await;

        let token = "a".repeat(64);

        let transport =
            AgentTransport::new(base_url, Some(token.clone()));

        let response = transport
            .post_json(
                "/verify-package",
                &json!({"filename":"photoos-test.popkg"}),
            )
            .await
            .expect("typed POST must succeed");

        assert_eq!(response.status, 200);
        assert_eq!(response.body["valid"], true);

        let request =
            String::from_utf8(request_rx.await.unwrap()).unwrap();

        assert!(request.starts_with(
            "POST /verify-package HTTP/1.1"
        ));

        assert!(request.to_ascii_lowercase().contains(
            &format!(
                "x-photoos-update-token: {}",
                token
            )
            .to_ascii_lowercase()
        ));

        assert!(request.contains(
            r#"{"filename":"photoos-test.popkg"}"#
        ));
    }

    #[tokio::test]
    async fn invalid_agent_json_is_a_typed_error() {
        let (base_url, _) =
            spawn_stub(502, "not-json").await;

        let transport = AgentTransport::new(base_url, None);

        let error = transport
            .get_json("/status")
            .await
            .expect_err("invalid JSON must fail");

        match error {
            AgentTransportError::InvalidJson {
                status,
                ..
            } => {
                assert_eq!(status, 502);
            }

            other => {
                panic!("unexpected error: {other:?}");
            }
        }
    }

    #[tokio::test]
    async fn upload_file_streams_exact_bytes_and_identity_headers() {
        let package = temp_file(
            "photoos-test.popkg",
            b"PHOTOOS-STREAM-TEST",
        );

        let (base_url, request_rx) =
            spawn_stub(
                201,
                r#"{"uploaded":true,"filename":"photoos-test.popkg"}"#,
            )
            .await;

        let token = "b".repeat(64);

        let transport =
            AgentTransport::new(base_url, Some(token.clone()));

        let response = transport
            .upload_file(
                "/upload-package",
                "photoos-test.popkg",
                &package,
            )
            .await
            .expect("streaming upload must succeed");

        assert_eq!(response.status, 201);
        assert_eq!(response.body["uploaded"], true);

        let request_bytes = request_rx.await.unwrap();
        let request =
            String::from_utf8_lossy(&request_bytes);

        assert!(request.starts_with(
            "POST /upload-package HTTP/1.1"
        ));

        let lowered = request.to_ascii_lowercase();

        assert!(lowered.contains(
            "content-type: application/octet-stream"
        ));

        assert!(lowered.contains(
            "x-photoos-filename: photoos-test.popkg"
        ));

        assert!(lowered.contains(
            &format!("x-photoos-update-token: {}", token)
                .to_ascii_lowercase()
        ));

        assert!(lowered.contains(
            "content-length: 19"
        ));

        assert!(
            request_bytes.ends_with(b"PHOTOOS-STREAM-TEST")
        );

        std::fs::remove_file(package).unwrap();
    }
}
