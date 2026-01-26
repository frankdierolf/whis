//! System Utility Commands
//!
//! Provides Tauri commands for system-level operations like audio device listing,
//! CLI toggle command retrieval, window reopening checks, and app exit.

use crate::state::AppState;
use tauri::{AppHandle, State};
use whis_core::{AutotypeToolStatus, WarmupConfig, get_autotype_tool_status, warmup_configured};

/// Get the command to toggle recording from an external source (e.g., GNOME custom shortcut).
/// Returns the actual executable path so users can copy-paste into their compositor settings.
#[tauri::command]
pub fn get_toggle_command() -> String {
    // Flatpak: use flatpak run command (required for sandboxed apps)
    if std::path::Path::new("/.flatpak-info").exists() {
        return "flatpak run ink.whis.Whis --toggle".to_string();
    }

    // AppImage: use APPIMAGE env var (the actual .AppImage file path)
    // Note: current_exe() returns /tmp/.mount_*/usr/bin/... which is ephemeral
    if let Ok(appimage_path) = std::env::var("APPIMAGE") {
        return format!("{} --toggle", appimage_path);
    }

    // Native/dev builds: use actual executable path
    if let Ok(exe_path) = std::env::current_exe()
        && let Ok(canonical) = exe_path.canonicalize()
    {
        return format!("{} --toggle", canonical.display());
    }

    // Fallback (shouldn't happen, but provides a reasonable default)
    "whis-desktop --toggle".to_string()
}

/// Check if user can reopen the window after closing
/// Returns true if tray is available OR a working shortcut exists
#[tauri::command]
pub fn can_reopen_window(state: State<'_, AppState>) -> bool {
    // If tray is available, user can always reopen from there
    if *state.tray_available.lock().unwrap() {
        return true;
    }

    // Check shortcut backend - some always work, some need verification
    let backend_info = crate::shortcuts::backend_info();
    match backend_info.backend.as_str() {
        "TauriPlugin" => true, // X11 shortcuts always work
        "ManualSetup" => true, // IPC toggle always available
        "PortalGlobalShortcuts" => {
            // Portal needs a bound shortcut without errors
            let has_shortcut = state.portal_shortcut.lock().unwrap().is_some();
            let no_error = state.portal_bind_error.lock().unwrap().is_none();
            has_shortcut && no_error
        }
        _ => false,
    }
}

#[tauri::command]
pub fn list_audio_devices() -> Result<Vec<whis_core::AudioDeviceInfo>, String> {
    whis_core::list_audio_devices().map_err(|e| e.to_string())
}

/// Exit the application gracefully
/// Called after settings have been flushed to disk
#[tauri::command]
pub fn exit_app(app: AppHandle) {
    app.exit(0);
}

/// Warm up HTTP client and cloud connections based on current settings.
///
/// This should be called after the app is mounted to reduce latency
/// on the first transcription request. The warmup is best-effort and
/// will not block the UI.
#[tauri::command]
pub async fn warmup_connections(state: State<'_, AppState>) -> Result<(), String> {
    let (provider, provider_api_key, post_processor, post_processor_api_key) = {
        let settings = state.settings.lock().unwrap();

        // Get provider and its API key
        let provider = Some(settings.transcription.provider.to_string());
        let provider_api_key = settings.transcription.api_key_from_settings();

        // Get post-processor and its API key
        let post_processor = match &settings.post_processing.processor {
            whis_core::PostProcessor::None => None,
            p => Some(p.to_string()),
        };
        let post_processor_api_key = if post_processor.is_some() {
            settings
                .post_processing
                .api_key_from_settings(&settings.transcription.api_keys)
        } else {
            None
        };

        (
            provider,
            provider_api_key,
            post_processor,
            post_processor_api_key,
        )
    };

    // Build warmup config
    let config = WarmupConfig {
        provider,
        provider_api_key,
        post_processor,
        post_processor_api_key,
    };

    // Run warmup (best-effort, errors are logged but not propagated)
    warmup_configured(&config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get the status of autotyping tools on the system.
///
/// Returns information about which tools are available, which is recommended,
/// and installation instructions if needed.
#[tauri::command]
pub fn get_autotype_tool_status_cmd() -> AutotypeToolStatus {
    get_autotype_tool_status()
}
