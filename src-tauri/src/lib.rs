pub mod core;
pub mod modules;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            core::backup::create_restore_point,
            core::system::check_admin_rights,
            core::system::get_system_info,
            modules::performance::optimize_performance,
            modules::startup::audit_startup,
            modules::startup::preview_startup_cleanup,
            modules::startup::clean_startup,
            modules::startup::has_startup_snapshot,
            modules::startup::restore_startup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
