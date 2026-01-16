use tauri::{command, AppHandle, Emitter, Runtime};

use crate::models::*;
use crate::FloatingBubbleExt;
use crate::Result;

/// Emit an event to the app, converting Tauri errors to our error type.
fn emit_event<R: Runtime>(app: &AppHandle<R>, event: &str, action: &str) -> Result<()> {
    app.emit(event, serde_json::json!({ "action": action }))
        .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))
}

/// Show the floating bubble overlay.
#[command]
pub(crate) async fn show_bubble<R: Runtime>(
    app: AppHandle<R>,
    options: Option<BubbleOptions>,
) -> Result<()> {
    app.floating_bubble().show(options.unwrap_or_default())
}

/// Hide the floating bubble overlay.
#[command]
pub(crate) async fn hide_bubble<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.floating_bubble().hide()
}

/// Check if the floating bubble is currently visible.
#[command]
pub(crate) async fn is_bubble_visible<R: Runtime>(app: AppHandle<R>) -> Result<VisibilityResponse> {
    app.floating_bubble().is_visible()
}

/// Request the overlay permission (SYSTEM_ALERT_WINDOW).
#[command]
pub(crate) async fn request_overlay_permission<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PermissionResponse> {
    app.floating_bubble().request_permission()
}

/// Check if the overlay permission (SYSTEM_ALERT_WINDOW) is granted.
#[command]
pub(crate) async fn has_overlay_permission<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PermissionResponse> {
    app.floating_bubble().has_permission()
}

/// Request the microphone permission (RECORD_AUDIO).
#[command]
pub(crate) async fn request_microphone_permission<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PermissionResponse> {
    app.floating_bubble().request_microphone_permission()
}

/// Check if the microphone permission (RECORD_AUDIO) is granted.
#[command]
pub(crate) async fn has_microphone_permission<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PermissionResponse> {
    app.floating_bubble().has_microphone_permission()
}

/// Set the bubble's visual state.
#[command]
pub(crate) async fn set_bubble_state<R: Runtime>(app: AppHandle<R>, state: String) -> Result<()> {
    app.floating_bubble().set_state(state)
}

/// Handle bubble click event from Android service (works when WebView inactive).
///
/// This is called via `trigger()` from Kotlin, bypassing WebView JavaScript.
/// Emits the same event that the WebView-based click handler would emit.
#[command]
pub(crate) async fn handle_bubble_click<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    emit_event(&app, "floating-bubble://click", "click")
}

/// Handle bubble close event from Android service (works when WebView inactive).
///
/// This is called via `trigger()` from Kotlin, bypassing WebView JavaScript.
/// Emits the same event that the WebView-based close handler would emit.
#[command]
pub(crate) async fn handle_bubble_close<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    emit_event(&app, "floating-bubble://close", "close")
}
