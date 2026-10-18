use local_agent_core::{domain::*, models, platform, providers::Ollama, registry, store::Store};

#[tokio::main]
async fn main() -> Result<()> {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let target = std::env::args()
        .nth(1)
        .ok_or_else(|| AppError::new("ARGUMENT", "Supply the explicit target app-data folder."))?;
    let path = std::path::Path::new(&target);
    let _lock = platform::lock_instance(path)?;
    let store = Store::open(path)?;
    if store.list::<Task>("tasks")?.iter().any(Task::active) {
        return Err(AppError::new("TASK_ACTIVE","An interrupted or running task needs review before profile setup. No preferences were changed."));
    }
    let verified = Store::open(&workspace.join(".local-data/model-verification-v03"))?;
    let provider = Ollama::new()?;
    let health = provider.health(&verified).await;
    let mut settings = store.settings();
    settings.intelligence = verified.settings().intelligence;
    for model in &health.models {
        if let Some(report) = models::calibration(
            &health,
            model,
            registry::identity(&model.name)
                .map(|s| registry::context(&settings, &s.tier))
                .unwrap_or(settings.context_size),
        ) {
            store.put(
                "calibrations",
                &format!(
                    "{}:{}:{}",
                    report.digest, report.runtime, report.context_size
                ),
                report,
            )?;
            if model.capability == "verified" {
                let key = format!(
                    "{}:{}",
                    model.digest,
                    health.version.as_deref().unwrap_or("")
                );
                let probe: serde_json::Value = verified.get("probes", &key)?;
                store.put("probes", &key, &probe)?;
            }
        }
    }
    let usable = |name: &str| {
        health
            .models
            .iter()
            .find(|m| m.name == name && m.local && m.capability == "verified")
            .filter(|m| {
                models::calibration(
                    &health,
                    m,
                    registry::identity(&m.name)
                        .map(|s| registry::context(&settings, &s.tier))
                        .unwrap_or(settings.context_size),
                )
                .is_some_and(|c| c.passed)
            })
            .map(|m| m.name.clone())
    };
    let balanced = usable(&registry::spec("balanced").unwrap().model_id).ok_or_else(|| {
        AppError::new(
            "CALIBRATION_REQUIRED",
            "Balanced has not passed; settings were preserved.",
        )
    })?;
    let strong = Some(registry::spec("strong").unwrap().model_id);
    let enabled = strong
        .as_ref()
        .and_then(|name| health.models.iter().find(|m| m.name == *name))
        .is_some_and(|m| models::strong_ready(&health, m, registry::context(&settings, "strong")));
    settings.model_profiles = ModelProfiles {
        fast: usable(&registry::spec("fast").unwrap().model_id),
        balanced: Some(balanced.clone()),
        strong,
        strong_enabled: enabled,
    };
    settings.model = Some(balanced);
    store.put("settings", "main", &settings)?;
    if let Some(project_path) = std::env::args().nth(2) {
        let core = local_agent_core::agent::AppCore::open(path)?;
        core.add_project(std::path::Path::new(&project_path))?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&settings.model_profiles)?
    );
    Ok(())
}
