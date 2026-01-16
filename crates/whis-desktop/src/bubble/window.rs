//! Bubble Window Creation and Positioning
//!
//! Creates the floating bubble overlay window with platform-specific configuration.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

/// Bubble window dimensions
const BUBBLE_SIZE: f64 = 48.0;

/// Offset from screen edges
const BUBBLE_OFFSET: f64 = 50.0;

/// Create the bubble overlay window (hidden by default)
pub fn create_bubble_window(app: &AppHandle) -> Result<(), String> {
    // Calculate initial position (will be updated when shown)
    let (x, y) = calculate_bubble_position(app).unwrap_or((100.0, 100.0));

    let window = WebviewWindowBuilder::new(
        app,
        "bubble",
        WebviewUrl::App("src/bubble/index.html".into()),
    )
    .title("Whis Bubble")
    .position(x, y)
    .inner_size(BUBBLE_SIZE, BUBBLE_SIZE)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;

    // Note: Do NOT call gtk_window.set_titlebar(None) here - it breaks window
    // positioning on Wayland. Transparency works via .transparent(true) alone.
    let _ = window;

    Ok(())
}

/// Calculate bubble position based on settings.
///
/// # Custom Position
///
/// If the user has dragged the bubble to a custom position, that position
/// is used. Otherwise, defaults to bottom-center of the work area.
///
/// # Known Limitation (Linux/Dev Mode)
///
/// Bubble positioning may not work correctly when running via `cargo tauri dev`.
/// This is due to Tauri/GTK window positioning limitations on Linux:
/// - Wayland does not support programmatic window positioning by design
/// - Dev mode initializes GTK differently than production builds
///
/// The bubble position works correctly when installed via `just install-desktop`.
///
/// Related Tauri issues:
/// - <https://github.com/tauri-apps/tauri/issues/7376> (Linux positioning)
/// - <https://github.com/tauri-apps/tauri/issues/12411> (Wayland limitations)
pub fn calculate_bubble_position(app: &AppHandle) -> Result<(f64, f64), String> {
    let state = app.state::<AppState>();

    // Use custom position if set by user dragging
    if let Some((x, y)) = state.with_settings(|s| s.ui.bubble.custom_position) {
        return Ok((x, y));
    }

    let monitor = app
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No primary monitor")?;

    // Use work_area instead of size - respects taskbars/panels
    let work_area = monitor.work_area();
    let scale = monitor.scale_factor();
    let work_area_width = work_area.size.width as f64 / scale;
    let work_area_height = work_area.size.height as f64 / scale;
    let work_area_x = work_area.position.x as f64 / scale;
    let work_area_y = work_area.position.y as f64 / scale;

    // Default to bottom-center position
    let x = work_area_x + (work_area_width - BUBBLE_SIZE) / 2.0;
    let y = work_area_y + work_area_height - BUBBLE_SIZE - BUBBLE_OFFSET;

    Ok((x, y))
}
