mod commands;
pub mod settings;
pub mod shortcuts;
mod state;
pub mod tray;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Load settings from disk FIRST, before initializing state
            let loaded_settings = settings::Settings::load();
            app.manage(state::AppState::new(loaded_settings));

            // Initialize system tray
            tray::setup_tray(app)?;

            // Setup global shortcuts (hybrid: Tauri plugin / Portal / CLI fallback)
            shortcuts::setup_shortcuts(app);

            // Start IPC listener for --toggle CLI commands
            shortcuts::start_ipc_listener(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::check_config,
            commands::get_settings,
            commands::save_settings,
            commands::get_shortcut_backend,
            commands::configure_shortcut,
            commands::get_portal_shortcut,
            commands::validate_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
