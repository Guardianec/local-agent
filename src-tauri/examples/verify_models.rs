use local_agent_core::{agent::AppCore, domain::*};

#[tokio::main]
async fn main() -> Result<()> {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let data = workspace.join(".local-data/model-verification-v03");
    let core = AppCore::open(&data)?;
    let mut settings = core.store.settings();
    settings.context_size = 8192;
    settings.model_profiles = ModelProfiles {
        fast: Some(local_agent_core::registry::spec("fast").unwrap().model_id),
        balanced: Some(
            local_agent_core::registry::spec("balanced")
                .unwrap()
                .model_id,
        ),
        strong: Some(local_agent_core::registry::spec("strong").unwrap().model_id),
        strong_enabled: false,
    };
    core.store.put("settings", "main", &settings)?;
    let names = std::env::args().skip(1).collect::<Vec<_>>();
    for name in names {
        println!("Calibrating {name} on a disposable built-in fixture...");
        match core.calibrate_model(&name).await {
            Ok(report) => println!(
                "{}",
                serde_json::to_string(
                    &serde_json::json!({"model":report.model,"passed":report.passed,"seconds":report.elapsed_ms/1000.0,"tokensPerSecond":report.tokens_per_second,"fullGpu":report.full_gpu,"allocationGiB":report.allocation_bytes.map(|v|v/1073741824.0),"freeGpuGiB":report.free_gpu_bytes.map(|v|v/1073741824.0),"message":report.message})
                )?
            ),
            Err(error) => println!("{}: {}", name, error),
        }
        let reports: Vec<ModelCalibration> = core.store.list("calibrations")?;
        std::fs::write(
            data.join("results.json"),
            serde_json::to_vec_pretty(&reports)?,
        )?;
    }
    Ok(())
}
