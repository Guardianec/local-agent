use crate::{agent::AppCore, domain::*, handoff};
use std::{path::PathBuf, sync::Arc};
use tauri::{State, WebviewWindow};

fn check(window: &WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err(AppError::new(
            "WINDOW_DENIED",
            "This window does not have permission to change tasks.",
        ));
    }
    Ok(())
}
#[tauri::command]
pub async fn install_browser(window: WebviewWindow, core: State<'_, Arc<AppCore>>) -> Result<()> {
    check(&window)?;
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || core.install_browser())
        .await
        .map_err(|e| {
            AppError::new("BROWSER_INSTALL", "Browser setup stopped unexpectedly.").detail(e)
        })?
}
#[tauri::command]
pub async fn bootstrap(window: WebviewWindow, core: State<'_, Arc<AppCore>>) -> Result<Bootstrap> {
    check(&window)?;
    core.bootstrap()
}
#[tauri::command]
pub async fn browser_workspace(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
) -> Result<Project> {
    check(&window)?;
    core.browser_workspace()
}
#[tauri::command]
pub async fn open_project(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
) -> Result<Option<Project>> {
    check(&window)?;
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Open project folder")
        .pick_folder()
        .await;
    if let Some(folder) = picked {
        let core = core.inner().clone();
        let path = folder.path().to_path_buf();
        Ok(Some(
            tokio::task::spawn_blocking(move || core.add_project(&path))
                .await
                .map_err(|e| {
                    AppError::new("PROJECT_OPEN", "The project could not be opened.").detail(e)
                })??,
        ))
    } else {
        Ok(None)
    }
}
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Named, versioned IPC arguments; no ambient authority.
pub async fn start_task(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    project_id: String,
    request: String,
    preset: String,
    model: Option<String>,
    mode: Mode,
    parent_task_id: Option<String>,
    inputs: Option<Vec<ContentBlock>>,
) -> Result<Task> {
    check(&window)?;
    core.inner().start_with_inputs(
        &project_id,
        &request,
        &preset,
        model,
        mode,
        parent_task_id,
        inputs.unwrap_or_default(),
    )
}
#[tauri::command]
pub async fn get_task(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<Task> {
    check(&window)?;
    core.store.get("tasks", &task_id)
}
#[tauri::command]
pub async fn cancel_task(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<()> {
    check(&window)?;
    core.cancel(&task_id)
}
#[tauri::command]
pub async fn answer_approval(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    task_id: String,
    approval_id: String,
    choice: String,
) -> Result<()> {
    check(&window)?;
    core.answer(&task_id, &approval_id, &choice)
}
#[tauri::command]
pub async fn get_review(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<Review> {
    check(&window)?;
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || core.review(&task_id))
        .await
        .map_err(|e| {
            AppError::new("REVIEW_FAILED", "The file review could not be prepared.").detail(e)
        })?
}
#[tauri::command]
pub async fn change_files(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    task_id: String,
    revision: u32,
    operation: String,
) -> Result<Task> {
    check(&window)?;
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || core.change_files(&task_id, revision, &operation))
        .await
        .map_err(|e| {
            AppError::new(
                "FILE_OPERATION",
                "The file operation stopped unexpectedly. Recovery information is retained.",
            )
            .detail(e)
        })?
}
#[tauri::command]
pub async fn runtime_status(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
) -> Result<RuntimeInfo> {
    check(&window)?;
    Ok(core.provider.health(&core.store).await)
}
#[tauri::command]
pub async fn probe_model(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    name: String,
) -> Result<ModelInfo> {
    check(&window)?;
    core.probe_model(&name).await
}
#[tauri::command]
pub async fn calibrate_model(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    name: String,
) -> Result<ModelCalibration> {
    check(&window)?;
    core.calibrate_model(&name).await
}
#[tauri::command]
pub async fn save_settings(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    mut settings: Settings,
) -> Result<Settings> {
    check(&window)?;
    if !matches!(settings.theme.as_str(), "dark" | "light" | "system")
        || !matches!(settings.context_size, 4096 | 8192 | 16384 | 32768)
        || [
            settings.intelligence.fast_context,
            settings.intelligence.balanced_context,
            settings.intelligence.strong_context,
        ]
        .iter()
        .any(|n| !matches!(n, 4096 | 8192 | 16384 | 32768))
    {
        return Err(AppError::new(
            "SETTINGS_INVALID",
            "Choose one of the available appearance and context options.",
        ));
    }
    let old = core.store.settings();
    for name in [
        &settings.model_profiles.fast,
        &settings.model_profiles.balanced,
        &settings.model_profiles.strong,
    ]
    .into_iter()
    .flatten()
    {
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err(AppError::new(
                "SETTINGS_INVALID",
                "Choose a valid installed model for each profile.",
            ));
        }
    }
    if settings.model_profiles.strong_enabled
        && (!old.model_profiles.strong_enabled
            || old.model_profiles.strong != settings.model_profiles.strong
            || old.intelligence.strong_context != settings.intelligence.strong_context)
    {
        let health = core.provider.health(&core.store).await;
        let ready = settings
            .model_profiles
            .strong
            .as_ref()
            .and_then(|name| {
                health
                    .models
                    .iter()
                    .find(|m| m.name == *name && m.local && m.capability == "verified")
            })
            .is_some_and(|model| {
                crate::models::strong_ready(
                    &health,
                    model,
                    crate::registry::context(&settings, "strong"),
                )
            });
        if !ready {
            return Err(AppError::new("MODEL_CALIBRATION_REQUIRED","Strong requires a passing coding benchmark at its selected context size. RAM offload is supported."));
        }
    }
    settings.model = settings.model_profiles.balanced.clone();
    if settings.node_path != old.node_path
        || settings.python_path != old.python_path
        || settings.git_path != old.git_path
    {
        return Err(AppError::new(
            "TOOL_PATH",
            "Choose executable tools using their file picker.",
        ));
    }
    core.store.put("settings", "main", &settings)?;
    Ok(settings)
}

#[tauri::command]
pub async fn model_catalog(window: WebviewWindow) -> Result<Vec<crate::registry::ModelSpec>> {
    check(&window)?;
    Ok(crate::registry::specs())
}
#[tauri::command]
pub async fn install_model(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    tier: String,
) -> Result<()> {
    check(&window)?;
    core.install_model(&tier).await
}
#[tauri::command]
pub async fn model_install_progress(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
) -> Result<Option<ModelInstallProgress>> {
    check(&window)?;
    Ok(core.store.get("services", "model-install").ok())
}
#[tauri::command]
pub async fn model_cleanup_candidates(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
) -> Result<Vec<CleanupCandidate>> {
    check(&window)?;
    core.cleanup_candidates().await
}
#[tauri::command]
pub async fn remove_obsolete_model(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    name: String,
) -> Result<()> {
    check(&window)?;
    core.remove_obsolete_model(&name).await
}
#[tauri::command]
pub async fn ingest_image(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    name: String,
    bytes_base64: String,
    source: String,
) -> Result<ImageAttachment> {
    use base64::Engine;
    check(&window)?;
    if core.busy() {
        return Err(AppError::new(
            "TASK_BUSY",
            "Wait for the current operation before attaching an image.",
        ));
    }
    if bytes_base64.len() > crate::images::MAX_IMAGE_BYTES * 4 / 3 + 8 {
        return Err(AppError::new("IMAGE_SIZE", "Choose an image up to 8 MiB."));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(bytes_base64)
        .map_err(|_| AppError::new("IMAGE_FORMAT", "The image data is invalid."))?;
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || crate::images::ingest(&core.store, &name, &bytes, &source))
        .await
        .map_err(|_| AppError::new("IMAGE_DECODE", "Image decoding stopped."))?
}
#[tauri::command]
pub async fn choose_images(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
) -> Result<Vec<ImageAttachment>> {
    use std::io::Read;
    check(&window)?;
    if core.busy() {
        return Err(AppError::new(
            "TASK_BUSY",
            "Wait for the current operation before attaching images.",
        ));
    }
    let files = rfd::AsyncFileDialog::new()
        .set_title("Attach local images")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_files()
        .await;
    let Some(files) = files else {
        return Ok(vec![]);
    };
    if files.len() > crate::images::MAX_IMAGES {
        return Err(AppError::new("IMAGE_COUNT", "Choose up to four images."));
    }
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for file in files {
            let mut bytes = Vec::new();
            std::fs::File::open(file.path())?
                .take(crate::images::MAX_IMAGE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            out.push(crate::images::ingest(
                &core.store,
                &file.file_name(),
                &bytes,
                "file",
            )?);
        }
        Ok(out)
    })
    .await
    .map_err(|_| AppError::new("IMAGE_DECODE", "Image decoding stopped."))?
}
#[tauri::command]
pub async fn image_preview(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    attachment_id: String,
) -> Result<String> {
    check(&window)?;
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || crate::images::preview(&core.store, &attachment_id))
        .await
        .map_err(|_| AppError::new("IMAGE_PREVIEW", "Image preview unavailable."))?
}
#[tauri::command]
pub async fn choose_tool(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    kind: String,
) -> Result<Settings> {
    check(&window)?;
    if !matches!(kind.as_str(), "node" | "python" | "git") {
        return Err(AppError::new("TOOL_KIND", "Choose a supported tool."));
    }
    let file = rfd::AsyncFileDialog::new()
        .set_title(format!("Choose {kind}.exe"))
        .add_filter("Windows executable", &["exe"])
        .pick_file()
        .await;
    let mut settings = core.store.settings();
    if let Some(file) = file {
        let path = file.path().canonicalize()?;
        if path.extension().is_none_or(|e| e != "exe") {
            return Err(AppError::new(
                "EXECUTABLE_INVALID",
                "Choose an installed .exe tool.",
            ));
        }
        let path = Some(path.to_string_lossy().into());
        match kind.as_str() {
            "node" => settings.node_path = path,
            "python" => settings.python_path = path,
            _ => settings.git_path = path,
        }
        core.store.put("settings", "main", &settings)?;
    }
    Ok(settings)
}
#[tauri::command]
pub async fn handoff_preview(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<HandoffPreview> {
    check(&window)?;
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || handoff::preview(&core.store, &task_id))
        .await
        .map_err(|e| {
            AppError::new("HANDOFF_FAILED", "The handoff could not be prepared.").detail(e)
        })?
}
#[tauri::command]
pub async fn export_handoff(
    window: WebviewWindow,
    core: State<'_, Arc<AppCore>>,
    preview_id: String,
) -> Result<Option<String>> {
    check(&window)?;
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Export handoff outside your project")
        .pick_folder()
        .await;
    if let Some(folder) = folder {
        let core = core.inner().clone();
        let path = folder.path().to_path_buf();
        Ok(Some(
            tokio::task::spawn_blocking(move || handoff::export(&core.store, &preview_id, &path))
                .await
                .map_err(|e| {
                    AppError::new("EXPORT_FAILED", "The handoff could not be exported.").detail(e)
                })??,
        ))
    } else {
        Ok(None)
    }
}
#[tauri::command]
pub async fn open_runtime_help(window: WebviewWindow, action: String) -> Result<()> {
    check(&window)?;
    let target = match action.as_str() {
        "download" => "https://ollama.com/download/windows",
        "models" => "https://ollama.com/library",
        "docs" => "https://docs.ollama.com/",
        _ => return Err(AppError::new("HELP_LINK", "This help page is unavailable.")),
    };
    use std::os::windows::process::CommandExt;
    let explorer = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or("C:\\Windows".into()))
        .join("explorer.exe");
    std::process::Command::new(explorer)
        .arg(target)
        .creation_flags(0x08000000)
        .spawn()?;
    Ok(())
}
