mod commands;
pub mod settings;
pub mod shortcuts;
mod state;
pub mod tray;
mod window;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Load settings from disk FIRST, before initializing state
            let loaded_settings = settings::Settings::load();
            app.manage(state::AppState::new(loaded_settings));

            // Initialize system tray (optional - may fail on tray-less environments)
            let tray_available = match tray::setup_tray(app) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!("Tray unavailable: {e}. Running in window mode.");
                    false
                }
            };

            // Setup global shortcuts (hybrid: Tauri plugin / Portal / CLI fallback)
            shortcuts::setup_shortcuts(app);

            // Start IPC listener for --toggle CLI commands
            shortcuts::start_ipc_listener(app.handle().clone());

            // If no tray, show main window immediately
            if !tray_available {
                window::show_main_window(app)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::is_api_configured,
            commands::get_settings,
            commands::save_settings,
            commands::shortcut_backend,
            commands::configure_shortcut,
            commands::configure_shortcut_with_trigger,
            commands::portal_shortcut,
            commands::validate_api_key,
            commands::reset_shortcut,
            commands::portal_bind_error,
            commands::get_toggle_command,
            commands::toggle_recording,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
