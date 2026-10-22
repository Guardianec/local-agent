use crate::{capabilities::Effect, domain::*, security, store::Store};
use process_wrap::tokio::{ChildWrapper, CommandWrap, JobObject, KillOnDrop};
use rmcp::{
    model::CallToolRequestParams,
    service::{serve_client_with_lifecycle_and_ct, ClientLifecycleMode, RunningService},
    RoleClient,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    time::Duration,
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../src/generated/")]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum Transport {
    Stdio {
        executable: String,
        arguments: Vec<String>,
        directory: String,
        environment: BTreeMap<String, String>,
    },
    Http {
        url: String,
        local: bool,
        headers: BTreeMap<String, String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../src/generated/")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Integration {
    pub id: String,
    pub name: String,
    pub preset: String,
    pub enabled: bool,
    pub transport: Transport,
    pub credential: Option<String>,
    pub approved: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    pub alias: String,
    pub description: String,
    pub schema: Value,
    pub fingerprint: String,
    pub effect: Effect,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub state: String,
    pub authentication: String,
    pub last_error: Option<String>,
    pub checked_at: String,
    pub protocol: String,
    pub tools: Vec<(String, Effect)>,
    pub withheld: usize,
}

pub fn endpoint(url: &str, local: bool) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AppError::new("MCP_CONFIG", "Enter a complete MCP HTTPS URL."))?;
    let host = parsed.host_str().unwrap_or("").trim_matches(['[', ']']);
    if url.len() > 2048
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || parsed.query().is_some()
        || (!local && (parsed.scheme() != "https" || parsed.port_or_known_default() != Some(443)))
        || (local
            && (!matches!(parsed.scheme(), "http" | "https")
                || !host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
                || matches!(
                    parsed.port_or_known_default(),
                    Some(11434 | 9222 | 9223 | 2375 | 2376)
                )))
    {
        return Err(AppError::new("MCP_CONFIG", "Use HTTPS without credentials, query or fragment. Local endpoints require an explicit loopback IP and a non-administrative port."));
    }
    Ok(parsed)
}

impl Integration {
    pub fn group(&self) -> String {
        match self.preset.as_str() {
            "github" => "github_read".into(),
            "context7" => "external_docs".into(),
            _ => format!("custom_mcp:{}", self.id),
        }
    }
    pub fn validate(&self) -> Result<()> {
        if uuid::Uuid::parse_str(&self.id).is_err()
            || self.name.trim().is_empty()
            || self.name.len() > 80
            || !matches!(self.preset.as_str(), "github" | "context7" | "custom")
        {
            return Err(AppError::new(
                "MCP_CONFIG",
                "Invalid integration identity or name.",
            ));
        }
        match &self.transport {
            Transport::Stdio {
                executable,
                arguments,
                directory,
                environment,
            } => {
                if self.preset != "custom"
                    || !Path::new(executable).is_absolute()
                    || !Path::new(executable).is_file()
                    || !Path::new(directory).is_absolute()
                    || !Path::new(directory).is_dir()
                    || arguments.len() > 32
                    || environment.len() > 16
                    || arguments.iter().any(|v| v.len() > 2048 || v.contains('\0'))
                    || !executable.to_lowercase().ends_with(".exe")
                {
                    return Err(AppError::new("MCP_CONFIG", "Choose an installed .exe, an existing working directory and at most 32 bounded arguments. Scripts require an explicit interpreter."));
                }
                for (key, reference) in environment {
                    if key.is_empty()
                        || key.len() > 80
                        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        || reference != "credential"
                    {
                        return Err(AppError::new("MCP_CONFIG", "Environment values must reference the protected credential; ambient environment import is disabled."));
                    }
                }
            }
            Transport::Http {
                url,
                local,
                headers,
            } => {
                let url = endpoint(url, *local)?;
                if headers.len() > 8
                    || headers.iter().any(|(name, reference)| {
                        http::HeaderName::from_bytes(name.as_bytes()).is_err()
                            || reference != "credential"
                            || matches!(
                                name.to_ascii_lowercase().as_str(),
                                "host"
                                    | "cookie"
                                    | "content-length"
                                    | "connection"
                                    | "transfer-encoding"
                            )
                            || name.to_ascii_lowercase().starts_with("mcp-")
                    })
                {
                    return Err(AppError::new("MCP_CONFIG", "Custom headers must reference the protected credential and cannot override transport headers."));
                }
                if self.preset == "github"
                    && (url.as_str() != "https://api.githubcopilot.com/mcp/"
                        || *local
                        || !headers.is_empty())
                    || self.preset == "context7"
                        && (url.as_str() != "https://mcp.context7.com/mcp"
                            || *local
                            || !headers.is_empty())
                {
                    return Err(AppError::new(
                        "MCP_CONFIG",
                        "Official presets use their fixed upstream endpoints.",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub fn list(store: &Store) -> Result<Vec<Integration>> {
    store.list("integrations")
}

/// Only trusted Settings IPC calls this; no model tool creates configurations.
pub fn save(
    store: &Store,
    mut integration: Integration,
    secret: Option<String>,
    confirmed: bool,
) -> Result<Integration> {
    integration.validate()?;
    if !confirmed {
        return Err(AppError::new(
            "MCP_CONSENT",
            "Review and confirm the exact server configuration before saving.",
        ));
    }
    if list(store)?.len() >= 16
        && store
            .get::<Integration>("integrations", &integration.id)
            .is_err()
    {
        return Err(AppError::new(
            "MCP_LIMIT",
            "At most 16 integrations can be configured.",
        ));
    }
    let previous = store
        .get::<Integration>("integrations", &integration.id)
        .ok();
    // A renderer cannot choose arbitrary blob references.
    integration.credential = previous.and_then(|value| value.credential);
    if let Some(secret) = secret {
        if secret.len() > 4096 || secret.contains(['\r', '\n', '\0']) {
            return Err(AppError::new(
                "MCP_CREDENTIAL",
                "The credential is invalid or too long.",
            ));
        }
        integration.credential = if secret.is_empty() {
            None
        } else {
            // Random salt prevents the blob filename from becoming a credential hash oracle.
            Some(store.blob(&serde_json::to_vec(&json!({"nonce":id(),"secret":secret}))?)?)
        };
    }
    integration.approved = true;
    store.put("integrations", &integration.id, &integration)?;
    store.remove("integration_status", &integration.id)?;
    Ok(integration)
}

pub fn secret(store: &Store, integration: &Integration) -> Result<Option<String>> {
    integration
        .credential
        .as_ref()
        .map(|reference| {
            let value: Value = serde_json::from_slice(&store.bytes(reference)?)?;
            value["secret"].as_str().map(str::to_owned).ok_or_else(|| {
                AppError::new("MCP_CREDENTIAL", "Re-enter this integration's credential.")
            })
        })
        .transpose()
}

fn effect(integration: &Integration, name: &str) -> Effect {
    // Neither a name prefix nor server-provided readOnlyHint grants authority.
    if integration.preset == "context7" && matches!(name, "resolve-library-id" | "query-docs")
        || integration.preset == "github"
            && matches!(
                name,
                "get_file_contents"
                    | "get_commit"
                    | "list_commits"
                    | "list_branches"
                    | "search_repositories"
                    | "search_issues"
                    | "search_pull_requests"
                    | "issue_read"
                    | "pull_request_read"
                    | "actions_get"
                    | "actions_list"
                    | "get_job_logs"
                    | "list_issues"
                    | "list_pull_requests"
            )
    {
        Effect::Read
    } else {
        Effect::ExternalUnknown
    }
}

fn check_schema(value: &Value, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    match value {
        Value::Object(object) => {
            object.len() <= 128
                && object.iter().all(|(key, value)| {
                    !matches!(
                        key.as_str(),
                        "$ref" | "$dynamicRef" | "$recursiveRef" | "pattern" | "patternProperties"
                    ) && check_schema(value, depth + 1)
                })
        }
        Value::Array(array) => {
            array.len() <= 128 && array.iter().all(|value| check_schema(value, depth + 1))
        }
        _ => true,
    }
}

pub struct Connection {
    service: Option<RunningService<RoleClient, ()>>,
    child: Option<Box<dyn ChildWrapper>>,
    cancel: CancellationToken,
    pub tools: Vec<ToolInfo>,
    pub protocol: String,
    credential: Option<String>,
    pub withheld: usize,
    http_error: Option<std::sync::Arc<std::sync::Mutex<Option<AppError>>>>,
}
fn protocol_error() -> AppError {
    AppError::new("MCP_PROTOCOL", "The MCP server failed, returned invalid protocol data, or needs unsupported authentication/input. Reconnect through Settings; no call is retried automatically.")
}

fn contains_credential(value: &Value, secret: &str) -> bool {
    match value {
        Value::String(text) => text.contains(secret),
        Value::Array(items) => items.iter().any(|item| contains_credential(item, secret)),
        Value::Object(fields) => fields
            .iter()
            .any(|(key, value)| key.contains(secret) || contains_credential(value, secret)),
        _ => false,
    }
}
fn redact_credential(value: &mut Value, secret: &str) {
    match value {
        Value::String(text) => *text = text.replace(secret, "[protected credential]"),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| redact_credential(item, secret)),
        Value::Object(fields) => {
            let old = std::mem::take(fields);
            for (key, mut value) in old {
                redact_credential(&mut value, secret);
                fields.insert(key.replace(secret, "[protected credential]"), value);
            }
        }
        _ => {}
    }
}

impl Connection {
    fn failure(&self) -> AppError {
        if self.cancel.is_cancelled() {
            AppError::new("CANCELLED", "MCP operation stopped.")
        } else {
            self.http_error
                .as_ref()
                .and_then(|error| error.lock().unwrap().clone())
                .unwrap_or_else(protocol_error)
        }
    }
    pub async fn connect(
        config: &Integration,
        credential: Option<String>,
        parent: &CancellationToken,
    ) -> Result<Self> {
        config.validate()?;
        if !config.approved || !config.enabled {
            return Err(AppError::new(
                "MCP_DISABLED",
                "Enable and approve this integration in Settings first.",
            ));
        }
        let cancel = parent.child_token();
        let mut connection = Self {
            service: None,
            child: None,
            cancel: cancel.clone(),
            tools: vec![],
            protocol: String::new(),
            credential,
            withheld: 0,
            http_error: None,
        };
        let lifecycle = ClientLifecycleMode::Auto {
            preferred_versions: vec![rmcp::model::ProtocolVersion::default()],
            legacy_version: None,
        };
        let service = match &config.transport {
            Transport::Stdio {
                executable,
                arguments,
                directory,
                environment,
            } => {
                let mut command = tokio::process::Command::new(executable);
                command
                    .args(arguments)
                    .current_dir(directory)
                    .env_clear()
                    .envs(crate::commands::minimal_env(
                        Path::new(executable),
                        Path::new(directory),
                    ))
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null());
                for key in environment.keys() {
                    command.env(
                        key,
                        connection.credential.as_deref().ok_or_else(|| {
                            AppError::new(
                                "MCP_CREDENTIAL",
                                "Supply the referenced protected credential first.",
                            )
                        })?,
                    );
                }
                command.creation_flags(0x08000000);
                let mut command = CommandWrap::from(command);
                command.wrap(KillOnDrop).wrap(JobObject);
                let mut child = command.spawn().map_err(|_| {
                    AppError::new(
                        "MCP_START",
                        "The MCP executable could not start under Windows process supervision.",
                    )
                })?;
                let stdout = child.stdout().take().ok_or_else(protocol_error)?;
                let stdin = child.stdin().take().ok_or_else(protocol_error)?;
                connection.child = Some(child);
                let transport =
                    rmcp::transport::async_rw::AsyncRwTransport::<RoleClient, _, _>::new(
                        stdout.take(16 * 1024 * 1024),
                        stdin,
                    )
                    .with_max_line_length(1024 * 1024);
                tokio::time::timeout(
                    Duration::from_secs(20),
                    serve_client_with_lifecycle_and_ct((), transport, lifecycle, cancel.clone()),
                )
                .await
                .map_err(|_| protocol_error())?
                .map_err(|_| protocol_error())?
            }
            Transport::Http {
                url,
                local,
                headers,
            } => {
                let mut values = HashMap::new();
                for key in headers.keys() {
                    values.insert(
                        http::HeaderName::from_bytes(key.as_bytes())
                            .map_err(|_| protocol_error())?,
                        http::HeaderValue::from_str(connection.credential.as_deref().ok_or_else(
                            || {
                                AppError::new(
                                    "MCP_CREDENTIAL",
                                    "Supply the referenced protected credential first.",
                                )
                            },
                        )?)
                        .map_err(|_| protocol_error())?,
                    );
                }
                if config.preset == "github" {
                    values.insert(
                        http::HeaderName::from_static("x-mcp-readonly"),
                        http::HeaderValue::from_static("true"),
                    );
                    values.insert(
                        http::HeaderName::from_static("x-mcp-lockdown"),
                        http::HeaderValue::from_static("true"),
                    );
                    values.insert(
                        http::HeaderName::from_static("x-mcp-toolsets"),
                        http::HeaderValue::from_static("repos,issues,pull_requests,actions"),
                    );
                }
                let http = crate::mcp_http::Http::new(endpoint(url, *local)?, *local, values);
                let last_error = http.error.clone();
                connection.http_error = Some(last_error.clone());
                let mut transport = rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(url.clone());
                transport.reinit_on_expired_session = false;
                transport.max_concurrent_requests = 1;
                transport.max_sse_event_size = 1024 * 1024;
                if headers.is_empty() {
                    transport.auth_header = connection.credential.clone();
                }
                let transport =
                    rmcp::transport::StreamableHttpClientTransport::with_client(http, transport);
                tokio::time::timeout(
                    Duration::from_secs(20),
                    serve_client_with_lifecycle_and_ct((), transport, lifecycle, cancel.clone()),
                )
                .await
                .map_err(|_| AppError::new("MCP_TIMEOUT", "MCP connection timed out."))?
                .map_err(|_| {
                    last_error
                        .lock()
                        .unwrap()
                        .clone()
                        .unwrap_or_else(protocol_error)
                })?
            }
        };
        connection.protocol = service
            .peer_info()
            .map(|info| info.protocol_version.to_string())
            .unwrap_or_default();
        connection.service = Some(service);
        connection.discover(config).await?;
        Ok(connection)
    }

    pub async fn discover(&mut self, config: &Integration) -> Result<()> {
        self.tools.clear();
        self.withheld = 0;
        let service = self.service.as_ref().ok_or_else(protocol_error)?;
        let mut tools = Vec::new();
        let mut cursor = None;
        let mut seen = HashSet::new();
        for _ in 0..8 {
            let page = tokio::select! {
                _ = self.cancel.cancelled() => return Err(AppError::new("CANCELLED", "MCP connection stopped.")),
                result = tokio::time::timeout(Duration::from_secs(15), service.list_tools(cursor)) => result.map_err(|_| protocol_error())?.map_err(|_| protocol_error())?,
            };
            for tool in page.tools {
                if seen.len() >= 128 || tool.name.len() > 128 || !seen.insert(tool.name.to_string())
                {
                    return Err(AppError::new(
                        "MCP_DISCOVERY",
                        "MCP discovery exceeded limits or returned duplicate tool names.",
                    ));
                }
                let schema = Value::Object((*tool.input_schema).clone());
                if self.credential.as_ref().is_some_and(|secret| {
                    serde_json::to_value(&tool)
                        .is_ok_and(|value| contains_credential(&value, secret))
                }) {
                    self.withheld += 1;
                    continue;
                }
                if serde_json::to_vec(&schema)?.len() > 16384
                    || !check_schema(&schema, 0)
                    || jsonschema::validator_for(&schema).is_err()
                {
                    self.withheld += 1;
                    continue;
                }
                let name = tool.name.to_string();
                let fingerprint = hash(&serde_json::to_vec(&schema)?);
                let alias = format!(
                    "mcp_{}",
                    &hash(format!("{}:{name}", config.id).as_bytes())[..24]
                );
                let description = security::bounded(
                    &security::redact(tool.description.as_deref().unwrap_or(&name)),
                    240,
                );
                tools.push(ToolInfo {
                    effect: effect(config, &name),
                    name,
                    alias,
                    description,
                    schema,
                    fingerprint,
                });
            }
            if let Some(next) = page.next_cursor {
                cursor = Some(serde_json::from_value(json!({"cursor":next}))?);
            } else {
                self.tools = tools;
                return Ok(());
            }
        }
        Err(AppError::new(
            "MCP_DISCOVERY",
            "MCP discovery exceeded eight pages.",
        ))
    }
    pub async fn call(&self, tool: &ToolInfo, arguments: Value) -> Result<Value> {
        if serde_json::to_vec(&arguments)?.len() > 16384
            || !arguments.is_object()
            || !jsonschema::validator_for(&tool.schema)
                .map_err(|_| protocol_error())?
                .is_valid(&arguments)
        {
            return Err(AppError::new(
                "MCP_ARGUMENTS",
                "Arguments do not match the selected MCP tool schema.",
            ));
        }
        let params: CallToolRequestParams =
            serde_json::from_value(json!({"name":tool.name,"arguments":arguments}))?;
        let service = self.service.as_ref().ok_or_else(protocol_error)?;
        let result = tokio::select! {
            _ = self.cancel.cancelled() => return Err(AppError::new("CANCELLED", "MCP operation stopped.")),
            result = tokio::time::timeout(Duration::from_secs(30), service.call_tool_with_mrtr_max_rounds(params, 1)) => result.map_err(|_| AppError::new("MCP_TIMEOUT", "MCP call timed out. Its effects are unknown; it was not retried."))?.map_err(|_| self.failure())?,
        };
        let mut safe = serde_json::to_value(&result)?;
        if let Some(secret) = &self.credential {
            redact_credential(&mut safe, secret);
        }
        let text = serde_json::to_string(&safe)?;
        Ok(
            json!({"source":"untrusted_mcp_evidence","tool":tool.name,"isError":result.is_error.unwrap_or(false),"truncated":text.len()>12000,"evidence":security::bounded(&security::redact(&text),12000)}),
        )
    }
    pub async fn close(mut self) -> Result<()> {
        self.cancel.cancel();
        if let Some(service) = self.service.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), service.cancel()).await;
        }
        if let Some(mut child) = self.child.take() {
            tokio::time::timeout(Duration::from_secs(5), Box::into_pin(child.kill()))
                .await
                .map_err(|_| AppError::new("MCP_CLEANUP", "MCP process cleanup timed out."))?
                .map_err(|_| AppError::new("MCP_CLEANUP", "MCP process cleanup failed."))?;
        }
        Ok(())
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(child) = &mut self.child {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(mode: &str) -> Integration {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("fixtures/mcp/server.cjs");
        Integration {
            id: id(),
            name: "Synthetic tools".into(),
            preset: "custom".into(),
            enabled: true,
            credential: None,
            approved: true,
            transport: Transport::Stdio {
                executable: crate::commands::find_program("node.exe")
                    .expect("Node required for MCP fixtures"),
                arguments: vec![fixture.to_string_lossy().into(), mode.into()],
                directory: fixture.parent().unwrap().to_string_lossy().into(),
                environment: BTreeMap::from([("TEST_TOKEN".into(), "credential".into())]),
            },
        }
    }
    #[test]
    fn urls_schemas_and_annotations_do_not_grant_authority() {
        let credential = "fixture\"quoted\\token";
        let mut value = json!({credential:[format!("echo {credential}")]});
        assert!(contains_credential(&value, credential));
        redact_credential(&mut value, credential);
        assert!(!contains_credential(&value, credential));
        assert!(!serde_json::to_string(&value).unwrap().contains("quoted"));
        for url in [
            "http://example.com/mcp",
            "https://user:pass@example.com/mcp",
            "https://example.com/mcp?token=secret",
            "https://example.com:444/mcp",
        ] {
            assert!(endpoint(url, false).is_err());
        }
        for url in [
            "http://127.0.0.1:11434/mcp",
            "http://localhost:3001/mcp",
            "http://192.168.1.1/mcp",
        ] {
            assert!(endpoint(url, true).is_err());
        }
        assert!(endpoint("http://127.0.0.1:3001/mcp", true).is_ok());
        assert!(!check_schema(
            &json!({"$ref":"https://example.com/schema"}),
            0
        ));
        assert!(!check_schema(&json!({"pattern":"(a+)+"}), 0));
        assert_eq!(
            effect(&config("modern"), "read_file"),
            Effect::ExternalUnknown
        );
    }
    #[test]
    fn credentials_are_protected_and_configuration_requires_confirmation() {
        let folder = tempfile::tempdir().unwrap();
        let store = Store::open(folder.path()).unwrap();
        let config = config("modern");
        assert!(save(&store, config.clone(), None, false).is_err());
        let saved = save(
            &store,
            config,
            Some("synthetic-only-credential-abc".into()),
            true,
        )
        .unwrap();
        assert_eq!(
            secret(&store, &saved).unwrap().unwrap(),
            "synthetic-only-credential-abc"
        );
        assert!(!serde_json::to_string(&list(&store).unwrap())
            .unwrap()
            .contains("synthetic-only-credential-abc"));
        let encrypted =
            std::fs::read(folder.path().join("blobs").join(saved.credential.unwrap())).unwrap();
        assert!(!String::from_utf8_lossy(&encrypted).contains("synthetic-only-credential-abc"));
    }
    #[tokio::test]
    async fn stdio_modern_legacy_sanitization_and_schema_validation() {
        for mode in ["modern", "legacy"] {
            let cancel = CancellationToken::new();
            let connection = Connection::connect(
                &config(mode),
                Some("synthetic-secret-canary".into()),
                &cancel,
            )
            .await
            .unwrap();
            assert_eq!(connection.tools.len(), 1);
            assert_eq!(connection.tools[0].effect, Effect::ExternalUnknown);
            assert!(connection
                .call(&connection.tools[0], json!({"q":1}))
                .await
                .is_err());
            let result = connection
                .call(&connection.tools[0], json!({"q":"hello"}))
                .await
                .unwrap();
            assert_eq!(result["source"], "untrusted_mcp_evidence");
            assert!(result.to_string().contains("SYSTEM OVERRIDE"));
            assert!(!result.to_string().contains("synthetic-secret-canary"));
            connection.close().await.unwrap();
        }
    }
    #[tokio::test]
    async fn duplicate_discovery_crash_and_cancellation_fail_closed() {
        for mode in ["duplicate", "crash"] {
            assert!(Connection::connect(
                &config(mode),
                Some("fixture-only-secret-canary-987".into()),
                &CancellationToken::new()
            )
            .await
            .is_err());
        }
        let cancel = CancellationToken::new();
        let connection = Connection::connect(
            &config("modern"),
            Some("fixture-only-secret-canary-987".into()),
            &cancel,
        )
        .await
        .unwrap();
        cancel.cancel();
        assert!(connection
            .call(&connection.tools[0], json!({"q":"slow"}))
            .await
            .is_err());
        connection.close().await.unwrap();
    }
    #[tokio::test]
    async fn streamable_http_modern_and_legacy() {
        use tokio::io::AsyncBufReadExt;
        for mode in ["modern", "legacy", "sse"] {
            let mut config = config(mode);
            let Transport::Stdio {
                executable,
                arguments,
                ..
            } = &config.transport
            else {
                unreachable!()
            };
            let mut server = tokio::process::Command::new(executable)
                .args(arguments)
                .arg("http")
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let mut line = String::new();
            tokio::time::timeout(
                Duration::from_secs(5),
                tokio::io::BufReader::new(server.stdout.take().unwrap()).read_line(&mut line),
            )
            .await
            .unwrap()
            .unwrap();
            config.transport = Transport::Http {
                url: format!("http://127.0.0.1:{}/mcp", line.trim()),
                local: true,
                headers: BTreeMap::new(),
            };
            let connection = Connection::connect(&config, None, &CancellationToken::new())
                .await
                .unwrap();
            let result = connection
                .call(&connection.tools[0], json!({"q":"http"}))
                .await
                .unwrap();
            assert!(result.to_string().contains("http"));
            connection.close().await.unwrap();
            server.kill().await.unwrap();
        }
    }
    async fn http_fixture(mode: &str) -> (Integration, tokio::process::Child) {
        use tokio::io::AsyncBufReadExt;
        let mut config = config(mode);
        let Transport::Stdio {
            executable,
            arguments,
            ..
        } = &config.transport
        else {
            unreachable!()
        };
        let mut server = tokio::process::Command::new(executable)
            .args(arguments)
            .arg("http")
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        tokio::time::timeout(
            Duration::from_secs(5),
            tokio::io::BufReader::new(server.stdout.take().unwrap()).read_line(&mut line),
        )
        .await
        .unwrap()
        .unwrap();
        config.transport = Transport::Http {
            url: format!("http://127.0.0.1:{}/mcp", line.trim()),
            local: true,
            headers: BTreeMap::new(),
        };
        (config, server)
    }
    #[tokio::test]
    async fn http_auth_rate_redirect_invalid_data_and_oversize_fail_visibly() {
        for (mode, code) in [
            ("auth", "MCP_AUTH"),
            ("rate", "MCP_RATE_LIMIT"),
            ("redirect", "MCP_REDIRECT"),
            ("malformed", "MCP_PROTOCOL"),
            ("unknown", "MCP_PROTOCOL"),
        ] {
            let (config, mut server) = http_fixture(mode).await;
            let error = Connection::connect(&config, None, &CancellationToken::new())
                .await
                .err()
                .expect("must fail closed");
            assert_eq!(error.code, code, "{mode}");
            server.kill().await.unwrap();
        }
        for mode in ["modern", "sse"] {
            let (config, mut server) = http_fixture(mode).await;
            let connection = Connection::connect(&config, None, &CancellationToken::new())
                .await
                .unwrap();
            assert!(connection
                .call(&connection.tools[0], json!({"q":"huge"}))
                .await
                .is_err());
            connection.close().await.unwrap();
            server.kill().await.unwrap();
        }
    }
    #[tokio::test]
    async fn changed_and_withheld_schemas_reconnect_and_name_collisions() {
        for mode in ["many", "unknown"] {
            assert!(Connection::connect(
                &config(mode),
                Some("canary-value-8267".into()),
                &CancellationToken::new()
            )
            .await
            .is_err());
        }
        let config = config("changed");
        let mut connection = Connection::connect(
            &config,
            Some("canary-value-8267".into()),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let old = connection.tools[0].fingerprint.clone();
        connection.discover(&config).await.unwrap();
        assert_ne!(old, connection.tools[0].fingerprint);
        connection.close().await.unwrap();
        let connection = Connection::connect(
            &self::config("invalid-schema"),
            Some("canary-value-8267".into()),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(connection.tools.is_empty());
        assert_eq!(connection.withheld, 1);
        connection.close().await.unwrap();
        let first = Connection::connect(
            &self::config("modern"),
            Some("canary-value-8267".into()),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let second = Connection::connect(
            &self::config("modern"),
            Some("canary-value-8267".into()),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_ne!(first.tools[0].alias, second.tools[0].alias);
        first.close().await.unwrap();
        second.close().await.unwrap();
    }
    #[tokio::test]
    async fn close_and_drop_kill_descendants_and_cancel_slow_start() {
        for explicit in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let port_file = temp.path().join("port.txt");
            let connection = Connection::connect(
                &config("modern"),
                Some("canary-value-8267".into()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
            connection
                .call(
                    &connection.tools[0],
                    json!({"q":format!("spawn:{}",port_file.display())}),
                )
                .await
                .unwrap();
            let port = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Ok(port) = std::fs::read_to_string(&port_file) {
                        if let Ok(port) = port.parse::<u16>() {
                            break port;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            assert!(tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok());
            if explicit {
                connection.close().await.unwrap();
            } else {
                drop(connection);
            }
            tokio::time::timeout(Duration::from_secs(5), async {
                while tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .is_ok()
                {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
        }
        let cancel = CancellationToken::new();
        let stop = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            stop.cancel();
        });
        assert!(tokio::time::timeout(
            Duration::from_secs(2),
            Connection::connect(
                &config("slow-start"),
                Some("canary-value-8267".into()),
                &cancel
            )
        )
        .await
        .unwrap()
        .is_err());
    }
    #[tokio::test]
    async fn tool_timeout_and_inflight_cancellation_do_not_retry() {
        let cancel = CancellationToken::new();
        let connection = Connection::connect(
            &config("modern"),
            Some("fixture-canary-526".into()),
            &cancel,
        )
        .await
        .unwrap();
        let began = std::time::Instant::now();
        let error = connection
            .call(&connection.tools[0], json!({"q":"slow"}))
            .await
            .unwrap_err();
        assert_eq!(error.code, "MCP_TIMEOUT");
        assert!(began.elapsed() < Duration::from_secs(35));
        let result = connection
            .call(&connection.tools[0], json!({"q":"after-timeout"}))
            .await
            .unwrap();
        assert!(
            result.to_string().contains("calls=2"),
            "Timed out action was repeated"
        );
        let stop = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            stop.cancel();
        });
        let error = connection
            .call(&connection.tools[0], json!({"q":"slow"}))
            .await
            .unwrap_err();
        assert_eq!(error.code, "CANCELLED");
        connection.close().await.unwrap();
    }
}
