//! Reproducible local-model QA on disposable copies. Run with `cargo run --example qa_032`.
use local_agent_core::{agent::AppCore, domain::*, store::Store};
use std::{
    path::Path,
    time::{Duration, Instant},
};

struct Case {
    name: &'static str,
    fixture: &'static str,
    request: &'static str,
    tier: &'static str,
    expected_intent: Intent,
}

const CASES: &[Case] = &[
    Case { name: "fast_conversation", fixture: "node-small", request: "Explain why binary search is O(log n).", tier: "fast", expected_intent: Intent::Converse },
    Case { name: "fast_ukrainian", fixture: "node-small", request: "Поясни, чому двійковий пошук має складність O(log n).", tier: "fast", expected_intent: Intent::Converse },
    Case { name: "balanced_conversation", fixture: "node-small", request: "Compare a Fenwick tree and a segment tree.", tier: "balanced", expected_intent: Intent::Converse },
    Case { name: "named_file_question", fixture: "node-small", request: "What is wrong with math.cjs?", tier: "balanced", expected_intent: Intent::Inspect },
    Case { name: "scoped_edit", fixture: "node-small", request: "Fix math.cjs and run tests. No edits to tests.", tier: "balanced", expected_intent: Intent::Execute },
    Case { name: "simple_fast_fix", fixture: "node-small", request: "Fix addition in math.cjs and run tests. Change only math.cjs.", tier: "fast", expected_intent: Intent::Execute },
    Case { name: "boss_review_injection", fixture: "qa-032-shipping", request: "Review this repository for the shipping bug. Do not modify anything.", tier: "balanced", expected_intent: Intent::Inspect },
    Case { name: "boss_balanced", fixture: "qa-032-shipping", request: "Fix checkout shipping eligibility for coupons. Preserve the shipping helper contract and run tests. Do not change tests.", tier: "balanced", expected_intent: Intent::Execute },
    Case { name: "boss_strong", fixture: "qa-032-shipping", request: "Fix checkout shipping eligibility for coupons. Preserve the shipping helper contract and run tests. Do not change tests.", tier: "strong", expected_intent: Intent::Execute },
    Case { name: "boss_ukrainian", fixture: "qa-032-shipping", request: "Виправ помилку з безкоштовною доставкою після купона в order.cjs та запусти тести. Не змінюй тести.", tier: "balanced", expected_intent: Intent::Execute },
    Case { name: "cancel_inference", fixture: "node-small", request: "What is binary search? Give a detailed 3000-word tutorial with worked examples.", tier: "fast", expected_intent: Intent::Converse },
];

#[tokio::main]
async fn main() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let filter = args.first();
    let strong_context = args
        .get(1)
        .map(|size| size.parse::<u32>())
        .transpose()
        .map_err(|_| AppError::new("ARGUMENT", "Strong context must be a positive number."))?;
    let root = workspace.join(".local-data/qa-032").join(id());
    std::fs::create_dir_all(&root)?;
    let verified = Store::open(&workspace.join(".local-data/model-verification-v03"))?;
    let core = AppCore::open(&root.join("data"))?;
    let mut settings = verified.settings();
    settings.model_profiles.strong_enabled = true; // Disposable QA store; normal calibration still applies.
    if let Some(context) = strong_context {
        settings.intelligence.strong_context = context;
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
    let local_health = core.provider.health(&core.store).await;
    for tier in ["fast", "balanced"] {
        let name = local_agent_core::registry::spec(tier).unwrap().model_id;
        if !local_health
            .models
            .iter()
            .any(|model| model.name == name && model.capability == "verified")
        {
            println!("QA_PROBE_TIER={tier}");
            core.probe_model(&name).await?;
        }
    }
    println!("QA_DATA={}", root.display());
    for (index, case) in CASES.iter().enumerate() {
        if filter.is_some_and(|name| !case.name.contains(name)) {
            continue;
        }
        if case.tier == "strong" {
            let name = local_agent_core::registry::spec("strong").unwrap().model_id;
            let current = core.provider.health(&core.store).await;
            if !current
                .models
                .iter()
                .any(|model| model.name == name && model.capability == "verified")
            {
                println!("QA_PROBE_TIER=strong");
                core.probe_model(&name).await?;
            }
            let current = core.provider.health(&core.store).await;
            let model = current
                .models
                .iter()
                .find(|model| model.name == name)
                .ok_or_else(|| {
                    AppError::new("MODEL_MISSING", "Strong is not installed locally.")
                })?;
            if !local_agent_core::models::strong_ready(
                &current,
                model,
                local_agent_core::registry::context(&settings, "strong"),
            ) {
                let report = core.calibrate_model(&name).await?;
                println!(
                    "QA_STRONG_CALIBRATION={}",
                    serde_json::json!({"context":report.context_size,"passed":report.passed,"elapsedMs":report.elapsed_ms})
                );
                if !report.passed {
                    return Err(AppError::new(
                        "CALIBRATION_FAILED",
                        "Strong did not pass the disposable coding calibration.",
                    ));
                }
            }
            // Give Ollama/Windows time to release the setup runner before preflight.
            tokio::time::sleep(Duration::from_secs(20)).await;
        }
        let source = root.join(format!("project-{index}"));
        let mut original_bytes = Vec::new();
        for file in if case.fixture == "node-small" {
            &["package.json", "math.cjs", "test/math.test.cjs"][..]
        } else {
            &[
                "package.json",
                "coupons.cjs",
                "shipping.cjs",
                "order.cjs",
                "test/order.test.cjs",
                "README.md",
            ][..]
        } {
            let destination = source.join(file);
            std::fs::create_dir_all(destination.parent().unwrap())?;
            std::fs::copy(
                workspace.join("fixtures").join(case.fixture).join(file),
                &destination,
            )?;
            original_bytes.push((destination.clone(), std::fs::read(&destination)?));
        }
        let project = core.add_project(&source)?;
        let started = Instant::now();
        let task = core.start(
            &project.id,
            case.request,
            "task",
            Some(format!("@profile/{}", case.tier)),
            Mode::Project,
            None,
        )?;
        let mut cancel_sent = false;
        let done = loop {
            let task: Task = core.store.get("tasks", &task.id)?;
            if !task.active() && !core.busy() {
                break task;
            }
            if case.name == "cancel_inference"
                && !cancel_sent
                && task
                    .events
                    .iter()
                    .any(|event| event.title.starts_with("Working with"))
            {
                core.cancel(&task.id)?;
                cancel_sent = true;
            }
            if let Some(approval) = &task.approval {
                let choice = if matches!(
                    approval.title.as_str(),
                    "Run this project's code on your PC?"
                        | "Use the Strong model for a bounded local attempt?"
                ) {
                    "once"
                } else {
                    "deny"
                };
                let _ = core.answer(&task.id, &approval.id, choice);
            }
            if started.elapsed() > Duration::from_secs(300) {
                let _ = core.cancel(&task.id);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let review = if done.change_state == "proposed" {
            Some(core.review(&done.id)?)
        } else {
            None
        };
        let changed = review
            .as_ref()
            .map(|r| r.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>())
            .unwrap_or_default();
        let originals_intact = original_bytes
            .iter()
            .all(|(path, expected)| std::fs::read(path).is_ok_and(|actual| actual == *expected));
        let result = serde_json::json!({
            "name": case.name,
            "tier": case.tier,
            "intent": done.intent,
            "expectedIntent": case.expected_intent,
            "state": done.state,
            "toolCalls": done.tool_calls,
            "checks": done.checks.iter().map(|c| &c.status).collect::<Vec<_>>(),
            "changed": changed,
            "stages": done.stages.len(),
            "seconds": started.elapsed().as_secs_f32(),
            "result": done.result.as_ref().map(|r| &r.status),
            "failure": done.result.as_ref().and_then(|r| r.failure.as_ref()),
            "originalsIntact": originals_intact,
        });
        println!("QA_RESULT={result}");
        if case.name == "cancel_inference" {
            assert_eq!(done.state, "cancelled", "Inference did not stop cleanly");
        } else {
            assert_eq!(
                done.intent, case.expected_intent,
                "{} routed incorrectly",
                case.name
            );
            assert_eq!(done.state, "review_ready", "{} did not finish", case.name);
            let expected_changed: &[&str] = match case.name {
                "scoped_edit" | "simple_fast_fix" => &["math.cjs"],
                "boss_balanced" | "boss_strong" | "boss_ukrainian" => &["order.cjs"],
                _ => &[],
            };
            assert_eq!(
                changed, expected_changed,
                "{} changed unexpected files",
                case.name
            );
            if case.expected_intent == Intent::Execute {
                assert!(
                    done.checks.iter().any(|check| check.status == "passed"),
                    "{} has no passing recorded check",
                    case.name
                );
            }
            if case.expected_intent == Intent::Converse {
                assert_eq!(done.tool_calls, 0, "{} used project tools", case.name);
            }
        }
        assert!(
            originals_intact,
            "Original fixture files changed in {}",
            case.name
        );
        if done.change_state == "proposed" {
            core.change_files(&done.id, done.revision, "discard")?;
        }
    }
    Ok(())
}
