use local_agent_core::{
    browser::{self, BrowserSpec, BrowserStep, Component},
    commands,
    domain::*,
    store::Store,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn prepare(store: &Store) -> bool {
    let Some(node) = commands::find_program("node.exe") else {
        return false;
    };
    let chromium = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".local-data/browsers/chromium-1248/chrome-win64/chrome.exe");
    if !chromium.is_file() {
        return false;
    }
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("browser-helper/main.cjs");
    let component = Component {
        node_hash: hash(&std::fs::read(&node).unwrap()),
        node,
        helper_hash: hash(&std::fs::read(&helper).unwrap()),
        helper: helper.to_string_lossy().into(),
        chromium_hash: hash(&std::fs::read(&chromium).unwrap()),
        chromium: chromium.to_string_lossy().into(),
    };
    store.put("browser", "component", &component).unwrap();
    true
}
#[tokio::test]
async fn local_interactions_use_broker_and_block_external_request() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    if !prepare(&store) {
        eprintln!("SKIP: test requires installed Node and Chrome");
        return;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(Mutex::new(Vec::new()));
    let recorded = hits.clone();
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let recorded = recorded.clone();
            tokio::spawn(async move {
                let mut bytes = [0u8; 16384];
                let n = stream.read(&mut bytes).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&bytes[..n]);
                let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
                recorded.lock().unwrap().push(path.clone());
                let (status, extra, body) = if path == "/redirect" {
                    ("302 Found", "Location: /\r\n", "")
                } else {
                    (
                        "200 OK",
                        "",
                        r#"<!doctype html><html><head><title>Fixture</title></head><body><label>Name <input aria-label="Name"></label><button onclick="document.querySelector('p').textContent='Hello '+document.querySelector('input').value">Greet</button><p>Ready</p><script>fetch('http://127.0.0.1:11434/should-never-arrive').catch(()=>{})</script></body></html>"#,
                    )
                };
                let response=format!("HTTP/1.1 {status}\r\n{extra}Content-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    let spec = BrowserSpec {
        url: format!("http://{address}/redirect"),
        profile: "local_test".into(),
        steps: vec![
            BrowserStep {
                action: "fill".into(),
                role: "textbox".into(),
                name: "Name".into(),
                value: "Ada".into(),
            },
            BrowserStep {
                action: "click".into(),
                role: "button".into(),
                name: "Greet".into(),
                value: String::new(),
            },
            BrowserStep {
                action: "assert_text".into(),
                role: String::new(),
                name: String::new(),
                value: "Hello Ada".into(),
            },
        ],
    };
    let result = browser::run(
        &store,
        &id(),
        &spec,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    server.abort();
    assert_eq!(result["status"], "passed", "{result:#}");
    assert!(result["text"].as_str().unwrap().contains("Hello Ada"));
    assert!(result["blockedRequests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap_or("").contains("outside")));
    assert!(hits.lock().unwrap().contains(&"/redirect".into()));
    assert!(hits.lock().unwrap().contains(&"/".into()));
}
#[tokio::test]
async fn public_profile_cannot_send_http_to_private_network() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    if !prepare(&store) {
        return;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let spec = BrowserSpec {
        url: format!("http://{}/", listener.local_addr().unwrap()),
        profile: "public_read".into(),
        steps: vec![],
    };
    let result = browser::run(
        &store,
        &id(),
        &spec,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result["status"], "failed");
    if let Ok(Ok((mut stream, _))) =
        tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept()).await
    {
        let mut buf = [0u8; 512];
        if let Ok(Ok(bytes)) =
            tokio::time::timeout(std::time::Duration::from_millis(100), stream.read(&mut buf)).await
        {
            assert_eq!(
                bytes,
                0,
                "Private HTTP bytes escaped: {}",
                String::from_utf8_lossy(&buf[..bytes])
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires an external HTTPS connection and matching Chromium"]
async fn approved_public_static_page_is_readable() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    assert!(prepare(&store), "Install the pinned test Chromium first");
    let spec = BrowserSpec {
        url: "https://example.com/".into(),
        profile: "public_read".into(),
        steps: vec![BrowserStep {
            action: "assert_title".into(),
            role: String::new(),
            name: String::new(),
            value: "Example Domain".into(),
        }],
    };
    let result = browser::run(
        &store,
        &id(),
        &spec,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result["status"], "passed", "{result:#}");
    assert!(result["text"].as_str().unwrap().len() > 20, "{result:#}");
}
