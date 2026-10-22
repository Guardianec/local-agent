//! Disposable real-model QA; GitHub uses only the user's protected QA credential and approved bounded logs.
use local_agent_core::{agent::AppCore, domain::*, mcp, registry, store::Store};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let tier = args.first().map(String::as_str).unwrap_or("balanced");
    let scenario = args.get(1).map(String::as_str).unwrap_or("docs");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let data = workspace
        .join(".local-data/mcp-model-acceptance")
        .join(id());
    let verified = Store::open(&workspace.join(".local-data/model-verification-v03"))?;
    let core = AppCore::open(&data.join("data"))?;
    let mut settings = verified.settings();
    settings.model_profiles.strong_enabled = true;
    settings.intelligence.automatic_escalation = false;
    if let Some(context) = args.get(2) {
        settings.intelligence.strong_context = context
            .parse()
            .map_err(|_| AppError::new("ARGUMENT", "Context must be numeric."))?;
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
            .filter(|report| report.digest == model.digest)
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
    let model = registry::spec(tier)
        .ok_or_else(|| AppError::new("ARGUMENT", "Choose fast, balanced or strong."))?
        .model_id;
    if !core
        .provider
        .health(&core.store)
        .await
        .models
        .iter()
        .any(|item| item.name == model && item.capability == "verified")
    {
        core.probe_model(&model).await?;
    }
    if tier == "strong" {
        let health = core.provider.health(&core.store).await;
        let installed = health
            .models
            .iter()
            .find(|item| item.name == model)
            .ok_or_else(|| AppError::new("MODEL_MISSING", "Strong is not installed."))?;
        if !local_agent_core::models::strong_ready(
            &health,
            installed,
            registry::context(&settings, "strong"),
        ) {
            let report = core.calibrate_model(&model).await?;
            println!(
                "Calibration: passed={} context={} elapsedMs={}",
                report.passed, report.context_size, report.elapsed_ms
            );
            if !report.passed {
                return Err(AppError::new(
                    "CALIBRATION_FAILED",
                    "Strong calibration failed.",
                ));
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
    let source = data.join("project");
    std::fs::create_dir_all(&source)?;
    std::fs::write(
        source.join("sample.ts"),
        "export const add = (a: number, b: number) => a - b;\n",
    )?;
    let project = core.add_project(&source)?;
    core.save_integration(
        mcp::Integration {
            id: id(),
            name: "Context7".into(),
            preset: "context7".into(),
            enabled: true,
            approved: false,
            credential: None,
            transport: mcp::Transport::Http {
                url: "https://mcp.context7.com/mcp".into(),
                local: false,
                headers: BTreeMap::new(),
            },
        },
        None,
        true,
    )?;
    let github_secret = if scenario == "github" {
        let qa = Store::open(&workspace.join(".local-data/mcp-completion-ui"))?;
        let mut integration = mcp::list(&qa)?
            .into_iter()
            .find(|config| {
                config.preset == "github"
                    && config.enabled
                    && config.approved
                    && config.credential.is_some()
            })
            .ok_or_else(|| {
                AppError::new("MCP_AUTH", "Configure GitHub in the isolated QA app first.")
            })?;
        let credential = mcp::secret(&qa, &integration)?;
        integration.id = id();
        integration.credential = None;
        core.save_integration(integration, credential.clone(), true)?;
        credential
    } else {
        None
    };
    let request = match scenario {
        "conversation" => "What is a Fenwick tree? Explain briefly.",
        "coding" => "Fix addition in sample.ts. Change only the subtraction operator to addition. No other changes.",
        "docs" => "Use Context7 to resolve the Tauri documentation library, then query its current tray icon API. Summarize the returned evidence briefly. Do not edit files.",
        "github" => "Use GitHub MCP to investigate the latest failed GitHub Actions run in Guardianec/local-agent. Use actions_list with method list_workflow_runs, perPage 1 and workflow_runs_filter status failure, then get_job_logs with failed_only true and a small tail_lines value if a failed run exists. Explain the recorded failure using bounded evidence. Do not edit files or perform external writes.",
        _ => return Err(AppError::new("ARGUMENT", "Choose conversation, coding or docs.")),
    };
    let started = Instant::now();
    let task = core.start(
        &project.id,
        request,
        "task",
        Some(model),
        Mode::Assisted,
        None,
    )?;
    let mut approvals = HashSet::new();
    let done: Task = loop {
        let current: Task = core.store.get("tasks", &task.id)?;
        if !current.active() {
            break current;
        }
        if let Some(approval) = &current.approval {
            if approvals.insert(approval.id.clone()) {
                core.answer(&task.id, &approval.id, "once")?;
            }
        }
        if started.elapsed() > Duration::from_secs(240) {
            let _ = core.cancel(&task.id);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    if let Some(secret) = &github_secret {
        assert!(!serde_json::to_string(&done)?.contains(secret));
        let handoff = local_agent_core::handoff::preview(&core.store, &done.id)?;
        assert!(!serde_json::to_string(&handoff)?.contains(secret));
        println!("PASS: GitHub credential absent from task and handoff records");
    }
    let mcp_count = done
        .events
        .iter()
        .filter(|event| event.kind == "mcp")
        .count();
    let review = core.review(&done.id)?;
    let summary = serde_json::json!({"tier":tier,"scenario":scenario,"state":done.state,"seconds":started.elapsed().as_secs_f64(),"tools":done.tool_calls,"mcpCalls":mcp_count,"changed":review.files.iter().map(|file|&file.path).collect::<Vec<_>>(),"summary":done.summary});
    std::fs::write(
        data.join("result.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    println!("{summary}");
    assert!(done.result.is_some());
    assert_eq!(
        std::fs::read_to_string(source.join("sample.ts"))?,
        "export const add = (a: number, b: number) => a - b;\n"
    );
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    match scenario {
        "conversation" => {
            assert_eq!(done.tool_calls, 0);
            assert_eq!(mcp_count, 0);
        }
        "coding" => {
            assert_eq!(mcp_count, 0);
            assert_eq!(review.files.len(), 1);
        }
        _ => {
            assert!(mcp_count >= 2, "Expected resolver and query-docs evidence");
            assert!(review.files.is_empty());
        }
    }
    Ok(())
}
