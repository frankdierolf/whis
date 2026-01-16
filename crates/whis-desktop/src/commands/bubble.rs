//! Bubble Overlay Commands
//!
//! Provides Tauri commands for bubble interactions.

use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Toggle recording from bubble click
/// Mirrors the tray toggle behavior
#[tauri::command]
pub async fn bubble_toggle_recording(app: AppHandle) -> Result<(), String> {
    crate::recording::toggle_recording(app);
    Ok(())
}

/// Get the current bubble window position
#[tauri::command]
pub fn bubble_get_position(app: AppHandle) -> Result<(f64, f64), String> {
    if let Some(window) = app.get_webview_window("bubble") {
        let pos = window.outer_position().map_err(|e| e.to_string())?;
        Ok((pos.x as f64, pos.y as f64))
    } else {
        Err("Bubble window not found".to_string())
    }
}

/// Move the bubble window by a delta (for drag operations)
#[tauri::command]
pub fn bubble_move_by(app: AppHandle, dx: f64, dy: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("bubble") {
        let pos = window.outer_position().map_err(|e| e.to_string())?;
        window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: pos.x + dx as i32,
                y: pos.y + dy as i32,
            }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Save the bubble's current position to settings
#[tauri::command]
pub fn bubble_save_position(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.with_settings_mut(|s| {
        s.ui.bubble.custom_position = Some((x, y));
    });
    // Persist to disk
    state.with_settings(|s| s.save());
    Ok(())
}
