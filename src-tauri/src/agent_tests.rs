use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn mock_runtime(
    handler: Arc<dyn Fn(Value, usize) -> Value + Send + Sync>,
) -> (String, tokio::task::JoinHandle<()>, Arc<AtomicUsize>) {
    let (url, server, count, _) = mock_runtime_with_log(handler).await;
    (url, server, count)
}
type RequestLog = Arc<Mutex<Vec<Value>>>;
#[tokio::test]
async fn preexisting_same_model_is_preserved_as_another_clients_residency() {
    let (endpoint, server, count, log) = mock_runtime_controlled(
        Arc::new(|_, _| panic!("Existing client must be preserved")),
        false,
        false,
        true,
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "Fix addition in math.cjs",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &t.id, false).await;
    assert_eq!(done.state, "failed");
    assert!(done.summary.contains("Another client"));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(!log
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["path"] == "/api/generate"));
    server.abort();
}
#[test]
fn deterministic_rename_preserves_bytes_and_rejects_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = ConfinedRoot::open(tmp.path()).unwrap();
    let bytes = b"\xef\xbb\xbfFirst line\r\nSecond line\r\n";
    root.write_new("one.txt", bytes).unwrap();
    let args = tools::RenameArgs {
        path: "one.txt".into(),
        destination: "nested/two.txt".into(),
        expected_hash: hash(bytes),
    };
    tool_execution::rename_exact(&root, &args).unwrap();
    assert!(root.read("one.txt", 1024).is_err());
    assert_eq!(root.read("nested/two.txt", 1024).unwrap(), bytes);
    root.write_new("source.txt", b"source\n").unwrap();
    let mut collision = tools::RenameArgs {
        path: "source.txt".into(),
        destination: "nested/two.txt".into(),
        expected_hash: hash(b"source\n"),
    };
    assert!(tool_execution::rename_exact(&root, &collision).is_err());
    assert_eq!(root.read("source.txt", 1024).unwrap(), b"source\n");
    assert_eq!(root.read("nested/two.txt", 1024).unwrap(), bytes);
    collision.destination = "new.txt".into();
    collision.expected_hash = "stale".into();
    assert!(tool_execution::rename_exact(&root, &collision).is_err());
    assert!(root.read("new.txt", 1024).is_err());
    collision.destination = "../outside.txt".into();
    assert!(tool_execution::rename_exact(&root, &collision).is_err());
}

#[tokio::test]
async fn rename_cannot_bypass_observe_or_denied_approval() {
    for mode in [Mode::Observe, Mode::Project] {
        let(endpoint,server,_)=mock_runtime(Arc::new(|_,index|match index{0=>tool("read_file",json!({"path":"math.cjs"})),1=>tool("rename_file",json!({"path":"math.cjs","destination":"renamed.cjs","expectedHash":hash(b"module.exports.add = (a,b) => a-b;\n")})),_=>tool("finish_task",json!({"summary":"Stopped","unresolved":[]}))})).await;
        let tmp = tempfile::tempdir().unwrap();
        let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
        Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
        configure_continuation_fixture(&core);
        let p = project(&core, &tmp.path().join("project"));
        let t = core
            .start(
                &p.id,
                "Rename math.cjs",
                "task",
                Some("@profile/balanced".into()),
                mode,
                None,
            )
            .unwrap();
        let done = finish(&core, &t.id, false).await;
        let work =
            snapshots::working_path(&core.store, done.snapshot_id.as_deref().unwrap()).unwrap();
        assert!(work.join("math.cjs").exists());
        assert!(!work.join("renamed.cjs").exists());
        assert_eq!(done.change_state, "none");
        server.abort();
    }
}
#[tokio::test]
async fn model_install_error_and_cancel_release_the_controller() {
    for cancel in [false, true] {
        let (endpoint, server, _, log) = mock_runtime_controlled(
            Arc::new(|_, _| panic!("No inference during install")),
            cancel,
            false,
            false,
        )
        .await;
        let tmp = tempfile::tempdir().unwrap();
        let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
        Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
        let operation_core = core.clone();
        let operation = tokio::spawn(async move { operation_core.install_model("balanced").await });
        if cancel {
            for _ in 0..200 {
                if log.lock().unwrap().iter().any(|r| r["path"] == "/api/pull") {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            core.cancel("model-install").unwrap();
        }
        let result = operation.await.unwrap();
        assert!(result.is_err());
        if cancel {
            assert_eq!(result.unwrap_err().code, "CANCELLED");
        }
        assert!(!core.busy());
        let progress: ModelInstallProgress = core.store.get("services", "model-install").unwrap();
        assert!(progress.done);
        assert!(progress.error.is_some());
        server.abort();
    }
}
async fn mock_runtime_with_log(
    handler: Arc<dyn Fn(Value, usize) -> Value + Send + Sync>,
) -> (
    String,
    tokio::task::JoinHandle<()>,
    Arc<AtomicUsize>,
    RequestLog,
) {
    mock_runtime_controlled(handler, false, false, false).await
}
async fn mock_runtime_controlled(
    handler: Arc<dyn Fn(Value, usize) -> Value + Send + Sync>,
    slow_load: bool,
    fail_load: bool,
    resident: bool,
) -> (
    String,
    tokio::task::JoinHandle<()>,
    Arc<AtomicUsize>,
    RequestLog,
) {
    let log: RequestLog = Arc::new(Mutex::new(vec![]));
    let recorded = log.clone();
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let requests = count.clone();
    let runner = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = server.accept().await else {
                break;
            };
            let handler = handler.clone();
            let requests = requests.clone();
            let recorded = recorded.clone();
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                let mut buf = [0u8; 8192];
                let (header_end, len) = loop {
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let len = headers
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        break (end + 4, len);
                    }
                };
                while bytes.len() < header_end + len {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                }
                let path = String::from_utf8_lossy(&bytes[..header_end])
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                let request_body: Value =
                    serde_json::from_slice(&bytes[header_end..]).unwrap_or(Value::Null);
                recorded
                    .lock()
                    .unwrap()
                    .push(json!({"path":path,"body":request_body}));
                let loading = path == "/api/generate" && request_body.get("options").is_some();
                if (loading || path == "/api/pull") && slow_load {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                let status = if loading && fail_load {
                    "500 Internal Server Error"
                } else {
                    "200 OK"
                };
                let value = match path.as_str() {
                    "/api/generate" => json!({"done":true}),
                    "/api/version" => json!({"version":"fixture-v1"}),
                    "/api/tags" => {
                        let models = [("fixture:8b","fixture-digest"),("fixture:strong","strong-digest")].into_iter().map(|(name,digest)|json!({"name":name,"digest":digest,"size":100000,"details":{"parameter_size":"8B","quantization_level":"Q4"}})).collect::<Vec<_>>();
                        json!({"models":models})
                    }
                    "/api/ps" => {
                        if resident {
                            json!({"models":[{"name":"fixture:8b","size":100000,"size_vram":100000}]})
                        } else {
                            json!({"models":[]})
                        }
                    }
                    "/api/show" => {
                        json!({"details":{"parameter_size":"8B"},"capabilities":["tools","vision"]})
                    }
                    "/api/chat" => {
                        let body = serde_json::from_slice(&bytes[header_end..]).unwrap();
                        handler(body, requests.fetch_add(1, Ordering::SeqCst))
                    }
                    _ => json!({}),
                };
                let body = format!("{}\n", serde_json::to_string(&value).unwrap());
                let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (format!("http://{address}"), runner, count, log)
}
fn tool(name: &str, args: Value) -> Value {
    json!({"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":name,"arguments":args}}]},"done":true,"eval_count":10})
}

fn configure_continuation_fixture(core: &AppCore) {
    for digest in ["fixture-digest", "strong-digest"] {
        core.store
            .put(
                "probes",
                &format!("{digest}:fixture-v1"),
                &json!({"status":"verified"}),
            )
            .unwrap();
    }
    let mut s = core.store.settings();
    s.model_profiles = ModelProfiles {
        fast: None,
        balanced: Some("fixture:8b".into()),
        strong: Some("fixture:strong".into()),
        strong_enabled: true,
    };
    core.store.put("settings", "main", &s).unwrap();
    core.store
        .put(
            "calibrations",
            "strong",
            &ModelCalibration {
                model: "fixture:strong".into(),
                digest: "strong-digest".into(),
                runtime: "fixture-v1".into(),
                context_size: 32768,
                passed: true,
                full_gpu: Some(false),
                elapsed_ms: 1.0,
                tokens_per_second: None,
                allocation_bytes: None,
                free_gpu_bytes: None,
                message: String::new(),
                evidence: String::new(),
                tested_at: now(),
                peak_ram_bytes: None,
                minimum_free_ram_bytes: None,
                time_to_first_token_ms: None,
                retries: 0,
                escalations: 0,
            },
        )
        .unwrap();
}

#[tokio::test]
async fn cancellation_during_load_releases_model_without_tools() {
    let (endpoint, server, count, log) = mock_runtime_controlled(
        Arc::new(|_, _| panic!("Inference must not start")),
        true,
        false,
        false,
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "Fix addition in math.cjs",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    for _ in 0..200 {
        if log
            .lock()
            .unwrap()
            .iter()
            .any(|r| r["path"] == "/api/generate" && r["body"].get("options").is_some())
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    core.cancel(&t.id).unwrap();
    let done = finish(&core, &t.id, false).await;
    assert_eq!(done.state, "cancelled");
    assert_eq!(done.tool_calls, 0);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(log
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["body"]["keep_alive"] == 0));
    assert!(core.owned_model.lock().await.is_none());
    server.abort();
}

#[tokio::test]
async fn failed_load_retains_task_and_never_executes_inference() {
    let (endpoint, server, count, _) = mock_runtime_controlled(
        Arc::new(|_, _| panic!("Inference must not start")),
        false,
        true,
        false,
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "Fix addition in math.cjs",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &t.id, false).await;
    assert_eq!(done.state, "failed");
    assert!(done.snapshot_id.is_some());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(!core.busy());
    server.abort();
}

#[tokio::test]
async fn disabling_escalation_preserves_evidence_without_loading_strong() {
    let (endpoint, server, _, log) = mock_runtime_with_log(Arc::new(|_, _| {
        tool("run_project_command", json!({"recipeId":"npm:test"}))
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let mut settings = core.store.settings();
    settings.intelligence.automatic_escalation = false;
    core.store.put("settings", "main", &settings).unwrap();
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "Fix addition in math.cjs",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &t.id, true).await;
    assert_eq!(done.state, "escalation_ready");
    assert_eq!(done.stages.len(), 1);
    assert!(done.continuation.is_some());
    assert!(!log
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["body"]["model"] == "fixture:strong"
            && matches!(r["path"].as_str(), Some("/api/chat" | "/api/generate"))));
    server.abort();
}

#[tokio::test]
async fn nonvision_model_never_receives_attached_image() {
    let (endpoint, server, count) =
        mock_runtime(Arc::new(|_, _| panic!("Image must not be sent"))).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    core.store
        .put(
            "model_metadata",
            "fixture-digest:fixture-v1",
            &json!({"vision":false,"thinking":false}),
        )
        .unwrap();
    let p = project(&core, &tmp.path().join("project"));
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(10, 10)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let image = crate::images::ingest(&core.store, "UI.png", png.get_ref(), "file").unwrap();
    let t = core
        .start_with_inputs(
            &p.id,
            "Describe this screenshot",
            "explain",
            Some("@profile/balanced".into()),
            Mode::Observe,
            None,
            vec![ContentBlock::Image {
                attachment_id: image.id,
            }],
        )
        .unwrap();
    let done = finish(&core, &t.id, false).await;
    assert_eq!(done.state, "failed");
    assert!(done.summary.contains("cannot process images"));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    server.abort();
}

#[tokio::test]
async fn balanced_escalates_with_images_evidence_and_same_snapshot_then_validates() {
    let original = "module.exports.add = (a,b) => a-b;\n";
    let (endpoint,server,_,log)=mock_runtime_with_log(Arc::new(move|body,index|{
        match index {
            0=>tool("read_file",json!({"path":"math.cjs"})),
            1|2=>tool("run_project_command",json!({"recipeId":"npm:test"})),
            3=>{assert_eq!(body["model"],"fixture:strong");let user=&body["messages"][1];assert_eq!(user["images"].as_array().unwrap().len(),1);let text=user["content"].as_str().unwrap();assert!(text.contains("read_file"));assert!(text.contains(&hash(original.as_bytes())));assert!(text.contains("failed"));assert!(text.contains("Continue the same task"));tool("apply_patch",json!({"path":"math.cjs","expectedHash":hash(original.as_bytes()),"hunks":[{"oldText":"a-b","newText":"a+b"}]}))},
            _=>tool("finish_task",json!({"summary":"Fixed","unresolved":[]}))
        }
    })).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(40, 30)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let image = crate::images::ingest(&core.store, "UI.png", png.get_ref(), "file").unwrap();
    let t = core
        .start_with_inputs(
            &p.id,
            "Fix addition in math.cjs",
            "task",
            Some("@profile/balanced".into()),
            Mode::Assisted,
            None,
            vec![ContentBlock::Image {
                attachment_id: image.id.clone(),
            }],
        )
        .unwrap();
    let done = finish(&core, &t.id, true).await;
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    assert_eq!(done.stages.len(), 2);
    assert_eq!(done.metrics.escalations, 1);
    assert_eq!(done.commands, 3);
    assert_eq!(done.checks.last().unwrap().status, "passed");
    assert_eq!(crate::images::ids(&done.inputs), vec![image.id.clone()]);
    assert_eq!(done.continuation.as_ref().unwrap().images, vec![image.id]);
    assert!(done
        .continuation
        .as_ref()
        .unwrap()
        .checks
        .iter()
        .any(|s| s.contains("failed")));
    assert!(std::fs::read_to_string(Path::new(&p.path).join("math.cjs"))
        .unwrap()
        .contains("a-b"));
    let requests = log.lock().unwrap();
    let unload = requests
        .iter()
        .position(|r| {
            r["path"] == "/api/generate"
                && r["body"]["model"] == "fixture:8b"
                && r["body"]["keep_alive"] == 0
        })
        .unwrap();
    let strong_load = requests
        .iter()
        .position(|r| {
            r["path"] == "/api/generate"
                && r["body"]["model"] == "fixture:strong"
                && r["body"].get("options").is_some()
        })
        .unwrap();
    assert!(unload < strong_load);
    assert_eq!(
        requests
            .iter()
            .filter(|r| r["path"] == "/api/chat" && r["body"]["model"] == "fixture:strong")
            .count(),
        2
    );
    server.abort();
}

#[tokio::test]
async fn strong_failure_exports_accumulated_continuation_without_image_pixels() {
    let (endpoint, server, _) = mock_runtime(Arc::new(|_, index| match index {
        0 => tool("read_file", json!({"path":"math.cjs"})),
        1 | 2 => tool("run_project_command", json!({"recipeId":"npm:test"})),
        _ => tool(
            "finish_task",
            json!({"summary":"No reliable fix","unresolved":["addition still fails"]}),
        ),
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(24, 24)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let attachment =
        crate::images::ingest(&core.store, "private-screen.png", png.get_ref(), "file").unwrap();
    let t = core
        .start_with_inputs(
            &p.id,
            "Fix addition in math.cjs",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
            vec![ContentBlock::Image {
                attachment_id: attachment.id,
            }],
        )
        .unwrap();
    let done = finish(&core, &t.id, true).await;
    assert_eq!(done.state, "escalation_ready", "{}", done.summary);
    assert_eq!(done.stages.len(), 2);
    assert!(done.summary.contains("Codex"));
    let package = crate::handoff::preview(&core.store, &done.id).unwrap();
    assert!(package.markdown.contains("Accumulated local investigation"));
    assert!(package.markdown.contains("fixture:strong"));
    assert!(package.markdown.contains("failed"));
    assert!(package.omissions.iter().any(|s| s.contains("image pixels")));
    let destination = tmp.path().join("exports");
    std::fs::create_dir(&destination).unwrap();
    let export = crate::handoff::export(&core.store, &package.id, &destination).unwrap();
    assert!(Path::new(&export)
        .join("evidence/continuation.json")
        .is_file());
    let image_body =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png.get_ref());
    for file in [
        "HANDOFF.md",
        "metadata.json",
        "evidence/continuation.json",
        "evidence/activity.json",
    ] {
        assert!(!std::fs::read_to_string(Path::new(&export).join(file))
            .unwrap()
            .contains(&image_body));
    }
    server.abort();
}
fn project(core: &AppCore, path: &Path) -> Project {
    std::fs::create_dir(path).unwrap();
    std::fs::write(
        path.join("math.cjs"),
        "module.exports.add = (a,b) => a-b;\n",
    )
    .unwrap();
    std::fs::write(
        path.join("package.json"),
        r#"{"name":"fixture","scripts":{"test":"node test.cjs"}}"#,
    )
    .unwrap();
    std::fs::write(path.join("test.cjs"),"require('node:assert/strict').equal(require('./math.cjs').add(2,3),5);console.log('test executed');").unwrap();
    core.add_project(path).unwrap()
}
async fn finish(core: &AppCore, id: &str, approve: bool) -> Task {
    let start = Instant::now();
    let mut answered = HashSet::new();
    loop {
        let task: Task = core.store.get("tasks", id).unwrap();
        if !task.active() {
            return task;
        }
        if let Some(a) = &task.approval {
            if answered.insert(a.id.clone()) {
                core.answer(id, &a.id, if approve { "once" } else { "deny" })
                    .unwrap();
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "{} {}",
            task.state,
            task.summary
        );
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
}

#[tokio::test]
async fn full_agent_protocol_read_edit_approve_check_and_review() {
    let handler = Arc::new(|body: Value, index: usize| match index {
        0 => tool("read_file", json!({"path":"math.cjs"})),
        1 => {
            let messages = body["messages"].as_array().unwrap();
            let last = messages.iter().rev().find(|m| m["role"] == "tool").unwrap();
            let result: Value = serde_json::from_str(last["content"].as_str().unwrap()).unwrap();
            tool(
                "apply_patch",
                json!({"path":"math.cjs","expectedHash":result["expectedHash"],"hunks":[{"oldText":"a-b","newText":"a+b"}]}),
            )
        }
        _ => tool(
            "finish_task",
            json!({"summary":"Tests: 999 passed (untrusted fabricated claim)","unresolved":[]}),
        ),
    });
    let (endpoint, server, _) = mock_runtime(handler).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    core.store
        .put(
            "probes",
            "fixture-digest:fixture-v1",
            &json!({"status":"verified"}),
        )
        .unwrap();
    let project = project(&core, &tmp.path().join("project"));
    let task = core
        .start(
            &project.id,
            "Fix addition in math.cjs",
            "task",
            Some("fixture:8b".into()),
            Mode::Assisted,
            None,
        )
        .unwrap();
    let done = finish(&core, &task.id, true).await;
    server.abort();
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    assert_eq!(done.change_state, "proposed");
    assert_eq!(
        done.checks.last().unwrap().status,
        "passed",
        "{}",
        done.checks.last().unwrap().output
    );
    assert!(!done.summary.contains("999"));
    let review = core.review(&task.id).unwrap();
    assert_eq!(review.files.len(), 1);
    assert!(review.files[0].after.contains("a+b"));
    assert!(
        std::fs::read_to_string(Path::new(&project.path).join("math.cjs"))
            .unwrap()
            .contains("a-b")
    );
    let applied = core.change_files(&task.id, done.revision, "apply").unwrap();
    assert!(
        std::fs::read_to_string(Path::new(&project.path).join("math.cjs"))
            .unwrap()
            .contains("a+b")
    );
    core.review(&task.id).unwrap();
    core.change_files(&task.id, applied.revision, "undo")
        .unwrap();
    assert!(
        std::fs::read_to_string(Path::new(&project.path).join("math.cjs"))
            .unwrap()
            .contains("a-b")
    );
}
#[tokio::test]
async fn malicious_tool_calls_exhaust_repair_budget_without_execution() {
    let (endpoint, server, count) = mock_runtime(Arc::new(|_, _| {
        tool(
            "run_shell",
            json!({"command":"powershell -EncodedCommand malicious"}),
        )
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    core.store
        .put(
            "probes",
            "fixture-digest:fixture-v1",
            &json!({"status":"verified"}),
        )
        .unwrap();
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "Fix math.cjs",
            "task",
            Some("fixture:8b".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &t.id, false).await;
    server.abort();
    assert_eq!(done.state, "escalation_ready");
    assert_eq!(done.commands, 0);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(done.change_state, "none");
}

#[tokio::test]
async fn synthetic_capability_probe_requires_tool_follow_up() {
    let (endpoint, server, _) = mock_runtime(Arc::new(|body, index| {
        if index == 0 {
            tool("read_probe", json!({}))
        } else {
            let proof = body["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap();
            json!({"message":{"content":proof},"done":true,"eval_count":5})
        }
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    let model = Ollama::test_endpoint(endpoint)
        .probe("fixture:8b", &store, CancellationToken::new())
        .await
        .unwrap();
    server.abort();
    assert_eq!(model.capability, "verified");
}

#[tokio::test]
async fn automatic_routing_records_balanced_fallback() {
    let (endpoint, server, count) = mock_runtime(Arc::new(|_,_|json!({"message":{"role":"assistant","content":"The function subtracts instead of adding."},"done":true,"eval_count":8}))).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    core.store
        .put(
            "probes",
            "fixture-digest:fixture-v1",
            &json!({"status":"verified"}),
        )
        .unwrap();
    let mut settings = core.store.settings();
    settings.model_profiles.balanced = Some("fixture:8b".into());
    settings.model_profiles.fast = Some("missing-model".into());
    core.store.put("settings", "main", &settings).unwrap();
    let p = project(&core, &tmp.path().join("project"));
    let task = core
        .start(
            &p.id,
            "Explain math.cjs.",
            "explain",
            None,
            Mode::Observe,
            None,
        )
        .unwrap();
    let done = finish(&core, &task.id, false).await;
    assert_eq!(done.model.as_deref(), Some("fixture:8b"));
    assert_eq!(
        done.route.level, 1,
        "Sentence punctuation must not hide the selected filename"
    );
    assert_eq!(done.route.strategy, "balanced");
    assert!(done.events.iter().any(|e| e.detail.contains("Fallback")));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn fast_upgrade_preserves_the_global_tool_repair_budget() {
    let (endpoint, server, count) = mock_runtime(Arc::new(|body, _| {
        if body["model"] == "fixture:strong" {
            tool("run_shell", json!({"command":"never execute"}))
        } else {
            tool("finish_task", json!({"summary":"Finished","unresolved":[]}))
        }
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    for digest in ["fixture-digest", "strong-digest"] {
        core.store
            .put(
                "probes",
                &format!("{digest}:fixture-v1"),
                &json!({"status":"verified"}),
            )
            .unwrap();
    }
    let mut settings = core.store.settings();
    settings.model_profiles = ModelProfiles {
        fast: Some("fixture:strong".into()),
        balanced: Some("fixture:8b".into()),
        ..Default::default()
    };
    core.store.put("settings", "main", &settings).unwrap();
    let p = project(&core, &tmp.path().join("project"));
    let task = core
        .start(
            &p.id,
            "Fix math.cjs",
            "task",
            Some("@profile/fast".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &task.id, false).await;
    assert_eq!(done.model.as_deref(), Some("fixture:8b"));
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    assert_eq!(done.turns, 3);
    assert_eq!(done.tool_calls, 3);
    assert_eq!(done.commands, 0);
    assert!(done
        .events
        .iter()
        .any(|e| e.title.contains("Escalating to balanced")));
    assert_eq!(count.load(Ordering::SeqCst), 3);
    server.abort();
}

#[tokio::test]
async fn invalid_batch_stops_before_any_later_action() {
    let (endpoint,server,_)=mock_runtime(Arc::new(|_,_|json!({"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"run_shell","arguments":{}}},{"function":{"name":"run_shell","arguments":{}}},{"function":{"name":"inspect_project","arguments":{}}}]},"done":true,"eval_count":1}))).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    core.store
        .put(
            "probes",
            "fixture-digest:fixture-v1",
            &json!({"status":"verified"}),
        )
        .unwrap();
    let p = project(&core, &tmp.path().join("project"));
    let task = core
        .start(
            &p.id,
            "Fix math.cjs",
            "task",
            Some("fixture:8b".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &task.id, false).await;
    assert_eq!(done.state, "escalation_ready");
    assert_eq!(done.tool_calls, 2);
    assert_eq!(done.commands, 0);
    server.abort();
}

#[tokio::test]
async fn strong_uses_shared_harness_without_requiring_full_gpu_placement() {
    let (endpoint, server, count) = mock_runtime(Arc::new(|_, _| {
        tool(
            "finish_task",
            json!({"summary":"No source change needed","unresolved":[]}),
        )
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    for digest in ["fixture-digest", "strong-digest"] {
        core.store
            .put(
                "probes",
                &format!("{digest}:fixture-v1"),
                &json!({"status":"verified"}),
            )
            .unwrap();
    }
    let mut settings = core.store.settings();
    settings.model_profiles = ModelProfiles {
        fast: None,
        balanced: Some("fixture:8b".into()),
        strong: Some("fixture:strong".into()),
        strong_enabled: true,
    };
    core.store.put("settings", "main", &settings).unwrap();
    core.store
        .put(
            "calibrations",
            "strong",
            &ModelCalibration {
                model: "fixture:strong".into(),
                digest: "strong-digest".into(),
                runtime: "fixture-v1".into(),
                context_size: 32768,
                passed: true,
                full_gpu: Some(true),
                elapsed_ms: 1.0,
                tokens_per_second: None,
                allocation_bytes: None,
                free_gpu_bytes: Some(2e9),
                message: String::new(),
                evidence: String::new(),
                tested_at: now(),
                peak_ram_bytes: None,
                minimum_free_ram_bytes: None,
                time_to_first_token_ms: None,
                retries: 0,
                escalations: 0,
            },
        )
        .unwrap();
    let p = project(&core, &tmp.path().join("project"));
    let task = core
        .start(
            &p.id,
            "Fix everything around the addition bug",
            "task",
            None,
            Mode::Project,
            None,
        )
        .unwrap();
    assert_eq!(task.route.level, 3);
    let done = finish(&core, &task.id, true).await;
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    assert_eq!(done.model.as_deref(), Some("fixture:strong"));
    assert_eq!(done.route.level, 3);
    assert!(done.events.iter().any(|e| e.title.contains("Using strong")));
    assert_eq!(done.turns, 1);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server.abort();
}
