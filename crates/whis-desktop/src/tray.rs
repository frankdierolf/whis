use crate::state::{AppState, RecordingState};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewWindowBuilder, WebviewUrl,
};
use whis_core::{
    copy_to_clipboard, parallel_transcribe, transcribe_audio, AudioRecorder, AudioResult, Config,
};

// Static icons for each state (pre-loaded at compile time)
const ICON_IDLE: &[u8] = include_bytes!("../icons/icon-idle.png");
const ICON_RECORDING: &[u8] = include_bytes!("../icons/icon-recording.png");
const ICON_PROCESSING: &[u8] = include_bytes!("../icons/icon-processing.png");

pub const TRAY_ID: &str = "whis-tray";


pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let record = MenuItem::with_id(app, "record", "Start Recording", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Whis", true, None::<&str>)?;

    // Store the record menu item for later updates
    if let Some(state) = app.try_state::<AppState>() {
        *state.record_menu_item.lock().unwrap() = Some(record.clone());
    }

    let menu = Menu::with_items(app, &[&record, &sep, &settings, &sep, &quit])?;

    // Use image crate for consistent rendering (same as set_tray_icon)
    let idle_bytes = include_bytes!("../icons/icon-idle.png");
    let img = image::load_from_memory(idle_bytes).expect("Failed to load idle icon");
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let idle_icon = Image::new_owned(rgba.into_raw(), width, height);

    // Use app cache dir for tray icons so Flatpak host can access them
    // (default /tmp is sandboxed and GNOME AppIndicator can't read it)
    let cache_dir = app.path().app_cache_dir().expect("Failed to get app cache dir");

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(idle_icon)
        .temp_dir_path(cache_dir)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Whis - Click to record")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "record" => {
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    toggle_recording(app_clone);
                });
            }
            "settings" => {
                open_settings_window(app.clone());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::TrayIconEvent;
            if let TrayIconEvent::Click { button, .. } = event {
                if button == tauri::tray::MouseButton::Left {
                    let app_handle = tray.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        toggle_recording(app_handle);
                    });
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn open_settings_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let window = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html".into())
    )
    .title("Whis Settings")
    .inner_size(600.0, 400.0)
    .min_inner_size(400.0, 300.0)
    .resizable(true)
    .decorations(false)
    .transparent(true)
    .build();

    // Fix Wayland window dragging by unsetting GTK titlebar
    // On Wayland, GTK's titlebar is required for dragging, but decorations(false)
    // removes it. By calling set_titlebar(None), we restore drag functionality
    // while keeping our custom chrome.
    #[cfg(target_os = "linux")]
    if let Ok(window) = window {
        use gtk::prelude::GtkWindowExt;
        if let Ok(gtk_window) = window.gtk_window() {
            gtk_window.set_titlebar(Option::<&gtk::Widget>::None);
        }
    }
}

fn toggle_recording(app: AppHandle) {
    let state = app.state::<AppState>();
    let current_state = *state.state.lock().unwrap();

    match current_state {
        RecordingState::Idle => {
            // Start recording
            if let Err(e) = start_recording_sync(&app, &state) {
                eprintln!("Failed to start recording: {e}");
            }
        }
        RecordingState::Recording => {
            // Stop recording and transcribe
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = stop_and_transcribe(&app_clone).await {
                    eprintln!("Failed to transcribe: {e}");
                }
            });
        }
        RecordingState::Processing => {
            // Already processing, ignore
        }
    }
}

fn start_recording_sync(app: &AppHandle, state: &AppState) -> Result<(), String> {
    // Load config if not already loaded
    {
        let mut config_guard = state.config.lock().unwrap();
        if config_guard.is_none() {
            // Try settings first, then environment variable
            let api_key = {
                let settings = state.settings.lock().unwrap();
                settings.openai_api_key.clone()
            }
            .or_else(|| std::env::var("OPENAI_API_KEY").ok());

            let api_key = api_key.ok_or(
                "No API key configured. Add it in Settings > API Keys.",
            )?;

            *config_guard = Some(Config { openai_api_key: api_key });
        }
    }

    // Start recording
    let mut recorder = AudioRecorder::new().map_err(|e| e.to_string())?;
    recorder.start_recording().map_err(|e| e.to_string())?;

    *state.recorder.lock().unwrap() = Some(recorder);
    *state.state.lock().unwrap() = RecordingState::Recording;

    // Update tray
    update_tray(app, RecordingState::Recording);
    println!("Recording started...");

    Ok(())
}

async fn stop_and_transcribe(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Update state to processing
    {
        *state.state.lock().unwrap() = RecordingState::Processing;
    }
    update_tray(app, RecordingState::Processing);
    println!("Processing...");

    // Get recorder and config
    let mut recorder = state
        .recorder
        .lock()
        .unwrap()
        .take()
        .ok_or("No active recording")?;

    let api_key = state
        .config
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Config not loaded")?
        .openai_api_key
        .clone();

    // Stop recording (synchronous file saving)
    // Note: AudioRecorder might need to be Send to be moved into async block? 
    // It is likely Send since it's in a Mutex.
    let audio_result = recorder.stop_and_save().map_err(|e| e.to_string())?;

    // Transcribe
    let transcription = match audio_result {
        // transcribe_audio is synchronous (blocking HTTP), so we should wrap it in spawn_blocking
        // to avoid blocking the async runtime
        AudioResult::Single(data) => {
            let api_key = api_key.clone();
            tauri::async_runtime::spawn_blocking(move || {
                transcribe_audio(&api_key, data)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
        },
        AudioResult::Chunked(chunks) => {
            // parallel_transcribe is async, so we can await it directly
            parallel_transcribe(&api_key, chunks, None)
                .await
                .map_err(|e| e.to_string())?
        }
    };

    // Copy to clipboard
    copy_to_clipboard(&transcription).map_err(|e| e.to_string())?;

    // Reset state
    {
        *state.state.lock().unwrap() = RecordingState::Idle;
    }
    update_tray(app, RecordingState::Idle);

    println!("Done: {}", &transcription[..transcription.len().min(50)]);

    Ok(())
}

fn update_tray(app: &AppHandle, new_state: RecordingState) {
    // Update menu item text using stored reference
    let app_state = app.state::<AppState>();
    if let Some(ref menu_item) = *app_state.record_menu_item.lock().unwrap() {
        let text = match new_state {
            RecordingState::Idle => "Start Recording",
            RecordingState::Recording => "Stop Recording",
            RecordingState::Processing => "Processing...",
        };
        let _ = menu_item.set_text(text);
        let _ = menu_item.set_enabled(new_state != RecordingState::Processing);
    }

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        // Update tooltip
        let tooltip = match new_state {
            RecordingState::Idle => "Whis - Click to record",
            RecordingState::Recording => "Whis - Recording... Click to stop",
            RecordingState::Processing => "Whis - Processing...",
        };
        let _ = tray.set_tooltip(Some(tooltip));

        // Set static icon based on state
        let icon = match new_state {
            RecordingState::Idle => ICON_IDLE,
            RecordingState::Recording => ICON_RECORDING,
            RecordingState::Processing => ICON_PROCESSING,
        };
        set_tray_icon(&tray, icon);
    }
}

fn set_tray_icon(tray: &tauri::tray::TrayIcon, icon_bytes: &[u8]) {
    match image::load_from_memory(icon_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            let icon = Image::new_owned(rgba.into_raw(), width, height);
            if let Err(e) = tray.set_icon(Some(icon)) {
                eprintln!("Failed to set tray icon: {e}");
            }
        }
        Err(e) => eprintln!("Failed to load tray icon: {e}"),
    }
}

/// Public wrapper for toggle_recording to be called from global shortcuts
pub fn toggle_recording_public(app: AppHandle) {
    toggle_recording(app);
}
