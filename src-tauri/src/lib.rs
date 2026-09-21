pub mod app_schema;
mod commands;
pub mod mcp;
pub mod runtime;

use tauri::Manager;

use runtime::RuntimeState;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let base_dir = app.path().app_data_dir().expect("resolve app data dir");
            let state = RuntimeState::init(base_dir).expect("initialize runtime state");
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::install_app,
            commands::list_apps,
            commands::get_app,
            commands::update_app,
            commands::start_app,
            commands::stop_app,
            commands::uninstall_app,
            commands::write_app_file,
            commands::read_app_file,
            commands::delete_app_file,
            commands::list_app_files,
            commands::write_shared_file,
            commands::read_shared_file,
            commands::list_shared_files,
            commands::list_records,
            commands::create_record,
            commands::update_record,
            commands::delete_record,
            commands::backup_app,
            commands::backup_all_apps,
            commands::list_backups,
            commands::restore_app,
            commands::start_scheduler,
            commands::stop_scheduler,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
