use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn interrupted_browser_profile_is_removed_without_removing_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let core = AppCore::open(&data).unwrap();
    let project = core.browser_workspace().unwrap();
    let started = core
        .start(
            &project.id,
            "Prepare for Codex",
            "handoff",
            None,
            Mode::Assisted,
            None,
        )
        .unwrap();
    let mut task = finish(&core, &started.id, false).await;
    let folder = data.join("tasks").join(&task.id);
    std::fs::create_dir_all(folder.join("browser").join("home").join("profile")).unwrap();
    std::fs::write(
        folder
            .join("browser")
            .join("home")
            .join("profile")
            .join("Cookies"),
        b"synthetic cache",
    )
    .unwrap();
    std::fs::write(folder.join("keep-evidence.txt"), b"retained evidence").unwrap();
    task.state = "running".into();
    task.result = None;
    task.finished_at = None;
    core.store.put("tasks", &task.id, &task).unwrap();
    drop(core);
    let reopened = AppCore::open(&data).unwrap();
    let interrupted: Task = reopened.store.get("tasks", &task.id).unwrap();
    assert_eq!(interrupted.state, "interrupted");
    assert!(!folder.join("browser/home").exists());
    assert_eq!(
        std::fs::read(folder.join("keep-evidence.txt")).unwrap(),
        b"retained evidence"
    );
}

#[tokio::test]
async fn browser_workspace_has_no_project_or_host_authority() {
    let tmp = tempfile::tempdir().unwrap();
    let core = AppCore::open(tmp.path()).unwrap();
    let project = core.browser_workspace().unwrap();
    assert!(project.browser_only);
    assert_eq!(project.id, core.browser_workspace().unwrap().id);
    let root = ConfinedRoot::open(Path::new(&project.path)).unwrap();
    let started = core
        .start(
            &project.id,
            "Prepare for Codex",
            "handoff",
            None,
            Mode::Assisted,
            None,
        )
        .unwrap();
    let mut task = finish(&core, &started.id, false).await;
    task.intent = Intent::Execute;
    let mut session = Session::new(CancellationToken::new());
    for name in ["read_file", "apply_patch", "run_project_command"] {
        assert_eq!(
            core.execute_tool(&mut task, &project, &root, name, json!({}), &mut session)
                .await
                .unwrap_err()
                .code,
            "BROWSER_WORKSPACE"
        );
    }
}

#[tokio::test]
async fn browser_and_viewport_survive_balanced_to_strong_without_reopening() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let site = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;
                let body = r#"<!doctype html><title>Continuation fixture</title><button onclick="document.querySelector('p').textContent='After escalation'">Continue fixture</button><p>Before escalation</p>"#;
                let _=stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
            });
        }
    });
    let target = url.clone();
    let (endpoint,runtime,_)=mock_runtime(Arc::new(move|body,index|match index {
        0=>tool("browser_open",json!({"url":target,"profile":"local_test","steps":[]})),
        1=>tool("browser_action",json!({"action":"screenshot"})),
        2=>tool("read_file",json!({"path":"math.cjs"})),
        3|4=>tool("run_project_command",json!({"recipeId":"npm:test"})),
        5=>{assert_eq!(body["model"],"fixture:strong");assert_eq!(body["messages"][1]["images"].as_array().unwrap().len(),1);assert!(body["messages"][1]["content"].as_str().unwrap().contains("browser_open"));tool("browser_action",json!({"action":"click","element":"1:1"}))},
        6=>tool("apply_patch",json!({"path":"math.cjs","expectedHash":hash(b"module.exports.add = (a,b) => a-b;\n"),"hunks":[{"oldText":"a-b","newText":"a+b"}]})),
        _=>tool("finish_task",json!({"summary":"Verified retained browser state","unresolved":[]}))
    })).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let node = commands::find_program("node.exe").expect("Node required for browser QA");
    let helper = workspace.join("browser-helper/main.cjs");
    let chromium = workspace.join(".local-data/browsers/chromium-1248/chrome-win64/chrome.exe");
    let component = crate::browser::Component {
        node_hash: hash(&std::fs::read(&node).unwrap()),
        node,
        helper_hash: hash(&std::fs::read(&helper).unwrap()),
        helper: helper.to_string_lossy().into(),
        chromium_hash: hash(&std::fs::read(&chromium).unwrap()),
        chromium: chromium.to_string_lossy().into(),
    };
    core.store.put("browser", "component", &component).unwrap();
    let project = project(&core, &tmp.path().join("project"));
    let task = core
        .start(
            &project.id,
            &format!("Fix math.cjs and verify the disposable browser fixture at {url}"),
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &task.id, true).await;
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    assert_eq!(done.stages.len(), 2);
    let observations = done
        .events
        .iter()
        .filter(|e| e.title == "Browser observation")
        .map(|e| serde_json::from_str::<Value>(&e.detail).unwrap())
        .collect::<Vec<_>>();
    let first = &observations.first().unwrap()["observation"];
    let last = &observations.last().unwrap()["observation"];
    assert_eq!(first["session"], last["session"]);
    assert!(last["text"].as_str().unwrap().contains("After escalation"));
    assert_eq!(crate::images::ids(&done.inputs).len(), 1);
    assert_eq!(done.checks.last().unwrap().status, "passed");
    runtime.abort();
    site.abort();
}

#[tokio::test]
async fn legacy_proposal_without_finish_timestamp_can_be_continued() {
    let (endpoint, server, _) = mock_runtime(Arc::new(|_, _| json!({"message":{"role":"assistant","content":"Please review the retained proposal."},"done":true}))).await;
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
    let mut prior = finish(&core, &t.id, false).await;
    prior.finished_at = None;
    prior.result = None;
    prior.change_state = "proposed".into();
    core.save(&prior).unwrap();
    let (tx, rx) = oneshot::channel();
    let handle = tokio::runtime::Handle::current();
    let worker = core.clone();
    std::thread::spawn(move || {
        let _entered = handle.enter();
        let _ = tx.send(worker.start(
            &p.id,
            "Fix it.",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            Some(prior.id),
        ));
    });
    let next = tokio::time::timeout(Duration::from_secs(3), rx)
        .await
        .expect("Continuation must not deadlock while saving the prior task")
        .unwrap()
        .unwrap();
    let done = finish(&core, &next.id, false).await;
    assert!(done.result.is_some());
    server.abort();
}

#[tokio::test]
async fn ambiguous_wording_uses_bounded_classifier_and_ask_cannot_authorize_edits() {
    let (endpoint, server, count) = mock_runtime(Arc::new(|body, index| {
        if index == 0 {
            assert!(body["tools"].as_array().is_none_or(Vec::is_empty));
            json!({"message":{"role":"assistant","content":"inspect"},"done":true})
        } else {
            json!({"message":{"role":"assistant","content":"Inspection report."},"done":true})
        }
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "Please take a closer look.",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &t.id, false).await;
    assert_eq!(done.intent, Intent::Inspect);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(done.snapshot_id.is_some());
    assert_eq!(
        crate::interaction::infer("Fix math.cjs", "ask", Some(&done), &p).0,
        Intent::Inspect
    );
    assert_eq!(
        crate::interaction::infer("Okay, apply that fix.", "task", Some(&done), &p).0,
        Intent::Execute
    );
    server.abort();
}

#[test]
fn lifecycle_has_explicit_legal_and_illegal_edges() {
    let explanation = crate::interaction::explanation(
        "In `order.cjs`, shipping used the discounted amount. All tests passed.",
    );
    assert!(explanation.contains("`order.cjs`"));
    assert!(explanation.contains("Model explanation (unverified)"));
    assert_eq!(
        crate::interaction::failure(&AppError::new(
            "MODEL_CALIBRATION_REQUIRED",
            "Enable Strong after a passing coding benchmark.",
        ))
        .category,
        "MODEL_UNAVAILABLE"
    );
    assert!(crate::interaction::legal_transition(
        "running",
        "waiting_approval"
    ));
    assert!(crate::interaction::legal_transition(
        "waiting_approval",
        "running"
    ));
    assert!(crate::interaction::legal_transition(
        "validating",
        "running"
    ));
    assert!(crate::interaction::legal_transition("running", "cancelled"));
    assert!(!crate::interaction::legal_transition(
        "review_ready",
        "running"
    ));
    assert!(!crate::interaction::legal_transition("running", "made_up"));
}

#[tokio::test]
async fn intent_matrix_and_context_followup() {
    let (endpoint, server, _, log) = mock_runtime_with_log(Arc::new(|_, _| json!({"message":{"role":"assistant","content":"The subtraction in math.cjs causes the failure."},"done":true}))).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    for (request, intent) in [
        ("What is wrong with math.cjs?", Intent::Inspect),
        (
            "Open the Tauri documentation and explain the current tray API.",
            Intent::Inspect,
        ),
        (
            "Can this application support browser tasks?",
            Intent::Converse,
        ),
        (
            "Знайди помилку на локальному сайті та виправ її.",
            Intent::Execute,
        ),
        ("Відкрий цю сторінку і коротко поясни її.", Intent::Inspect),
        ("Поясни, чому math.cjs працює неправильно.", Intent::Inspect),
        (
            "Знайди помилку, але поки нічого не змінюй.",
            Intent::Inspect,
        ),
        ("Виправ помилку та запусти тести.", Intent::Execute),
        ("Fix math.cjs. No edits to tests.", Intent::Execute),
        (
            "Fix math.cjs. Don't change anything except math.cjs.",
            Intent::Execute,
        ),
    ] {
        assert_eq!(
            crate::interaction::infer(request, "task", None, &p).0,
            intent,
            "{request}"
        );
    }
    for (request, intent) in [
        ("What is a Fenwick tree?", Intent::Converse),
        ("What is a segment tree?", Intent::Converse),
        ("What do you think about this architecture?", Intent::Converse),
        ("Do you think this model setup is good?", Intent::Converse),
        ("Can this application support MCP?", Intent::Converse),
        ("Explain this function.", Intent::Inspect),
        ("Look through this repository and tell me what you would improve. Do not modify anything.", Intent::Inspect),
        ("Check whether the tests pass.", Intent::Inspect),
        ("Find why this test fails, but don't change anything.", Intent::Inspect),
        ("Fix the failing tests.", Intent::Execute),
        ("Implement this small feature.", Intent::Execute),
        ("What looks wrong?", Intent::Converse),
        ("Fix this.", Intent::Execute),
        ("Fix the UI. Do not change tests, HTML or package.json.", Intent::Execute),
        ("How would you fix this?", Intent::Converse),
        ("Can the app support authentication?", Intent::Converse),
    ] {
        let t = core.start(&p.id, request, "task", Some("@profile/balanced".into()), Mode::Project, None).unwrap();
        let done = finish(&core, &t.id, false).await;
        assert_eq!(done.intent, intent, "{request}");
        assert!(!done.summary.trim().is_empty());
        assert!(done.result.is_some(), "{request}");
        assert_eq!(done.change_state, "none");
    }
    let t = core
        .start(
            &p.id,
            "Why is this failing?",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let prior = finish(&core, &t.id, false).await;
    let t = core
        .start(
            &p.id,
            "Fix it.",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            Some(prior.id),
        )
        .unwrap();
    let done = finish(&core, &t.id, false).await;
    assert_eq!(done.intent, Intent::Execute);
    let requests = log.lock().unwrap();
    let last = requests
        .iter()
        .rev()
        .find(|r| r["path"] == "/api/chat")
        .unwrap();
    assert!(last["body"]["messages"]
        .to_string()
        .contains("subtraction in math.cjs"));
    server.abort();
}

#[tokio::test]
async fn rapid_two_project_inspections_keep_context_separate() {
    let (endpoint, server, _, log) = mock_runtime_with_log(Arc::new(
        |_, _| json!({"message":{"role":"assistant","content":"Inspection complete."},"done":true}),
    ))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let alpha = project(&core, &tmp.path().join("alpha"));
    let beta = project(&core, &tmp.path().join("beta"));
    std::fs::write(
        tmp.path().join("alpha/math.cjs"),
        "// ALPHA_SYNTHETIC\nmodule.exports.add = (a,b) => a-b;\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("beta/math.cjs"),
        "// BETA_SYNTHETIC\nmodule.exports.add = (a,b) => a-b;\n",
    )
    .unwrap();
    for index in 0..20 {
        let chosen = if index % 2 == 0 { &alpha } else { &beta };
        let task = core
            .start(
                &chosen.id,
                "Explain math.cjs.",
                "task",
                Some("@profile/balanced".into()),
                Mode::Observe,
                None,
            )
            .unwrap();
        let done = finish(&core, &task.id, false).await;
        assert_eq!(done.intent, Intent::Inspect);
        assert_eq!(done.project_id, chosen.id);
        assert_eq!(done.tool_calls, 0);
        assert_eq!(done.change_state, "none");
        let requests = log.lock().unwrap();
        let chat = requests
            .iter()
            .rev()
            .find(|request| request["path"] == "/api/chat")
            .unwrap();
        let body = chat["body"]["messages"].to_string();
        let (own, other) = if index % 2 == 0 {
            ("ALPHA_SYNTHETIC", "BETA_SYNTHETIC")
        } else {
            ("BETA_SYNTHETIC", "ALPHA_SYNTHETIC")
        };
        assert!(body.contains(own), "task {index} missed its own project");
        assert!(
            !body.contains(other),
            "task {index} leaked the other project"
        );
    }
    server.abort();
}

#[tokio::test]
async fn empty_and_interrupted_responses_have_bounded_recovery() {
    for (content, done_flag, recover) in [
        ("   ", true, true),
        ("", true, false),
        ("partial", false, true),
        ("partial", false, false),
    ] {
        let (endpoint, server, count) = mock_runtime(Arc::new(move |_, index| {
            if recover && index > 0 {
                json!({"message":{"role":"assistant","content":"Recovered answer."},"done":true})
            } else {
                json!({"message":{"role":"assistant","content":content},"done":done_flag})
            }
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
                "What is a tree?",
                "task",
                Some("@profile/balanced".into()),
                Mode::Project,
                None,
            )
            .unwrap();
        let done = finish(&core, &t.id, false).await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert_eq!(
            done.state,
            if recover {
                "review_ready"
            } else {
                "escalation_ready"
            }
        );
        assert!(done.result.is_some());
        assert!(!done.summary.trim().is_empty());
        assert_eq!(done.tool_calls, 0);
        server.abort();
    }
}

#[tokio::test]
async fn inspect_blocks_unsolicited_writes_and_reports_failed_tests() {
    let (endpoint, server, _) = mock_runtime(Arc::new(|body, index| match index {
        0 => { assert!(!body["tools"].to_string().contains("apply_patch")); tool("create_file", json!({"path":"unauthorized.txt","content":"bad"})) },
        1 => tool("inspect_project", json!({})),
        2 => { let messages = body["messages"].as_array().unwrap(); let last = messages.iter().rev().find(|m| m["role"] == "tool").unwrap(); let result: Value = serde_json::from_str(last["content"].as_str().unwrap()).unwrap(); tool("run_project_command", json!({"recipeId":result["recipes"][0]["id"]})) },
        _ => json!({"message":{"role":"assistant","content":"The test fails because addition subtracts. I did not edit files."},"done":true}),
    })).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "Find why this test fails, but don't change anything.",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &t.id, true).await;
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    assert_eq!(done.checks.last().unwrap().status, "failed");
    assert_eq!(done.stages.len(), 1);
    assert_eq!(done.change_state, "none");
    assert!(done.summary.contains("addition subtracts"));
    assert!(!Path::new(&p.path).join("unauthorized.txt").exists());
    assert!(!core.review(&done.id).unwrap().can_apply);
    server.abort();
}

#[tokio::test]
async fn conversation_preserves_answer_without_tools() {
    let (endpoint, server, _, log) = mock_runtime_with_log(Arc::new(|_, _|
        json!({"message":{"role":"assistant","content":"A Fenwick tree supports prefix sums."},"done":true})
    )).await;
    let tmp = tempfile::tempdir().unwrap();
    let mut core = AppCore::open(&tmp.path().join("data")).unwrap();
    Arc::get_mut(&mut core).unwrap().provider = Ollama::test_endpoint(endpoint);
    configure_continuation_fixture(&core);
    let p = project(&core, &tmp.path().join("project"));
    let t = core
        .start(
            &p.id,
            "What is a Fenwick tree?",
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )
        .unwrap();
    let done = finish(&core, &t.id, false).await;
    assert_eq!(done.summary, "A Fenwick tree supports prefix sums.");
    assert_eq!(done.tool_calls, 0);
    assert!(log
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["path"] == "/api/chat")
        .all(|r| r["body"]["tools"].as_array().is_none_or(Vec::is_empty)));
    server.abort();
}

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
            assert!(
                task.result.is_some(),
                "Every terminal task must have a structured result: {}",
                task.state
            );
            return task;
        }
        if let Some(a) = &task.approval {
            if answered.insert(a.id.clone()) {
                core.answer(id, &a.id, if approve { "once" } else { "deny" })
                    .unwrap();
            }
        }
        assert!(
            // Shared Windows runners start several real npm checks more slowly.
            start.elapsed() < Duration::from_secs(60),
            "{} {} ({} commands; {} checks)",
            task.state,
            task.summary,
            task.commands,
            task.checks.len()
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
    let (endpoint, server, count) = mock_runtime(Arc::new(|body, index| {
        if index == 0 {
            assert_eq!(
                body["tools"][0]["function"]["parameters"]["required"],
                json!(["request"])
            );
            tool("read_probe", json!({"probe_id":"1"}))
        } else if index == 1 {
            assert!(body["messages"]
                .to_string()
                .contains("Tool arguments were rejected"));
            tool("read_probe", json!({"request":"proof"}))
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
    assert_eq!(count.load(Ordering::SeqCst), 3);
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
