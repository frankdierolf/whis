//! Preset Management Commands
//!
//! Provides Tauri commands for managing transcription presets (built-in and user-created).
//! Presets contain predefined prompts and post-processing configurations.

use super::save_settings_to_store;
use crate::state::AppState;
use tauri::{AppHandle, State};
use whis_core::preset::{Preset, PresetSource};

/// Preset info for the UI
#[derive(serde::Serialize)]
pub struct PresetInfo {
    pub name: String,
    pub description: String,
    pub is_builtin: bool,
}

/// Full preset details for editing
#[derive(serde::Serialize)]
pub struct PresetDetails {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub post_processor: Option<String>,
    pub model: Option<String>,
    pub is_builtin: bool,
}

/// Input for creating a new preset
#[derive(serde::Deserialize)]
pub struct CreatePresetInput {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub post_processor: Option<String>,
    pub model: Option<String>,
}

/// Input for updating an existing preset
#[derive(serde::Deserialize)]
pub struct UpdatePresetInput {
    pub description: String,
    pub prompt: String,
    pub post_processor: Option<String>,
    pub model: Option<String>,
}

/// List all available presets (built-in + user)
#[tauri::command]
pub fn list_presets() -> Vec<PresetInfo> {
    Preset::list_all()
        .into_iter()
        .map(|(p, source)| PresetInfo {
            name: p.name,
            description: p.description,
            is_builtin: source == PresetSource::BuiltIn,
        })
        .collect()
}

/// Apply a preset - updates settings with the preset's configuration and sets it as active
#[tauri::command]
pub async fn apply_preset(
    app: AppHandle,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (preset, _) = Preset::load(&name)?;

    let settings_clone = {
        let mut settings = state.settings.lock().unwrap();

        // Apply preset's post-processing prompt
        settings.post_processing.prompt = Some(preset.prompt.clone());

        // Apply preset's post-processor override if specified
        if let Some(post_processor_str) = &preset.post_processor
            && let Ok(post_processor) = post_processor_str.parse()
        {
            settings.post_processing.processor = post_processor;
        }

        // Set this preset as active
        settings.ui.active_preset = Some(name);

        settings.clone()
    };

    // Save the settings to store
    save_settings_to_store(&app, &settings_clone)?;

    // Clear cached transcription config since settings changed
    *state.transcription_config.lock().unwrap() = None;

    Ok(())
}

/// Get the active preset name (if any)
#[tauri::command]
pub fn get_active_preset(state: State<'_, AppState>) -> Option<String> {
    let settings = state.settings.lock().unwrap();
    settings.ui.active_preset.clone()
}

/// Set the active preset
#[tauri::command]
pub async fn set_active_preset(
    app: AppHandle,
    name: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_clone = {
        let mut settings = state.settings.lock().unwrap();
        settings.ui.active_preset = name;
        settings.clone()
    };
    save_settings_to_store(&app, &settings_clone)?;
    Ok(())
}

/// Get full details of a preset for viewing/editing
#[tauri::command]
pub fn get_preset_details(name: String) -> Result<PresetDetails, String> {
    let (preset, source) = Preset::load(&name)?;

    Ok(PresetDetails {
        name: preset.name,
        description: preset.description,
        prompt: preset.prompt,
        post_processor: preset.post_processor,
        model: preset.model,
        is_builtin: source == PresetSource::BuiltIn,
    })
}

/// Create a new user preset
#[tauri::command]
pub fn create_preset(input: CreatePresetInput) -> Result<PresetInfo, String> {
    // Validate name
    Preset::validate_name(&input.name, false)?;

    // Check if preset already exists
    if Preset::load(&input.name).is_ok() {
        return Err(format!("A preset named '{}' already exists", input.name));
    }

    // Create and save the preset
    let preset = Preset {
        name: input.name.clone(),
        description: input.description.clone(),
        prompt: input.prompt,
        post_processor: input.post_processor,
        model: input.model,
    };

    preset.save()?;

    Ok(PresetInfo {
        name: input.name,
        description: input.description,
        is_builtin: false,
    })
}

/// Update an existing user preset
#[tauri::command]
pub fn update_preset(name: String, input: UpdatePresetInput) -> Result<PresetInfo, String> {
    // Check it's not a built-in
    if Preset::is_builtin(&name) {
        return Err(format!("Cannot edit built-in preset '{}'", name));
    }

    // Check preset exists
    let (mut preset, _) = Preset::load(&name)?;

    // Update fields
    preset.description = input.description.clone();
    preset.prompt = input.prompt;
    preset.post_processor = input.post_processor;
    preset.model = input.model;

    // Save
    preset.save()?;

    Ok(PresetInfo {
        name,
        description: input.description,
        is_builtin: false,
    })
}

/// Delete a user preset
#[tauri::command]
pub fn delete_preset(
    app: AppHandle,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Delete the preset file
    Preset::delete(&name)?;

    // If this was the active preset, clear it
    let settings_clone = {
        let mut settings = state.settings.lock().unwrap();
        if settings.ui.active_preset.as_deref() == Some(&name) {
            settings.ui.active_preset = None;
            Some(settings.clone())
        } else {
            None
        }
    };

    if let Some(settings) = settings_clone {
        save_settings_to_store(&app, &settings)?;
    }

    Ok(())
}
