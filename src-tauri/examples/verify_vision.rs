//! Explicit developer acceptance tool; only its disposable fixture can run code.
use local_agent_core::{agent::AppCore, domain::*, images, registry, snapshots, store::Store};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let tier = args.first().map(String::as_str).unwrap_or("fast");
    let coding = args.get(1).is_some_and(|v| v == "coding");
    let name = registry::spec(tier)
        .ok_or_else(|| AppError::new("ARGUMENT", "Choose a registry tier."))?
        .model_id;
    let source = root.join(".local-data/v03-vision");
    let verified = Store::open(&root.join(".local-data/model-verification-v03"))?;
    let core = AppCore::open(&source.join(format!("data-{}", id())))?;
    let settings = verified.settings();
    core.store.put("settings", "main", &settings)?;
    let health = core.provider.health(&verified).await;
    let model = health
        .models
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| AppError::new("MODEL_MISSING", "Benchmark this installed model first."))?;
    let key = format!("{}:{}", model.digest, health.version.unwrap_or_default());
    let probe: serde_json::Value = verified.get("probes", &key)?;
    core.store.put("probes", &key, &probe)?;
    for report in verified.list::<ModelCalibration>("calibrations")? {
        core.store.put(
            "calibrations",
            &format!(
                "{}:{}:{}",
                report.digest, report.runtime, report.context_size
            ),
            &report,
        )?;
    }
    let project = core.add_project(&source.join(if coding {
        "project"
    } else {
        "read-only-project"
    }))?;
    let original = if coding {
        Some(std::fs::read(Path::new(&project.path).join("styles.css"))?)
    } else {
        None
    };
    let package = std::fs::read(Path::new(&project.path).join("package.json")).ok();
    let test = std::fs::read(Path::new(&project.path).join("test.cjs")).ok();
    let image = images::ingest(
        &core.store,
        "screenshot.png",
        &std::fs::read(source.join("screenshot.png"))?,
        "file",
    )?;
    let request = if coding {
        "Fix the layout issue shown in the attached screenshot. Inspect project files and change only styles.css so the Save settings button uses normal flow below the email field without covering it. Remove obsolete offsets. Run the existing test recipe. Do not change tests, HTML or package.json."
    } else {
        "Describe the attached screenshot: quote the exact numeric error code in the red banner and explain the visible problem with the Save settings button. Do not read project files; they have no screenshot information. Finish with your description."
    };
    let task = core.start_with_inputs(
        &project.id,
        request,
        if coding { "task" } else { "explain" },
        Some(format!("@profile/{tier}")),
        if coding { Mode::Project } else { Mode::Observe },
        None,
        vec![ContentBlock::Image {
            attachment_id: image.id,
        }],
    )?;
    let started = Instant::now();
    let done = loop {
        let t: Task = core.store.get("tasks", &task.id)?;
        if !t.active() && !core.busy() {
            break t;
        }
        if let Some(a) = &t.approval {
            let safe = coding
                && a.title == "Run this project's code on your PC?"
                && t.snapshot_id
                    .as_ref()
                    .and_then(|sid| snapshots::working_path(&core.store, sid).ok())
                    .is_some_and(|work| {
                        package == std::fs::read(work.join("package.json")).ok()
                            && test == std::fs::read(work.join("test.cjs")).ok()
                            && std::fs::read_dir(&work).is_ok_and(|entries| entries.count() == 4)
                    });
            let _ = core.answer(&t.id, &a.id, if safe { "once" } else { "deny" });
        }
        if started.elapsed() > Duration::from_secs(300) {
            let _ = core.cancel(&task.id);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let mut passed = done.state == "review_ready";
    if coding {
        let changes = snapshots::deltas(
            &core.store,
            &core
                .store
                .get("snapshots", done.snapshot_id.as_deref().unwrap_or(""))?,
        )?;
        passed &= changes.len() == 1
            && changes[0].path == "styles.css"
            && done.checks.last().is_some_and(|c| c.status == "passed")
            && Some(std::fs::read(Path::new(&project.path).join("styles.css"))?) == original;
    } else {
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(source.join("expected.json"))?)?;
        passed &= done.summary.contains(expected["code"].as_str().unwrap());
    }
    let result = serde_json::json!({"passed":passed,"tier":tier,"model":name,"kind":if coding{"vision+coding"}else{"vision"},"taskId":done.id,"state":done.state,"summary":done.summary,"seconds":started.elapsed().as_secs_f64(),"checks":done.checks,"metrics":done.metrics,"snapshotId":done.snapshot_id,"data":core.store.root});
    std::fs::write(
        source.join(format!(
            "{tier}-{}.json",
            if coding { "coding" } else { "description" }
        )),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"passed":passed,"tier":tier,"kind":result["kind"],"summary":done.summary,"seconds":result["seconds"]})
        )?
    );
    if !passed {
        return Err(AppError::new(
            "VISION_ACCEPTANCE",
            "The real vision task did not satisfy its acceptance checks.",
        ));
    }
    Ok(())
}
