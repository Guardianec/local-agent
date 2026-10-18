use crate::{agent::AppCore, domain::*, fs::ConfinedRoot, models, snapshots};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

const PACKAGE: &str = r#"{"name":"local-agent-calibration","version":"1.0.0","scripts":{"test":"node --test test.cjs"}}"#;
const MATH: &str = "function add(a,b){return a-b;}\nmodule.exports={add};\n";
const TEST: &str = "const {test}=require('node:test'); const assert=require('node:assert/strict'); const {add}=require('./math.cjs'); test('positive addition',()=>assert.equal(add(2,3),5)); test('negative addition',()=>assert.equal(add(-2,-3),-5)); test('zero',()=>assert.equal(add(0,0),0));\n";

impl AppCore {
    pub async fn calibrate_model(&self, name: &str) -> Result<ModelCalibration> {
        let cancel = self.reserve_service("model-calibration")?;
        let result = self.calibration_inner(name, cancel).await;
        self.finish_service();
        result
    }
    async fn calibration_inner(
        &self,
        name: &str,
        cancel: CancellationToken,
    ) -> Result<ModelCalibration> {
        let settings = self.store.settings();
        let health = self.provider.health(&self.store).await;
        let model = health
            .models
            .iter()
            .find(|m| m.name == name && m.local)
            .ok_or_else(|| {
                AppError::new(
                    "MODEL_UNAVAILABLE",
                    "Choose an installed local model to calibrate.",
                )
            })?;
        let mut report = ModelCalibration {
            model: name.into(),
            digest: model.digest.clone(),
            runtime: health.version.clone().unwrap_or_default(),
            context_size: crate::registry::context_for_model(&settings, name),
            passed: false,
            full_gpu: None,
            elapsed_ms: 0.0,
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
        };
        if health.models.iter().any(|m| m.loaded) {
            return Err(AppError::new(
                "MODEL_BUSY",
                "Another model is loaded. Stop its current use before benchmarking.",
            ));
        }
        crate::lifecycle::preflight(
            &health,
            model,
            report.context_size,
            crate::registry::identity(name).is_some_and(|s| s.tier == "strong"),
        )?;
        let proof = self.provider.probe(name, &self.store, cancel.clone()).await;
        let _ = self.provider.unload(name).await;
        if let Err(error) = proof {
            if cancel.is_cancelled() {
                return Err(AppError::new("CANCELLED", "Model calibration stopped."));
            }
            report.message = error.message;
            report.evidence = error.details.unwrap_or_default();
            self.save_calibration(&report)?;
            return Ok(report);
        }
        let run_folder = self.store.root.join("calibration-runs").join(id());
        let source = run_folder.join("fixture");
        std::fs::create_dir_all(&source)?;
        for (path, text) in [
            ("package.json", PACKAGE),
            ("math.cjs", MATH),
            ("test.cjs", TEST),
        ] {
            std::fs::write(source.join(path), text)?;
        }
        let mut child = AppCore::open(&run_folder.join("data"))?;
        let child_mut = Arc::get_mut(&mut child).expect("new calibration controller");
        child_mut.provider = self.provider.clone();
        child_mut.calibration_run = true;
        let mut child_settings = settings.clone();
        child_settings.intelligence.fast_context = report.context_size;
        child_settings.intelligence.balanced_context = report.context_size;
        child_settings.intelligence.strong_context = report.context_size;
        child_settings.model_profiles = ModelProfiles {
            fast: Some(name.into()),
            balanced: Some(name.into()),
            ..Default::default()
        };
        child.store.put("settings", "main", &child_settings)?;
        child.store.put(
            "probes",
            &format!("{}:{}", model.digest, report.runtime),
            &serde_json::json!({"status":"verified","version":1,"at":now()}),
        )?;
        let project = child.add_project(&source)?;
        let started = Instant::now();
        let task = child.start(&project.id,"Fix addition in math.cjs so add(a,b) returns their sum for positive and negative numbers. Read the file, change only math.cjs, and run the existing test recipe. Leave test.cjs and package.json unchanged. Finish after tests pass.","task",Some(name.into()),Mode::Project,None)?;
        let mut last_sample = Instant::now() - Duration::from_secs(2);
        let final_task = loop {
            let task: Task = child.store.get("tasks", &task.id)?;
            if !task.active() && !child.busy() {
                break task;
            }
            if cancel.is_cancelled() || started.elapsed() > Duration::from_secs(240) {
                let _ = child.cancel(&task.id);
            }
            if let Some(approval) = &task.approval {
                let safe = approval.title == "Run this project's code on your PC?"
                    && calibration_files_unchanged(&child, &task);
                let _ = child.answer(&task.id, &approval.id, if safe { "once" } else { "deny" });
            }
            if task.turns > 0 && last_sample.elapsed() >= Duration::from_secs(1) {
                let loaded = self.provider.health(&self.store).await;
                if let Some(memory) = &loaded.memory {
                    report.minimum_free_ram_bytes = Some(
                        report
                            .minimum_free_ram_bytes
                            .unwrap_or(f64::MAX)
                            .min(memory.available_bytes),
                    );
                    report.peak_ram_bytes = Some(
                        report
                            .peak_ram_bytes
                            .unwrap_or(0.0)
                            .max(memory.total_bytes - memory.available_bytes),
                    );
                }
                if let Some(m) = loaded.models.iter().find(|m| m.name == name && m.loaded) {
                    if let (Some(gpu), Some(_)) = (m.vram, m.loaded_bytes) {
                        report.allocation_bytes =
                            Some(report.allocation_bytes.unwrap_or(0.0).max(gpu));
                        let full = models::fully_gpu(m) == Some(true);
                        report.full_gpu = Some(report.full_gpu.unwrap_or(true) && full);
                        if let Some(gpu) = loaded.gpu {
                            report.free_gpu_bytes = Some(
                                report
                                    .free_gpu_bytes
                                    .unwrap_or(f64::MAX)
                                    .min(gpu.free_bytes),
                            );
                        }
                    }
                }
                last_sample = Instant::now();
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        report.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        report.time_to_first_token_ms = final_task.metrics.time_to_first_token_ms;
        report.retries = final_task.metrics.retries;
        report.escalations = final_task.metrics.escalations;
        report.tokens_per_second = (final_task.model_eval_ms > 0.0)
            .then(|| 1000.0 * final_task.generated_tokens as f64 / final_task.model_eval_ms);
        let changes = final_task
            .snapshot_id
            .as_ref()
            .and_then(|sid| {
                child
                    .store
                    .get::<snapshots::Snapshot>("snapshots", sid)
                    .ok()
            })
            .and_then(|s| snapshots::deltas(&child.store, &s).ok())
            .unwrap_or_default();
        let current_manifest = final_task
            .snapshot_id
            .as_ref()
            .and_then(|sid| snapshots::working_path(&child.store, sid).ok())
            .and_then(|path| ConfinedRoot::open(&path).ok())
            .and_then(|root| snapshots::manifest(&root).ok())
            .map(|files| snapshots::manifest_hash(&files));
        report.passed = final_task.state == "review_ready"
            && calibration_files_unchanged(&child, &final_task)
            && changes.len() == 1
            && changes[0].path == "math.cjs"
            && final_task.checks.iter().any(|c| {
                c.status == "passed"
                    && Some(c.manifest_hash.as_str()) == current_manifest.as_deref()
            })
            && std::fs::read_to_string(source.join("math.cjs"))? == MATH;
        report.message = if report.passed {
            if report.full_gpu == Some(true) && report.free_gpu_bytes.is_some_and(|v|v>=models::GPU_RESERVE) { "Coding fixture passed with full GPU allocation and measured headroom." }
            else { "Coding fixture passed. Partial GPU/system RAM placement is supported; live available memory is checked before each load." }
        } else { "Coding fixture did not pass. Review the recorded checks and activity before using this model." }.into();
        report.evidence = crate::security::bounded(
            &crate::security::redact(&format!(
                "Result: {}\nChecks:\n{}\nActivity:\n{}",
                final_task.summary,
                final_task
                    .checks
                    .iter()
                    .map(|c| format!("{}: {}\n{}", c.name, c.status, c.output))
                    .collect::<Vec<_>>()
                    .join("\n"),
                final_task
                    .events
                    .iter()
                    .map(|e| format!("{}: {}", e.title, e.detail))
                    .collect::<Vec<_>>()
                    .join("\n")
            )),
            20000,
        );
        if cancel.is_cancelled() {
            return Err(AppError::new(
                "CANCELLED",
                "Model calibration stopped. Disposable fixture evidence was retained.",
            ));
        }
        self.save_calibration(&report)?;
        Ok(report)
    }
    fn save_calibration(&self, report: &ModelCalibration) -> Result<()> {
        self.store.put(
            "calibrations",
            &format!(
                "{}:{}:{}",
                report.digest, report.runtime, report.context_size
            ),
            report,
        )
    }
}

fn calibration_files_unchanged(core: &AppCore, task: &Task) -> bool {
    let Some(sid) = &task.snapshot_id else {
        return false;
    };
    let Ok(path) = snapshots::working_path(&core.store, sid) else {
        return false;
    };
    let Ok(root) = ConfinedRoot::open(Path::new(&path)) else {
        return false;
    };
    root.read("package.json", 10000)
        .is_ok_and(|v| v == PACKAGE.as_bytes())
        && root
            .read("test.cjs", 10000)
            .is_ok_and(|v| v == TEST.as_bytes())
        && root.list().is_ok_and(|(files, _)| files.len() == 3)
}
