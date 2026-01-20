//! Cross-platform hotkey support (push-to-talk mode)
//!
//! - Linux/macOS: Uses rdev for keyboard grab (supports X11, Wayland, and macOS)
//! - Windows: Uses global-hotkey crate (Tauri-maintained)
//!
//! Push-to-talk: Recording starts when hotkey is pressed, stops when released.

use anyhow::Result;
use tokio::sync::mpsc::UnboundedReceiver;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_like;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use unix_like as platform;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;

/// Hotkey events for push-to-talk mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// Hotkey was pressed - start recording
    Pressed,
    /// Hotkey was released - stop recording
    Released,
}

/// Opaque guard that keeps the hotkey listener alive
#[allow(dead_code)]
pub struct HotkeyGuard(platform::HotkeyGuard);

/// Setup the hotkey listener for push-to-talk mode.
/// Returns a receiver for hotkey press/release events and a guard that must be kept alive.
pub fn setup(hotkey_str: &str) -> Result<(UnboundedReceiver<HotkeyEvent>, HotkeyGuard)> {
    let (rx, guard) = platform::setup(hotkey_str)?;
    Ok((rx, HotkeyGuard(guard)))
}

/// Validate a hotkey string and return normalized form if valid
///
/// Examples of valid hotkeys: "ctrl+alt+w", "super+shift+r", "cmd+option+w"
pub fn validate(hotkey_str: &str) -> Result<String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use whis_core::hotkey::Hotkey;
        let hotkey = Hotkey::parse(hotkey_str).map_err(|e| anyhow::anyhow!(e))?;
        Ok(hotkey.to_normalized_string())
    }
    #[cfg(target_os = "windows")]
    {
        // Windows validation via global-hotkey
        windows::validate(hotkey_str)
    }
}
