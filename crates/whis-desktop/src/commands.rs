use crate::settings::Settings;
use crate::shortcuts::ShortcutBackendInfo;
use crate::state::{AppState, RecordingState};
use tauri::{AppHandle, State};
use whis_core::Config;

#[derive(serde::Serialize)]
pub struct StatusResponse {
    pub state: String,
    pub config_valid: bool,
}

#[derive(serde::Serialize)]
pub struct SaveSettingsResponse {
    pub needs_restart: bool,
}

#[tauri::command]
pub async fn check_config() -> Result<bool, String> {
    Config::from_env().map(|_| true).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusResponse, String> {
    let current_state = *state.state.lock().unwrap();
    let config_valid = state.config.lock().unwrap().is_some();

    Ok(StatusResponse {
        state: match current_state {
            RecordingState::Idle => "idle".to_string(),
            RecordingState::Recording => "recording".to_string(),
            RecordingState::Processing => "processing".to_string(),
        },
        config_valid,
    })
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let mut settings = state.settings.lock().unwrap();
    // Refresh from disk to ensure latest
    *settings = Settings::load();
    Ok(settings.clone())
}

#[tauri::command]
pub fn get_shortcut_backend() -> ShortcutBackendInfo {
    crate::shortcuts::get_backend_info()
}

#[tauri::command]
pub async fn configure_shortcut(app: AppHandle) -> Result<Option<String>, String> {
    crate::shortcuts::open_configure_shortcuts(app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_portal_shortcut(state: State<'_, AppState>) -> Result<Option<String>, String> {
    // First check if we have it cached in state
    let cached = state.portal_shortcut.lock().unwrap().clone();
    if cached.is_some() {
        return Ok(cached);
    }

    // Otherwise try reading from dconf (GNOME stores shortcuts there)
    Ok(crate::shortcuts::read_portal_shortcut_from_dconf())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<SaveSettingsResponse, String> {
    // Check what changed
    let (api_key_changed, shortcut_changed) = {
        let current = state.settings.lock().unwrap();
        (
            current.openai_api_key != settings.openai_api_key,
            current.shortcut != settings.shortcut,
        )
    };

    {
        let mut state_settings = state.settings.lock().unwrap();
        *state_settings = settings.clone();
        state_settings.save().map_err(|e| e.to_string())?;
    }

    // Clear cached config if API key changed
    if api_key_changed {
        *state.config.lock().unwrap() = None;
    }

    // Only update shortcut if it actually changed
    let needs_restart = if shortcut_changed {
        crate::shortcuts::update_shortcut(&app, &settings.shortcut)
            .map_err(|e| e.to_string())?
    } else {
        false
    };

    Ok(SaveSettingsResponse { needs_restart })
}

#[tauri::command]
pub fn validate_api_key(api_key: String) -> Result<bool, String> {
    // Validate format: OpenAI keys start with "sk-"
    if api_key.is_empty() {
        return Ok(true); // Empty is valid (will fall back to env var)
    }

    if !api_key.starts_with("sk-") {
        return Err("Invalid key format. OpenAI keys start with 'sk-'".to_string());
    }

    Ok(true)
}