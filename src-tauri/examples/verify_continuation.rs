//! Real-model orchestration acceptance, deliberately not a reasoning benchmark.
use local_agent_core::{agent::AppCore, domain::*, handoff, registry, snapshots, store::Store};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let root = workspace
        .join(".local-data/continuation-acceptance")
        .join(id());
    let source = root.join("project");
    std::fs::create_dir_all(&source)?;
    let package = r#"{"name":"local-agent-continuation-fixture","version":"1.0.0","scripts":{"test":"node test.cjs"}}"#;
    let test="console.error('CONTROLLED_DEPENDENCY_UNAVAILABLE: external test dependency is intentionally unavailable. Preserve test.cjs and package.json; do not fabricate a passing result.');process.exitCode=1;\n";
    for (name,text) in [("package.json",package),("test.cjs",test),("probe.cjs","// The external dependency cannot be restored by changing this local value.\nmodule.exports={ready:false};\n")]{std::fs::write(source.join(name),text)?;}
    let verified = Store::open(&workspace.join(".local-data/model-verification-v03"))?;
    let core = AppCore::open(&root.join("data"))?;
    let health = core.provider.health(&verified).await;
    let mut settings = verified.settings();
    settings.intelligence.automatic_escalation = true;
    let strong_name = registry::spec("strong").unwrap().model_id;
    let strong = health
        .models
        .iter()
        .find(|m| m.name == strong_name)
        .ok_or_else(|| AppError::new("MODEL_MISSING", "Install and benchmark Strong first."))?;
    if !local_agent_core::models::strong_ready(
        &health,
        strong,
        registry::context(&settings, "strong"),
    ) {
        return Err(AppError::new(
            "CALIBRATION_REQUIRED",
            "Strong needs its current coding benchmark.",
        ));
    }
    settings.model_profiles.strong_enabled = true;
    core.store.put("settings", "main", &settings)?;
    for model in &health.models {
        let key = format!(
            "{}:{}",
            model.digest,
            health.version.as_deref().unwrap_or("")
        );
        if let Ok(probe) = verified.get::<serde_json::Value>("probes", &key) {
            core.store.put("probes", &key, &probe)?;
        }
    }
    for c in &health.calibrations {
        core.store.put(
            "calibrations",
            &format!("{}:{}:{}", c.digest, c.runtime, c.context_size),
            c,
        )?;
    }
    let project = core.add_project(&source)?;
    let task=core.start(&project.id,"Investigate the failing test in this disposable project. Read probe.cjs, run the existing test twice to confirm repeatability, and diagnose the blocker. Keep test.cjs and package.json unchanged; do not disable validation or claim success without a passing check. Preserve the investigation if this external dependency cannot be repaired locally.","task",Some("@profile/balanced".into()),Mode::Project,None)?;
    let start = Instant::now();
    let mut prior_checks = 0;
    let mut strong_start_seq = 0;
    let mut sampled = false;
    let mut strong_response = false;
    let done = loop {
        let t: Task = core.store.get("tasks", &task.id)?;
        if t.stages.iter().any(|s| s.tier == "strong") {
            if !sampled {
                let c = t.continuation.as_ref().unwrap();
                prior_checks = c.checks.len();
                strong_start_seq = t
                    .events
                    .iter()
                    .rev()
                    .find(|e| e.title.starts_with("Using strong"))
                    .map_or(0, |e| e.seq);
                sampled = true;
                println!("Strong stage continues snapshot {:?} with {} prior check records and {} useful tool outcomes.",t.snapshot_id,prior_checks,c.tool_outcomes.len());
            }
            strong_response |= t
                .events
                .iter()
                .any(|e| e.kind == "usage" && e.seq > strong_start_seq);
        }
        if !t.active() && !core.busy() {
            break t;
        }
        if let Some(approval) = &t.approval {
            let unchanged = t
                .snapshot_id
                .as_ref()
                .and_then(|sid| snapshots::working_path(&core.store, sid).ok())
                .is_some_and(|work| {
                    std::fs::read_to_string(work.join("package.json")).is_ok_and(|v| v == package)
                        && std::fs::read_to_string(work.join("test.cjs")).is_ok_and(|v| v == test)
                        && std::fs::read_dir(work).is_ok_and(|v| v.count() == 3)
                });
            let safe = unchanged
                && (approval.title == "Run this project's code on your PC?"
                    || approval.title == "Escalate this investigation to Strong?");
            let _ = core.answer(&t.id, &approval.id, if safe { "once" } else { "deny" });
        }
        if start.elapsed() > Duration::from_secs(480) {
            let _ = core.cancel(&task.id);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let preview = handoff::preview(&core.store, &done.id)?;
    let output = root.join("export");
    std::fs::create_dir(&output)?;
    let export = handoff::export(&core.store, &preview.id, &output)?;
    let passed = done.state == "escalation_ready"
        && done.stages.iter().any(|s| s.tier == "balanced")
        && done.stages.iter().any(|s| s.tier == "strong")
        && prior_checks >= 2
        && strong_response
        && preview
            .markdown
            .contains("CONTROLLED_DEPENDENCY_UNAVAILABLE")
        && preview.markdown.contains(&strong_name);
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":passed,"seconds":start.elapsed().as_secs_f64(),"priorChecks":prior_checks,"strongResponse":strong_response,"task":done,"handoff":export}),
        )?,
    )?;
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"passed":passed,"state":done.state,"stages":done.stages,"checks":done.checks.len(),"seconds":start.elapsed().as_secs_f64(),"summary":done.summary,"evidence":root})
        )?
    );
    if !passed {
        return Err(AppError::new(
            "CONTINUATION_ACCEPTANCE",
            "The real-model continuation checks did not all pass; inspect the retained result.",
        ));
    }
    Ok(())
}
