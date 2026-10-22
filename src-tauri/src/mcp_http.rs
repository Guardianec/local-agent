//! HTTP policy adapter only; rmcp owns MCP lifecycle, metadata, cancellation and SSE semantics.
use futures_util::{stream::BoxStream, StreamExt};
use http::{HeaderName, HeaderValue};
use rmcp::{
    model::ClientJsonRpcMessage,
    transport::streamable_http_client::{
        StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
    },
};
use sse_stream::{Sse, SseStream};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

const BODY_LIMIT: usize = 1024 * 1024;
const TOTAL_LIMIT: usize = 16 * 1024 * 1024;
type Error = StreamableHttpError<std::io::Error>;

#[derive(Clone)]
pub(crate) struct Http {
    pub url: reqwest::Url,
    pub local: bool,
    pub headers: HashMap<HeaderName, HeaderValue>,
    total: Arc<AtomicUsize>,
    pub error: Arc<Mutex<Option<crate::domain::AppError>>>,
}
fn failure(message: &'static str) -> Error {
    StreamableHttpError::UnexpectedServerResponse(message.into())
}
impl Http {
    pub fn new(url: reqwest::Url, local: bool, headers: HashMap<HeaderName, HeaderValue>) -> Self {
        Self {
            url,
            local,
            headers,
            total: Arc::new(AtomicUsize::new(0)),
            error: Arc::new(Mutex::new(None)),
        }
    }
    fn fail(&self, code: &str, message: &'static str) -> Error {
        *self.error.lock().unwrap() = Some(crate::domain::AppError::new(code, message));
        failure(message)
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the SDK HTTP transport contract.
    async fn request(
        &self,
        method: reqwest::Method,
        uri: &str,
        session: Option<&str>,
        auth: Option<&str>,
        headers: HashMap<HeaderName, HeaderValue>,
        message: Option<&ClientJsonRpcMessage>,
    ) -> Result<reqwest::Response, Error> {
        if uri != self.url.as_str() {
            return Err(failure("MCP endpoint changed; reconnect through Settings."));
        }
        let host = self
            .url
            .host_str()
            .ok_or_else(|| failure("Missing MCP host."))?
            .trim_matches(['[', ']']);
        let port = self
            .url
            .port_or_known_default()
            .ok_or_else(|| failure("Missing MCP port."))?;
        let addresses: Vec<_> = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| failure("MCP DNS timed out."))?
        .map_err(|_| failure("MCP DNS failed."))?
        .collect();
        if addresses.is_empty()
            || addresses.iter().any(|address| {
                if self.local {
                    !address.ip().is_loopback()
                } else {
                    !crate::browser::public_ip(address.ip())
                }
            })
        {
            return Err(failure("MCP network policy refused the resolved address."));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &addresses)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| failure("MCP TLS client could not start."))?;
        let mut request = client
            .request(method, self.url.clone())
            .header("accept", "application/json, text/event-stream");
        for (name, value) in self.headers.iter().chain(headers.iter()) {
            request = request.header(name, value);
        }
        if let Some(session) = session {
            request = request.header("mcp-session-id", session);
        }
        if let Some(auth) = auth {
            request = request.bearer_auth(auth);
        }
        if let Some(message) = message {
            request = request.json(message);
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                self.fail("MCP_TIMEOUT", "MCP request timed out; it was not retried.")
            } else {
                self.fail(
                    "MCP_NETWORK",
                    "MCP network or TLS request failed. Check the endpoint and certificate.",
                )
            }
        })?;
        if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
            return Err(self.fail(
                "MCP_AUTH",
                "MCP authentication required; update the protected credential in Settings.",
            ));
        }
        if response.status().as_u16() == 429 {
            return Err(self.fail("MCP_RATE_LIMIT", "MCP rate limit reached. Try again later."));
        }
        if response.status().is_redirection() {
            return Err(self.fail(
                "MCP_REDIRECT",
                "MCP redirects are blocked. Configure the final endpoint explicitly.",
            ));
        }
        Ok(response)
    }
    fn stream(
        &self,
        response: reqwest::Response,
    ) -> BoxStream<'static, Result<tokio_util::bytes::Bytes, std::io::Error>> {
        let total = self.total.clone();
        let mut count = 0usize;
        response
            .bytes_stream()
            .map(move |chunk| {
                let chunk = chunk.map_err(|_| std::io::Error::other("MCP response interrupted"))?;
                count = count.saturating_add(chunk.len());
                let used = total
                    .fetch_add(chunk.len(), Ordering::Relaxed)
                    .saturating_add(chunk.len());
                if count > BODY_LIMIT || used > TOTAL_LIMIT {
                    return Err(std::io::Error::other("MCP response budget exceeded"));
                }
                Ok(chunk)
            })
            .boxed()
    }
}
impl StreamableHttpClient for Http {
    type Error = std::io::Error;
    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, Error> {
        let response = self
            .request(
                reqwest::Method::POST,
                &uri,
                session.as_deref(),
                auth.as_deref(),
                headers,
                Some(&message),
            )
            .await?;
        let status = response.status();
        if status.as_u16() == 202 || status.as_u16() == 204 {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        let session = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mime = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if mime.starts_with("text/event-stream") && status.is_success() {
            return Ok(StreamableHttpPostResponse::Sse(
                SseStream::from_bytes_stream(self.stream(response)).boxed(),
                session,
            ));
        }
        let mut stream = self.stream(response);
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            body.extend_from_slice(
                &chunk
                    .map_err(|_| failure("MCP response exceeded its limit or was interrupted."))?,
            );
        }
        // Parse protocol errors on non-2xx statuses too, so SDK legacy fallback remains authoritative.
        if mime.starts_with("application/json") {
            if let Ok(message) = serde_json::from_slice(&body) {
                return Ok(StreamableHttpPostResponse::Json(message, session));
            }
        }
        Err(failure(
            "MCP returned an unsupported HTTP status or invalid JSON response.",
        ))
    }
    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), Error> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &uri,
                Some(&session),
                auth.as_deref(),
                headers,
                None,
            )
            .await?;
        if response.status().is_success() || matches!(response.status().as_u16(), 404 | 405) {
            Ok(())
        } else {
            Err(failure("MCP session cleanup failed."))
        }
    }
    async fn get_stream(
        &self,
        uri: Arc<str>,
        session: Option<Arc<str>>,
        last: Option<String>,
        auth: Option<String>,
        mut headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, sse_stream::Error>>, Error> {
        if let Some(last) = last {
            headers.insert(
                HeaderName::from_static("last-event-id"),
                HeaderValue::from_str(&last).map_err(|_| failure("Invalid MCP event ID."))?,
            );
        }
        let response = self
            .request(
                reqwest::Method::GET,
                &uri,
                session.as_deref(),
                auth.as_deref(),
                headers,
                None,
            )
            .await?;
        if response.status().as_u16() == 405 {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        if !response.status().is_success() {
            return Err(failure("MCP event stream failed."));
        }
        Ok(SseStream::from_bytes_stream(self.stream(response)).boxed())
    }
}
