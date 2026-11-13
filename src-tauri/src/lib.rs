mod commands;
mod demo;
mod state;

use state::AppState;
use tauri::Manager;
use wotslog_core::credentials::KeyringStore;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage(AppState::new(
                dir.join("provider.json"),
                std::sync::Arc::new(KeyringStore::new("dev.wotslog.ai", "ai-api-key")),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::select_log,
            commands::load_demo,
            commands::scan_log,
            commands::cancel_scan,
            commands::query_logs,
            commands::prepare_analysis,
            commands::analyze_error,
            commands::demo_analysis,
            commands::save_ai_settings,
            commands::get_ai_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running WotsLog AI");
}
