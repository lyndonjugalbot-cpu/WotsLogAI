// Every app command gets an explicit `allow-<command>` permission; capabilities grant only those.
const COMMANDS: &[&str] = &[
    "select_log",
    "load_demo",
    "scan_log",
    "cancel_scan",
    "query_logs",
    "prepare_analysis",
    "analyze_error",
    "demo_analysis",
    "save_ai_settings",
    "get_ai_settings",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build failed");
}
