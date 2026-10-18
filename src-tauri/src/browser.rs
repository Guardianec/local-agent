use crate::{
    commands::{self, CommandPlan},
    domain::*,
    security,
    store::Store,
};
use base64::Engine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::ServerOptions,
};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

#[derive(Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/generated/")]
pub struct BrowserSpec {
    pub url: String,
    pub profile: String,
    pub steps: Vec<BrowserStep>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/generated/")]
pub struct BrowserStep {
    pub action: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Component {
    pub node: String,
    pub node_hash: String,
    pub helper: String,
    pub helper_hash: String,
    pub chromium: String,
    pub chromium_hash: String,
}

pub fn validate(spec: &BrowserSpec) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(&spec.url)
        .map_err(|_| AppError::new("BROWSER_URL", "Enter an HTTP or HTTPS page address."))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || spec.url.len() > 2048
        || security::has_secret(&spec.url)
    {
        return Err(AppError::new(
            "BROWSER_URL",
            "Credentials, sensitive query values and non-web addresses are blocked.",
        ));
    }
    if !matches!(spec.profile.as_str(), "public_read" | "local_test") || spec.steps.len() > 10 {
        return Err(AppError::new(
            "BROWSER_SCOPE",
            "Choose a supported profile with at most 10 steps.",
        ));
    }
    let ip = url.host_str().and_then(|h| h.parse::<IpAddr>().ok());
    if spec.profile == "local_test"
        && !(url.host_str() == Some("localhost") || ip.is_some_and(|a| a.is_loopback()))
    {
        return Err(AppError::new(
            "BROWSER_ORIGIN",
            "Local tests must use an explicitly approved loopback server.",
        ));
    }
    if spec.profile == "local_test"
        && matches!(
            url.port_or_known_default(),
            Some(11434 | 9222 | 9223 | 2375 | 2376)
        )
    {
        return Err(AppError::new(
            "BROWSER_ORIGIN",
            "Model, debugger and administrative service ports are blocked.",
        ));
    }
    for step in &spec.steps {
        if !matches!(
            step.action.as_str(),
            "assert_text" | "assert_title" | "click" | "fill" | "select" | "check"
        ) || step.name.len() > 256
            || step.value.len() > 2000
            || security::has_secret(&step.value)
        {
            return Err(AppError::new(
                "BROWSER_ACTION",
                "This browser step is unsupported or contains sensitive input.",
            ));
        }
        if spec.profile == "public_read" && !step.action.starts_with("assert_") {
            return Err(AppError::new(
                "BROWSER_READ_ONLY",
                "Public pages support reading and assertions only.",
            ));
        }
        if !step.action.starts_with("assert_")
            && !matches!(
                step.role.as_str(),
                "button" | "link" | "textbox" | "checkbox" | "combobox"
            )
        {
            return Err(AppError::new(
                "BROWSER_TARGET",
                "Use a supported accessible control role.",
            ));
        }
    }
    Ok(url)
}
pub fn component(store: &Store) -> Result<Component> {
    let c: Component = store.get("browser", "component").map_err(|_| {
        AppError::new(
            "BROWSER_SETUP",
            "Install the optional browser tools in Settings first.",
        )
    })?;
    for (path, digest) in [
        (&c.node, &c.node_hash),
        (&c.helper, &c.helper_hash),
        (&c.chromium, &c.chromium_hash),
    ] {
        if hash(&std::fs::read(path)?) != *digest {
            return Err(AppError::new(
                "BROWSER_COMPONENT_CHANGED",
                "Browser tools changed on disk. Reinstall them before use.",
            ));
        }
    }
    Ok(c)
}
pub fn install(store: &Store, source: &Path, cancel: CancellationToken) -> Result<Component> {
    let settings = store.settings();
    let node = settings
        .node_path
        .or_else(|| commands::find_program("node.exe"))
        .ok_or_else(|| {
            AppError::new(
                "NODE_MISSING",
                "Choose Node.js in Settings before installing browser tools.",
            )
        })?;
    let root = store.root.join("browser-component");
    std::fs::create_dir_all(root.join("home"))?;
    let app_node = root.join("node.exe");
    if !app_node.exists() {
        std::fs::copy(&node, &app_node)?;
    }
    let helper = source.join("main.cjs");
    if !helper.is_file() || !source.join("node_modules/playwright-core/cli.js").is_file() {
        return Err(AppError::new("BROWSER_PAYLOAD","Browser helper resources are missing. Rebuild using the documented browser dependency step."));
    }
    let mut environment = commands::minimal_env(&app_node, &root.join("home"));
    environment.insert(
        "PLAYWRIGHT_BROWSERS_PATH".into(),
        root.join("browsers").to_string_lossy().into(),
    );
    let plan = CommandPlan {
        executable: app_node.clone(),
        args: vec![
            source
                .join("node_modules/playwright-core/cli.js")
                .to_string_lossy()
                .into(),
            "install".into(),
            "chromium".into(),
            "--no-shell".into(),
        ],
        directory: root.clone(),
        environment: environment.clone(),
        timeout: Duration::from_secs(600),
    };
    let result = commands::execute(plan, cancel.clone())?;
    if result.code != Some(0) {
        return Err(AppError::new(
            "BROWSER_INSTALL",
            "Browser installation did not finish. You can retry; coding tools remain available.",
        )
        .detail(result.output));
    }
    let where_plan = CommandPlan {
        executable: app_node.clone(),
        args: vec![helper.to_string_lossy().into(), "--where".into()],
        directory: root,
        environment,
        timeout: Duration::from_secs(10),
    };
    let result = commands::execute(where_plan, cancel)?;
    let chromium = PathBuf::from(result.output.trim());
    if !chromium.is_file() {
        return Err(AppError::new(
            "BROWSER_INSTALL",
            "The downloaded browser could not be located.",
        ));
    }
    let c = Component {
        node: app_node.to_string_lossy().into(),
        node_hash: hash(&std::fs::read(&app_node)?),
        helper: helper.to_string_lossy().into(),
        helper_hash: hash(&std::fs::read(&helper)?),
        chromium: chromium.to_string_lossy().into(),
        chromium_hash: hash(&std::fs::read(&chromium)?),
    };
    store.put("browser", "component", &c)?;
    Ok(c)
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !a.is_private()
                && !a.is_loopback()
                && !a.is_link_local()
                && !a.is_multicast()
                && !a.is_unspecified()
                && o[0] != 0
                && o[0] < 224
                && o[0] != 127
                && !(o[0] == 100 && (64..=127).contains(&o[1]))
                && !(o[0] == 192 && o[1] == 0)
                && !(o[0] == 192 && o[1] == 88 && o[2] == 99)
                && !(o[0] == 198 && (18..=19).contains(&o[1]))
                && !(o[0] == 198 && o[1] == 51 && o[2] == 100)
                && !(o[0] == 203 && o[1] == 0 && o[2] == 113)
                && a != std::net::Ipv4Addr::BROADCAST
        }
        IpAddr::V6(a) => a
            .to_ipv4_mapped()
            .map(|a| public_ip(IpAddr::V4(a)))
            .unwrap_or_else(|| {
                // Conservative global-unicast scope; exclude special transition,
                // benchmarking, documentation and ORCHID prefixes as well.
                let s = a.segments();
                (s[0] & 0xe000) == 0x2000
                    && !(s[0] == 0x2001 && s[1] < 0x200)
                    && !(s[0] == 0x2001 && s[1] == 0xdb8)
                    && s[0] != 0x2002
                    && !(s[0] == 0x3fff && s[1] < 0x1000)
            }),
    }
}

pub async fn run(
    store: &Store,
    task_id: &str,
    spec: &BrowserSpec,
    cancel: CancellationToken,
) -> Result<Value> {
    let url = validate(spec)?;
    let component = component(store)?;
    let origin = url.origin().ascii_serialization();
    let nonce = id();
    let pipe = format!(r"\\.\pipe\local-agent-{}", id());
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&pipe)?;
    let folder = store.root.join("tasks").join(task_id).join("browser");
    std::fs::create_dir_all(folder.join("home"))?;
    let input = folder.join(format!("{}.json", id()));
    std::fs::write(
        &input,
        serde_json::to_vec(
            &json!({"pipe":pipe,"nonce":nonce,"chromium":component.chromium,"spec":spec}),
        )?,
    )?;
    let mut env = commands::minimal_env(Path::new(&component.node), &folder.join("home"));
    env.insert(
        "PLAYWRIGHT_BROWSERS_PATH".into(),
        store
            .root
            .join("browser-component/browsers")
            .to_string_lossy()
            .into(),
    );
    let plan = CommandPlan {
        executable: PathBuf::from(&component.node),
        args: vec![component.helper, input.to_string_lossy().into()],
        directory: folder,
        environment: env,
        timeout: Duration::from_secs(120),
    };
    let run_cancel = cancel.child_token();
    let broker_cancel = run_cancel.child_token();
    let token = run_cancel.clone();
    let mut child = tokio::task::spawn_blocking(move || commands::execute(plan, token));
    let spec = spec.clone();
    let broker = async {
        tokio::select! {_=broker_cancel.cancelled()=>return Err(AppError::new("CANCELLED","Browser check stopped.")),r=tokio::time::timeout(Duration::from_secs(15),server.connect())=>r.map_err(|_|AppError::new("BROWSER_CONNECT","The browser helper did not connect."))??};
        let (reader, mut writer) = tokio::io::split(server);
        let mut reader = BufReader::new(reader);
        let mut count = 0usize;
        let mut total = 0usize;
        loop {
            let mut line = String::new();
            let read = tokio::select! {_=broker_cancel.cancelled()=>break,r=reader.read_line(&mut line)=>r?};
            if read == 0 {
                break;
            }
            if line.len() > 128 * 1024 {
                return Err(AppError::new(
                    "BROWSER_FRAME",
                    "Browser request exceeded the transport limit.",
                ));
            }
            count += 1;
            if count > 100 {
                return Err(AppError::new(
                    "BROWSER_LIMIT",
                    "Browser request count exceeded its limit.",
                ));
            }
            let request: Value = serde_json::from_str(&line)?;
            if request["nonce"] != nonce {
                return Err(AppError::new(
                    "BROWSER_NONCE",
                    "An unauthorized browser connection was refused.",
                ));
            }
            let response = match fetch(&request, &origin, &spec, &broker_cancel).await {
                Ok(v) => v,
                Err(e) => json!({"error":e.message}),
            };
            total += response["body"].as_str().map_or(0, str::len);
            if total > 14 * 1024 * 1024 {
                return Err(AppError::new(
                    "BROWSER_LIMIT",
                    "Browser data exceeded its session limit.",
                ));
            }
            let mut response = response;
            response["id"] = request["id"].clone();
            let mut bytes = serde_json::to_vec(&response)?;
            bytes.push(b'\n');
            writer.write_all(&bytes).await?;
        }
        Ok(())
    };
    tokio::pin!(broker);
    let result = tokio::select! {
        done=&mut child=>{broker_cancel.cancel();done.map_err(|e|AppError::new("BROWSER_WORKER","The browser worker stopped.").detail(e))??},
        done=&mut broker=>{if let Err(e)=done{run_cancel.cancel();let _=child.await;return Err(e);}child.await.map_err(|e|AppError::new("BROWSER_WORKER","The browser worker stopped.").detail(e))??}
    };
    let value: Value = serde_json::from_str(result.output.trim()).map_err(|_| {
        AppError::new("BROWSER_FAILED", "The browser could not finish its check.")
            .detail(security::bounded(&result.output, 2000))
    })?;
    Ok(value)
}
async fn fetch(
    request: &Value,
    origin: &str,
    spec: &BrowserSpec,
    cancel: &CancellationToken,
) -> Result<Value> {
    let raw = request["url"]
        .as_str()
        .ok_or_else(|| AppError::new("BROWSER_URL", "Missing browser address."))?;
    let url = reqwest::Url::parse(raw)
        .map_err(|_| AppError::new("BROWSER_URL", "Invalid page address."))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.origin().ascii_serialization() != origin
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AppError::new(
            "BROWSER_ORIGIN",
            "A request outside the approved page origin was blocked.",
        ));
    }
    let method = request["method"].as_str().unwrap_or("GET");
    if !matches!(method, "GET" | "HEAD") && spec.profile != "local_test" {
        return Err(AppError::new(
            "BROWSER_EFFECT",
            "Public-page submissions are blocked.",
        ));
    }
    if !matches!(
        method,
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    ) {
        return Err(AppError::new(
            "BROWSER_METHOD",
            "This request method is not supported.",
        ));
    }
    let body = request["body"].as_str().unwrap_or("");
    if body.len() > 64 * 1024 || security::has_secret(body) || security::has_secret(raw) {
        return Err(AppError::new(
            "BROWSER_SECRET",
            "Sensitive or oversized browser data was blocked.",
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| AppError::new("BROWSER_HOST", "Missing host."))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host, port)),
    )
    .await
    .map_err(|_| AppError::new("BROWSER_DNS", "The page host did not resolve."))??
    .collect::<Vec<_>>();
    let allowed = |a: &SocketAddr| {
        if spec.profile == "local_test" {
            a.ip().is_loopback()
        } else {
            public_ip(a.ip())
        }
    };
    if addresses.is_empty() || addresses.iter().any(|a| !allowed(a)) {
        return Err(AppError::new(
            "BROWSER_ADDRESS",
            "Private, reserved or unapproved network destinations are blocked.",
        ));
    }
    let pinned = *addresses
        .iter()
        .find(|a| a.is_ipv4())
        .unwrap_or(&addresses[0]);
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(host, pinned)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| {
            AppError::new("BROWSER_NETWORK", "Could not prepare page request.").detail(e)
        })?;
    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|_| AppError::new("BROWSER_METHOD", "Invalid method."))?,
            url.clone(),
        )
        .body(body.to_string());
    if let Some(v) = request["headers"]["content-type"].as_str() {
        if !v.is_empty() {
            req = req.header("content-type", v);
        }
    }
    let mut response = tokio::select! {_=cancel.cancelled()=>return Err(AppError::new("CANCELLED","Browser check stopped.")),r=req.send()=>r.map_err(|e|AppError::new("BROWSER_NETWORK","The page request failed.").detail(e))?};
    if response.status().is_redirection() {
        let target = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| url.join(v).ok())
            .ok_or_else(|| {
                AppError::new(
                    "BROWSER_REDIRECT",
                    "The page returned an unsupported redirect.",
                )
            })?;
        if request["document"] != true || target.origin().ascii_serialization() != origin {
            return Err(AppError::new("BROWSER_REDIRECT","Subresource or cross-origin redirect blocked. Open the final address in a new approved check."));
        }
        return Ok(json!({"redirect":target.as_str()}));
    }
    let status = response.status().as_u16();
    let mut headers = BTreeMap::new();
    for key in [
        "content-type",
        "content-security-policy",
        "access-control-allow-origin",
        "access-control-allow-methods",
        "access-control-allow-headers",
        "x-content-type-options",
    ] {
        if let Some(v) = response.headers().get(key).and_then(|v| v.to_str().ok()) {
            headers.insert(key, v.to_string());
        }
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = tokio::select! {_=cancel.cancelled()=>return Err(AppError::new("CANCELLED","Browser check stopped.")),r=response.chunk()=>r.map_err(|e|AppError::new("BROWSER_NETWORK","The page response stopped early.").detail(e))?}
    {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > 512 * 1024 {
            return Err(AppError::new(
                "BROWSER_RESOURCE_LIMIT",
                "A page resource exceeded the 512 KiB limit.",
            ));
        }
    }
    Ok(
        json!({"status":status,"headers":headers,"body":base64::engine::general_purpose::STANDARD.encode(bytes)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_sensitive_origins_and_actions() {
        for url in [
            "file:///C:/secret",
            "http://127.0.0.1:11434",
            "http://localhost:9223",
        ] {
            assert!(validate(&BrowserSpec {
                url: url.into(),
                profile: "local_test".into(),
                steps: vec![]
            })
            .is_err());
        }
        assert!(!public_ip("127.0.0.1".parse().unwrap()));
        assert!(!public_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!public_ip("169.254.169.254".parse().unwrap()));
        assert!(public_ip("1.1.1.1".parse().unwrap()));
        for ip in [
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "2001:db8::1",
            "2002:7f00:1::1",
            "64:ff9b::7f00:1",
            "3fff::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
