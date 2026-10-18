use local_agent_core::{agent::AppCore, domain::*, snapshots, store::Store};
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[tokio::main]
async fn main() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let root = workspace
        .join(".local-data/fast-file-acceptance")
        .join(id());
    let source = root.join("project");
    std::fs::create_dir_all(&source)?;
    let files = [
        ("one.txt", "alpha.txt", "Alpha content\n"),
        ("two.txt", "beta.txt", "Beta content\n"),
        ("three.txt", "gamma.txt", "Gamma content\n"),
    ];
    std::fs::write(source.join("notes.md"),"This small fixture stores three plain text records. Rename files while preserving their exact content. No project commands are needed.\n")?;
    for (old, _, text) in files {
        std::fs::write(source.join(old), text)?;
    }
    let verified = Store::open(&workspace.join(".local-data/model-verification-v03"))?;
    let core = AppCore::open(&root.join("data"))?;
    let settings = verified.settings();
    core.store.put("settings", "main", &settings)?;
    let health = core.provider.health(&verified).await;
    for m in &health.models {
        let key = format!("{}:{}", m.digest, health.version.as_deref().unwrap_or(""));
        if let Ok(probe) = verified.get::<serde_json::Value>("probes", &key) {
            core.store.put("probes", &key, &probe)?;
        }
    }
    let project = core.add_project(&source)?;
    let task=core.start(&project.id,"Summarize notes.md and rename exactly three files: one.txt to alpha.txt, two.txt to beta.txt, three.txt to gamma.txt. Preserve every file's exact contents. Read before editing. Use the deterministic approved rename_file tool for each rename, one rename at a time and at most two tools per response. Keep notes.md unchanged. Do not run commands. Finish with a short summary of notes.md and the operations.","task",None,Mode::Project,None)?;
    let start = Instant::now();
    let done = loop {
        let task: Task = core.store.get("tasks", &task.id)?;
        if !task.active() && !core.busy() {
            break task;
        }
        if let Some(a) = &task.approval {
            let safe = a.title == "Rename this file in the task copy?"
                && files
                    .iter()
                    .any(|(old, new, _)| a.detail == format!("{old} → {new}"));
            let _ = core.answer(&task.id, &a.id, if safe { "once" } else { "deny" });
        }
        if start.elapsed() > Duration::from_secs(240) {
            let _ = core.cancel(&task.id);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let work = snapshots::working_path(&core.store, done.snapshot_id.as_deref().unwrap_or(""))?;
    let mut passed = done.state == "review_ready"
        && done.stages.iter().all(|s| s.tier == "fast")
        && done.stages.len() == 1
        && done.commands == 0
        && files.iter().all(|(old, new, text)| {
            !work.join(old).exists()
                && std::fs::read_to_string(work.join(new)).is_ok_and(|v| v == *text)
                && std::fs::read_to_string(source.join(old)).is_ok_and(|v| v == *text)
        })
        && done
            .events
            .iter()
            .any(|e| e.title == "Agent notes (unverified)" && !e.detail.is_empty());
    let deltas = snapshots::deltas(
        &core.store,
        &core
            .store
            .get("snapshots", done.snapshot_id.as_deref().unwrap())?,
    )?;
    passed &= deltas.len() == 6
        && std::fs::read(work.join("notes.md"))? == std::fs::read(source.join("notes.md"))?;
    let mut apply_undo = false;
    if passed {
        core.review(&done.id)?;
        let applied = core.change_files(&done.id, done.revision, "apply")?;
        passed &= files.iter().all(|(old, new, text)| {
            !source.join(old).exists()
                && std::fs::read_to_string(source.join(new)).is_ok_and(|v| v == *text)
        });
        core.review(&done.id)?;
        core.change_files(&done.id, applied.revision, "undo")?;
        apply_undo = files.iter().all(|(old, new, text)| {
            !source.join(new).exists()
                && std::fs::read_to_string(source.join(old)).is_ok_and(|v| v == *text)
        });
        passed &= apply_undo;
    }
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":passed,"applyUndo":apply_undo,"seconds":start.elapsed().as_secs_f64(),"task":done}),
        )?,
    )?;
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"passed":passed,"route":done.route,"stages":done.stages,"seconds":start.elapsed().as_secs_f64(),"summary":done.summary,"evidence":root})
        )?
    );
    if !passed {
        return Err(AppError::new(
            "FAST_ACCEPTANCE",
            "The real Fast summary/rename scenario did not pass every check.",
        ));
    }
    Ok(())
}
