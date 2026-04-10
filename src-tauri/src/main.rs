#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            log::info!("Ingst application starting...");
            
            let window = app.get_webview_window("main").unwrap();
            window.set_title("Ingst").ok();
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ingst_lib::commands::scan_source,
            ingst_lib::commands::get_dest_info,
            ingst_lib::commands::build_ingest_plan,
            ingst_lib::commands::execute_ingest,
            ingst_lib::commands::cancel_ingest,
            ingst_lib::commands::pause_ingest,
            ingst_lib::commands::resume_ingest,
            ingst_lib::commands::get_settings,
            ingst_lib::commands::save_settings,
            ingst_lib::commands::get_mounted_volumes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
