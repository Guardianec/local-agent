//! Real local-model QA. Only synthetic copies and anonymous public reads.
use local_agent_core::{agent::AppCore, browser::Component, commands, domain::*, store::Store};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let tier = args.first().map(String::as_str).unwrap_or("balanced");
    let ukrainian = args.get(1).is_some_and(|s| s == "uk");
    let visual = args.get(1).is_some_and(|s| s == "visual");
    let read = ukrainian || args.get(1).is_some_and(|s| s == "read");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let qa_root = workspace.join(".local-data/browser-acceptance");
    let data = if let Some(reuse) = args.get(3) {
        let path = Path::new(reuse).canonicalize()?;
        if !path.starts_with(qa_root.canonicalize()?) {
            return Err(AppError::new(
                "ARGUMENT",
                "Reuse is limited to disposable browser QA folders.",
            ));
        }
        path
    } else {
        qa_root.join(id())
    };
    let verified = Store::open(&workspace.join(".local-data/model-verification-v03"))?;
    let core = AppCore::open(&data.join("data"))?;
    let mut settings = verified.settings();
    settings.model_profiles.strong_enabled = true;
    if let Some(context) = args.get(2) {
        settings.intelligence.strong_context = context
            .parse()
            .map_err(|_| AppError::new("ARGUMENT", "Strong QA context must be numeric."))?;
    }
    core.store.put("settings", "main", &settings)?;
    let health = core.provider.health(&verified).await;
    for model in &health.models {
        let key = format!(
            "{}:{}",
            model.digest,
            health.version.as_deref().unwrap_or("")
        );
        if let Ok(probe) = verified.get::<serde_json::Value>("probes", &key) {
            core.store.put("probes", &key, &probe)?;
        }
        for report in health
            .calibrations
            .iter()
            .filter(|c| c.digest == model.digest)
        {
            core.store.put(
                "calibrations",
                &format!(
                    "{}:{}:{}",
                    report.digest, report.runtime, report.context_size
                ),
                report,
            )?;
        }
    }
    let model = local_agent_core::registry::spec(tier)
        .ok_or_else(|| AppError::new("ARGUMENT", "Choose fast, balanced or strong."))?
        .model_id;
    if !core
        .provider
        .health(&core.store)
        .await
        .models
        .iter()
        .any(|m| m.name == model && m.capability == "verified")
    {
        core.probe_model(&model).await?;
    }
    if tier == "strong" {
        let health = core.provider.health(&core.store).await;
        let installed = health
            .models
            .iter()
            .find(|m| m.name == model)
            .ok_or_else(|| AppError::new("MODEL_MISSING", "Strong is not installed."))?;
        if !local_agent_core::models::strong_ready(
            &health,
            installed,
            local_agent_core::registry::context(&settings, "strong"),
        ) {
            println!("STRONG_CALIBRATION=required_for_current_runtime_and_context");
            tokio::time::sleep(Duration::from_secs(5)).await;
            let report = core.calibrate_model(&model).await?;
            println!(
                "STRONG_CALIBRATION={}",
                serde_json::json!({"passed":report.passed,"context":report.context_size,"elapsedMs":report.elapsed_ms})
            );
            if !report.passed {
                return Err(AppError::new(
                    "CALIBRATION_FAILED",
                    "Strong did not pass its matching coding benchmark.",
                ));
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
    let node = commands::find_program("node.exe")
        .ok_or_else(|| AppError::new("NODE_MISSING", "Node is required for this fixture."))?;
    let helper = workspace.join("browser-helper/main.cjs");
    let chromium = workspace.join(".local-data/browsers/chromium-1248/chrome-win64/chrome.exe");
    let component = Component {
        node_hash: hash(&std::fs::read(&node)?),
        node,
        helper_hash: hash(&std::fs::read(&helper)?),
        helper: helper.to_string_lossy().into(),
        chromium_hash: hash(&std::fs::read(&chromium)?),
        chromium: chromium.to_string_lossy().into(),
    };
    core.store.put("browser", "component", &component)?;
    let source = data.join("project");
    let files = [
        "package.json",
        "report.cjs",
        "server.cjs",
        "index.html",
        "README.md",
        "test/report.test.cjs",
    ];
    std::fs::create_dir_all(source.join("test"))?;
    let mut originals = Vec::new();
    for file in files {
        let bytes = std::fs::read(workspace.join("fixtures/browser-report").join(file))?;
        std::fs::write(source.join(file), &bytes)?;
        originals.push((file, bytes));
    }
    let code = id().replace('-', "")[..6].to_uppercase();
    let mut visual_url = String::new();
    let visual_server = if visual {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        visual_url = format!("http://{}/", listener.local_addr()?);
        let html=r#"<!doctype html><title>Synthetic visual fixture</title><style>body{font:24px system-ui;background:white;color:#172235}h1{position:absolute;top:20px;left:30px}button{position:absolute;top:55px;left:30px;font-size:24px;padding:12px}canvas{position:absolute;top:180px;left:30px}</style><h1>Report dashboard</h1><button>Load report</button><canvas width="650" height="130"></canvas><script>const c=document.querySelector('canvas').getContext('2d');c.font='bold 64px monospace';c.fillText('FIXTURE_CODE',20,90)</script>"#.replace("FIXTURE_CODE",&code);
        Some(tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let html = html.clone();
                tokio::spawn(async move {
                    let mut buffer = [0u8; 4096];
                    let _ = stream.read(&mut buffer).await;
                    let _=stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",html.len()).as_bytes()).await;
                });
            }
        }))
    } else {
        None
    };
    let project = if read || visual {
        core.browser_workspace()?
    } else {
        core.add_project(&source)?
    };
    let visual_request=format!("Open {visual_url} using local_read and take a viewport screenshot. Quote the six-character code drawn in the canvas and identify whether the heading and button overlap. Do not interact or submit anything.");
    let request = if visual {
        visual_request.as_str()
    } else if ukrainian {
        "Відкрий https://example.com/ і коротко поясни її українською. Нічого не надсилай."
    } else if read {
        "Read https://example.com/ and summarize what it says. Use web_fetch; do not interact or submit anything."
    } else {
        "Fix the local web app report loading bug. FIRST reproduce the HTTP 500 in the browser BEFORE reading or editing report.cjs: start its npm:dev server, open its returned URL with browser_open local_test, click Load report and inspect console/network evidence. Fix only report.cjs, run npm:test, stop and restart the dev server, navigate the existing browser to the new returned URL and click Load report to verify the repair. Do not edit tests. Use a screenshot only if it helps."
    };
    let task = core.start(
        &project.id,
        request,
        "task",
        Some(format!("@profile/{tier}")),
        Mode::Project,
        None,
    )?;
    let started = Instant::now();
    let mut answered = std::collections::HashSet::new();
    let done = loop {
        let task: Task = core.store.get("tasks", &task.id)?;
        if !task.active() && !core.busy() {
            break task;
        }
        if let Some(approval) = &task.approval {
            if answered.insert(approval.id.clone()) {
                let allowed = if tier == "strong"
                    && approval.title == "Use the Strong model for a bounded local attempt?"
                {
                    true
                } else if read || visual {
                    approval.risk == "browser_access"
                        && approval.preview.contains(if visual {
                            visual_url.as_str()
                        } else {
                            "example.com"
                        })
                } else {
                    (approval.risk != "browser_access"
                        || approval.preview.contains("http://127.0.0.1:"))
                        && matches!(
                            approval.title.as_str(),
                            "Start this project's development server?"
                                | "Run this project's code on your PC?"
                                | "Open this isolated browser session?"
                                | "Allow this additional browser origin?"
                                | "Use the Strong model for a bounded local attempt?"
                                | "Confirm a bounded local attempt"
                        )
                };
                println!(
                    "APPROVAL={} decision={}",
                    approval.title,
                    if allowed { "once" } else { "deny" }
                );
                core.answer(
                    &task.id,
                    &approval.id,
                    if allowed { "once" } else { "deny" },
                )?;
            }
        }
        if started.elapsed() > Duration::from_secs(650) {
            let _ = core.cancel(&task.id);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    for (file, bytes) in originals {
        assert_eq!(
            std::fs::read(source.join(file))?,
            bytes,
            "Original fixture changed: {file}"
        );
    }
    let review = core.review(&done.id)?;
    let browser = done
        .events
        .iter()
        .filter(|e| e.title == "Browser observation")
        .map(|e| e.detail.as_str())
        .collect::<Vec<_>>();
    let result = serde_json::json!({"tier":tier,"readOnly":read||visual,"visual":visual,"task":done.id,"state":done.state,"intent":done.intent,"seconds":started.elapsed().as_secs_f64(),"tools":done.tool_calls,"checks":done.checks.iter().map(|c|(&c.name,&c.status)).collect::<Vec<_>>(),"changed":review.files.iter().map(|d|&d.path).collect::<Vec<_>>(),"browserObservations":browser.len(),"summary":done.summary});
    println!("BROWSER_QA={result}");
    std::fs::write(
        data.join("result.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    if read || visual {
        assert_eq!(done.intent, Intent::Inspect);
        assert!(review.files.is_empty());
        if visual {
            assert!(!local_agent_core::images::ids(&done.inputs).is_empty());
            assert!(
                done.summary.to_uppercase().contains(&code),
                "Vision did not quote the canvas-only code"
            );
        } else if ukrainian {
            assert!(
                !browser.is_empty() || done.events.iter().any(|e| e.title == "Web observation")
            );
            assert!(done
                .summary
                .chars()
                .any(|c| ('\u{0400}'..='\u{04ff}').contains(&c)));
        } else {
            assert!(done.events.iter().any(|e| e.title == "Web observation"));
        }
    } else {
        assert!(review.files.iter().any(|d| d.path == "report.cjs"));
        assert!(review.files.iter().all(|d| d.path == "report.cjs"));
        assert!(done.checks.iter().any(|c| c.status == "passed"));
        assert!(
            browser.iter().any(|s| s.contains("500")),
            "Missing browser reproduction evidence"
        );
        assert!(
            browser
                .iter()
                .any(|s| s.contains("Synthetic report: 3 items")),
            "Missing successful browser retest"
        );
    }
    assert!(done.result.is_some());
    assert!(
        !data
            .join("data")
            .join("tasks")
            .join(&done.id)
            .join("browser")
            .join("home")
            .exists(),
        "Temporary browser HOME survived terminal cleanup"
    );
    if let Some(server) = visual_server {
        server.abort();
    }
    Ok(())
}
