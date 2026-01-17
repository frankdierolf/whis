//! Windows hotkey support using global-hotkey crate (push-to-talk mode)

use anyhow::Result;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use std::sync::mpsc::Receiver;

use super::HotkeyEvent;

pub struct HotkeyGuard {
    _manager: GlobalHotKeyManager,
}

pub fn setup(hotkey_str: &str) -> Result<(Receiver<HotkeyEvent>, HotkeyGuard)> {
    let converted = convert_to_global_hotkey_format(hotkey_str)?;
    let hotkey: HotKey = converted
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid hotkey '{}': {:?}", hotkey_str, e))?;

    let manager = GlobalHotKeyManager::new()
        .map_err(|e| anyhow::anyhow!("Failed to create hotkey manager: {:?}", e))?;

    manager.register(hotkey.clone()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to register hotkey '{}': {:?}\n\n\
            This may mean the hotkey is already registered by another application.",
            hotkey_str,
            e
        )
    })?;

    let receiver = GlobalHotKeyEvent::receiver().clone();
    let hotkey_id = hotkey.id();
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        loop {
            if let Ok(event) = receiver.recv() {
                if event.id() == hotkey_id {
                    let hotkey_event = match event.state() {
                        HotKeyState::Pressed => HotkeyEvent::Pressed,
                        HotKeyState::Released => HotkeyEvent::Released,
                    };
                    let _ = tx.send(hotkey_event);
                }
            }
        }
    });

    Ok((rx, HotkeyGuard { _manager: manager }))
}

/// Convert our hotkey format to global-hotkey format
///
/// Input: "ctrl+alt+w" (our format)
/// Output: "Ctrl+Alt+KeyW" (global-hotkey format)
fn convert_to_global_hotkey_format(s: &str) -> Result<String> {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();

    if parts.is_empty() {
        anyhow::bail!("Empty hotkey string");
    }

    let mut result = Vec::new();
    let mut has_main_key = false;

    for part in parts {
        let lower = part.to_lowercase();
        let converted = match lower.as_str() {
            // Modifiers
            "ctrl" | "control" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "super" | "meta" | "win" | "cmd" => "Super".to_string(),

            // Single letters -> KeyX format
            key if key.len() == 1 && key.chars().next().unwrap().is_ascii_alphabetic() => {
                has_main_key = true;
                format!("Key{}", key.to_uppercase())
            }

            // Numbers -> DigitX format
            key if key.len() == 1 && key.chars().next().unwrap().is_ascii_digit() => {
                has_main_key = true;
                format!("Digit{}", key)
            }

            // Function keys
            "f1" => {
                has_main_key = true;
                "F1".to_string()
            }
            "f2" => {
                has_main_key = true;
                "F2".to_string()
            }
            "f3" => {
                has_main_key = true;
                "F3".to_string()
            }
            "f4" => {
                has_main_key = true;
                "F4".to_string()
            }
            "f5" => {
                has_main_key = true;
                "F5".to_string()
            }
            "f6" => {
                has_main_key = true;
                "F6".to_string()
            }
            "f7" => {
                has_main_key = true;
                "F7".to_string()
            }
            "f8" => {
                has_main_key = true;
                "F8".to_string()
            }
            "f9" => {
                has_main_key = true;
                "F9".to_string()
            }
            "f10" => {
                has_main_key = true;
                "F10".to_string()
            }
            "f11" => {
                has_main_key = true;
                "F11".to_string()
            }
            "f12" => {
                has_main_key = true;
                "F12".to_string()
            }

            // Special keys
            "space" => {
                has_main_key = true;
                "Space".to_string()
            }
            "enter" | "return" => {
                has_main_key = true;
                "Enter".to_string()
            }
            "escape" | "esc" => {
                has_main_key = true;
                "Escape".to_string()
            }
            "tab" => {
                has_main_key = true;
                "Tab".to_string()
            }
            "backspace" => {
                has_main_key = true;
                "Backspace".to_string()
            }
            "delete" | "del" => {
                has_main_key = true;
                "Delete".to_string()
            }
            "insert" | "ins" => {
                has_main_key = true;
                "Insert".to_string()
            }
            "home" => {
                has_main_key = true;
                "Home".to_string()
            }
            "end" => {
                has_main_key = true;
                "End".to_string()
            }
            "pageup" | "pgup" => {
                has_main_key = true;
                "PageUp".to_string()
            }
            "pagedown" | "pgdn" => {
                has_main_key = true;
                "PageDown".to_string()
            }
            "up" => {
                has_main_key = true;
                "ArrowUp".to_string()
            }
            "down" => {
                has_main_key = true;
                "ArrowDown".to_string()
            }
            "left" => {
                has_main_key = true;
                "ArrowLeft".to_string()
            }
            "right" => {
                has_main_key = true;
                "ArrowRight".to_string()
            }

            _ => anyhow::bail!("Unknown key: {}", part),
        };
        result.push(converted);
    }

    if !has_main_key {
        anyhow::bail!("No main key specified in hotkey");
    }

    Ok(result.join("+"))
}

/// Validate a hotkey string and return normalized form if valid
pub fn validate(hotkey_str: &str) -> Result<String> {
    // Try to convert to global-hotkey format (validates syntax)
    let converted = convert_to_global_hotkey_format(hotkey_str)?;

    // Try to parse as HotKey to validate it's a valid combination
    let _: HotKey = converted
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid hotkey '{}': {:?}", hotkey_str, e))?;

    // Return a normalized form
    Ok(converted)
}
