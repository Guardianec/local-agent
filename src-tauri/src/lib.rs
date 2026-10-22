pub mod agent;
pub mod browser;
pub mod browser_session;
pub mod calibration;
pub mod capabilities;
pub mod commands;
pub mod continuation;
pub mod dev_server;
pub mod domain;
pub mod environment;
pub mod fs;
pub mod handoff;
pub mod images;
mod ipc;
pub mod lifecycle;
pub mod mcp;
mod mcp_http;
pub mod model_management;
pub mod models;
pub mod platform;
pub mod projects;
pub mod providers;
pub mod registry;
pub mod router;
pub mod security;
pub mod snapshots;
pub mod store;
pub mod tools;
pub mod web;

pub fn run() {
    use tauri::{Emitter, Manager};
    tauri::Builder::default()
        .setup(|app| {
            let path=app.path().app_local_data_dir()?;
            #[cfg(debug_assertions)]
            let path=std::env::var_os("LOCAL_AGENT_TEST_DATA").map(std::path::PathBuf::from).unwrap_or(path);
            let session=match platform::lock_instance(&path){Ok(lock)=>lock,Err(error)=>{rfd::MessageDialog::new().set_title("Local Agent").set_description(error.to_string()).set_level(rfd::MessageLevel::Info).show();return Err(Box::new(error));}};
            app.manage(session);
            let core=match agent::AppCore::open(&path){Ok(core)=>core,Err(error)=>{rfd::MessageDialog::new().set_title("Local Agent — recovery data preserved").set_description(format!("Local task storage could not be opened. No project files were changed.\n\nData folder: {}\n\n{}",path.display(),error)).set_level(rfd::MessageLevel::Error).show();return Err(Box::new(error));}};
            #[cfg(not(debug_assertions))]
            core.set_browser_source(app.path().resource_dir()?.join("browser-helper"));
            let handle=app.handle().clone();core.set_listener(std::sync::Arc::new(move|task|{let _=handle.emit_to("main","task-updated",task);}));
            app.manage(core);Ok(())
        })
        .invoke_handler(tauri::generate_handler![ipc::capability_catalog,ipc::integration_status,ipc::integrations,ipc::save_integration,ipc::remove_integration,ipc::test_integration,ipc::bootstrap,ipc::browser_workspace,ipc::open_project,ipc::start_task,ipc::get_task,ipc::cancel_task,ipc::answer_approval,ipc::get_review,ipc::change_files,ipc::runtime_status,ipc::probe_model,ipc::calibrate_model,ipc::save_settings,ipc::choose_tool,ipc::handoff_preview,ipc::export_handoff,ipc::open_runtime_help,ipc::install_browser,ipc::model_catalog,ipc::install_model,ipc::model_install_progress,ipc::model_cleanup_candidates,ipc::remove_obsolete_model,ipc::ingest_image,ipc::choose_images,ipc::image_preview])
        .run(tauri::generate_context!())
        .expect("Local Agent could not start");
}
pub mod interaction;
