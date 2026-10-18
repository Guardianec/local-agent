//! Local-only acceptance against installed models and synthetic project data.
use local_agent_core::{
    agent::AppCore,
    domain::*,
    providers::{ChatOptions, ModelProvider, ThinkingPolicy},
    store::Store,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let root = workspace.join(".local-data/interaction-acceptance");
    let source = root.join("project");
    std::fs::create_dir_all(source.join("test"))?;
    for file in ["package.json", "math.cjs", "test/math.test.cjs"] {
        std::fs::copy(
            workspace.join("fixtures/node-small").join(file),
            source.join(file),
        )?;
    }
    let verified = Store::open(&workspace.join(".local-data/model-verification-v03"))?;
    let core = AppCore::open(&root.join("data"))?;
    core.store.put("settings", "main", &verified.settings())?;
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
    let project = core.add_project(&source)?;
    let direct = core
        .provider
        .chat(
            "qwen3.5:4b-q8_0",
            &[serde_json::json!({"role":"user","content":"What is a Fenwick tree?"})],
            &[],
            ChatOptions {
                context: 16384,
                thinking: ThinkingPolicy::Minimal,
            },
            CancellationToken::new(),
        )
        .await?;
    println!(
        "Direct Ollama: {} characters",
        direct.message["content"].as_str().unwrap_or("").len()
    );
    // Release only the model this acceptance process just loaded.
    core.provider.unload("qwen3.5:4b-q8_0").await?;
    let requests = [
        "What is a Fenwick tree?",
        "What do you think about this architecture?",
        "Can this application support MCP?",
        "Explain this function.",
        "Look through this repository and tell me what you would improve. Do not modify anything.",
        "Check whether the tests pass.",
        "Find why this test fails, but don't change anything.",
        "Fix the failing tests.",
    ];
    for request in requests {
        let task = core.start(
            &project.id,
            request,
            "task",
            Some("@profile/balanced".into()),
            Mode::Project,
            None,
        )?;
        let start = Instant::now();
        let done = loop {
            let task: Task = core.store.get("tasks", &task.id)?;
            if !task.active() && !core.busy() {
                break task;
            }
            if let Some(a) = &task.approval {
                let _ = core.answer(
                    &task.id,
                    &a.id,
                    if a.title == "Run this project's code on your PC?" {
                        "once"
                    } else {
                        "deny"
                    },
                );
            }
            if start.elapsed() > Duration::from_secs(240) {
                let _ = core.cancel(&task.id);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        println!(
            "{:?} | {} | {} tools | {} checks | {}",
            done.intent,
            done.state,
            done.tool_calls,
            done.checks.len(),
            request
        );
        assert!(done.result.is_some());
        assert!(!done.summary.trim().is_empty());
        if done.intent == Intent::Converse {
            assert_eq!(done.tool_calls, 0);
        }
        if done.change_state == "proposed" {
            core.change_files(&done.id, done.revision, "discard")?;
        }
    }
    println!("Acceptance data retained locally for native UI inspection.");
    Ok(())
}
