//! Whis Desktop - Tauri Application
//!
//! Desktop application for voice transcription with global keyboard shortcuts
//! and system tray integration.
//!
//! ## Architecture
//!
//! ```text
//! whis-desktop/
//! ├── bubble/        - Floating bubble overlay (experimental)
//! ├── commands/      - Tauri command handlers (30+ commands)
//! ├── recording/     - Recording orchestration & pipeline
//! ├── shortcuts/     - Global keyboard shortcuts (3 backends)
//! ├── tray/          - System tray UI & interactions
//! ├── state.rs       - Application state management
//! ├── window.rs      - Window utilities
//! ├── lib.rs         - Application entry point
//! └── main.rs        - CLI argument parsing
//! ```

pub mod bubble;
mod commands;
pub mod recording;
pub mod shortcuts;
mod state;
pub mod tray;
mod window;

use tauri::{Emitter, Manager};
use whis_core::{Settings, warn};

pub fn run(start_in_tray: bool) {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.contains(&"--start-in-tray".to_string()) {
                match app.get_webview_window("main") {
                    Some(window) => {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    None => {
                        let _ = window::show_main_window(app);
                    }
                }
            }
        }))
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            // Load settings from disk
            let loaded_settings = Settings::load();

            // Initialize state with tray availability
            app.manage(state::AppState::new(loaded_settings, true));

            // Initialize system tray (optional - may fail on tray-less environments)
            let _tray_available = match tray::setup_tray(app) {
                Ok(_) => true,
                Err(e) => {
                    warn!("Tray unavailable: {e}. Running in window mode.");
                    false
                }
            };

            // Initialize floating bubble window (hidden by default)
            if let Err(e) = bubble::create_bubble_window(app.handle()) {
                warn!("Bubble unavailable: {e}");
            }

            // Setup global shortcuts (hybrid: Tauri plugin / Portal / CLI fallback)
            shortcuts::setup_shortcuts(app);

            // Start IPC listener for --toggle CLI commands
            shortcuts::start_ipc_listener(app.handle().clone());

            // Only show main window if NOT starting in tray
            if !start_in_tray {
                window::show_main_window(app.handle())?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Prevent immediate close - emit event to frontend for graceful shutdown
                api.prevent_close();
                let _ = window.emit("window-close-requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            // System commands
            commands::get_toggle_command,
            commands::can_reopen_window,
            commands::list_audio_devices,
            commands::exit_app,
            commands::warmup_connections,
            // Validation commands
            commands::validate_openai_api_key,
            commands::validate_mistral_api_key,
            commands::validate_groq_api_key,
            commands::validate_deepgram_api_key,
            commands::validate_elevenlabs_api_key,
            // Recording commands
            commands::get_status,
            commands::is_api_configured,
            commands::toggle_recording,
            // Settings commands
            commands::get_settings,
            commands::save_settings,
            commands::check_config_readiness,
            commands::get_defaults,
            commands::get_cloud_providers,
            // Shortcut commands
            commands::shortcut_backend,
            commands::configure_shortcut,
            commands::configure_shortcut_with_trigger,
            commands::portal_shortcut,
            commands::reset_shortcut,
            commands::portal_bind_error,
            commands::rdev_grab_error,
            commands::check_input_group_membership,
            commands::open_keyboard_settings,
            commands::get_shortcut_instructions,
            commands::system_shortcut_from_dconf,
            commands::check_shortcut_path_mismatch,
            commands::update_shortcut_command,
            // Model commands
            commands::download_whisper_model,
            commands::get_whisper_models,
            commands::is_whisper_model_valid,
            commands::get_parakeet_models,
            commands::is_parakeet_model_valid,
            commands::download_parakeet_model,
            commands::get_active_download,
            // Preset commands
            commands::list_presets,
            commands::apply_preset,
            commands::get_active_preset,
            commands::set_active_preset,
            commands::get_preset_details,
            commands::create_preset,
            commands::update_preset,
            commands::delete_preset,
            // Ollama commands
            commands::test_ollama_connection,
            commands::list_ollama_models,
            commands::pull_ollama_model,
            commands::start_ollama,
            commands::check_ollama_status,
            // Bubble commands
            commands::bubble_toggle_recording,
            commands::bubble_get_position,
            commands::bubble_move_by,
            commands::bubble_save_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
