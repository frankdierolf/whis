//! Shared hotkey parsing and matching logic for CLI and Desktop
//!
//! This module provides a unified `Hotkey` struct and parsing logic used by both
//! whis-cli and whis-desktop for global keyboard shortcuts.
//!
//! Note: AltGr (right Alt on international keyboards) is treated as a distinct key
//! from Alt. Hotkeys configured with "Alt" will only match the left Alt key.

use rdev::{Event, EventType, Key};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

/// Error type for hotkey parsing failures.
#[derive(Debug, Error)]
pub enum HotkeyParseError {
    #[error("Empty hotkey string")]
    Empty,
    #[error("No main key specified in hotkey")]
    NoMainKey,
    #[error("Unknown key: {0}")]
    UnknownKey(String),
}

/// Lock a mutex, recovering from poisoned state if needed.
///
/// This is useful for keyboard event handlers where we want to continue
/// processing even if a previous thread panicked while holding the lock.
pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Creates an rdev grab callback that tracks pressed keys and fires on hotkey match.
///
/// This is the shared implementation used by both CLI and Desktop for direct keyboard
/// capture on Linux. It handles:
/// - Tracking currently pressed keys
/// - Detecting when the hotkey combination is pressed (push-to-talk start)
/// - Detecting when the main key is released (push-to-talk stop)
/// - Preventing double-fire on key repeat (via triggered flag)
///
/// Returns `None` to consume the event (hotkey was triggered), `Some(event)` to pass through.
///
/// # Example
/// ```ignore
/// let callback = create_grab_callback(hotkey,
///     || println!("Hotkey pressed - start recording!"),
///     || println!("Hotkey released - stop recording!"),
/// );
/// rdev::grab(callback)?;
/// ```
pub fn create_grab_callback<FPress, FRelease>(
    hotkey: Hotkey,
    on_trigger: FPress,
    on_release: FRelease,
) -> impl Fn(Event) -> Option<Event> + Send
where
    FPress: Fn() + Send + 'static,
    FRelease: Fn() + Send + 'static,
{
    let pressed_keys: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
    let hotkey_triggered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let main_key = hotkey.key;

    move |event: Event| -> Option<Event> {
        match event.event_type {
            EventType::KeyPress(key) => {
                let mut keys = lock_or_recover(&pressed_keys);
                keys.insert(key);

                let mut triggered = lock_or_recover(&hotkey_triggered);
                if *triggered {
                    return Some(event); // Already triggered, pass through
                }

                if hotkey.is_pressed(&keys) {
                    *triggered = true;
                    on_trigger();
                    return None; // Consume event
                }
                Some(event)
            }
            EventType::KeyRelease(key) => {
                let mut keys = lock_or_recover(&pressed_keys);
                keys.remove(&key);

                if key == main_key {
                    let mut triggered = lock_or_recover(&hotkey_triggered);
                    if *triggered {
                        *triggered = false;
                        on_release();
                    }
                }
                Some(event)
            }
            _ => Some(event),
        }
    }
}

/// Macro to generate key string to rdev::Key mappings.
macro_rules! key_mappings {
    ($input:expr; $($name:pat => $key:ident),* $(,)?) => {
        match $input {
            $($name => Ok(Key::$key),)*
            other => Err(HotkeyParseError::UnknownKey(other.to_string())),
        }
    };
}

/// Macro to generate rdev::Key to string mappings.
macro_rules! key_to_str {
    ($key:expr; $($variant:ident => $name:expr),* $(,)?) => {
        match $key {
            $(Key::$variant => $name,)*
            _ => "?",
        }
    };
}

/// Represents a hotkey combination (modifiers + main key)
#[derive(Debug, Clone)]
pub struct Hotkey {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_key: bool,
    pub key: Key,
}

impl Hotkey {
    /// Parse a hotkey string like "ctrl+alt+w" or "Ctrl+Alt+KeyW" into a Hotkey
    ///
    /// Supported modifier aliases:
    /// - ctrl, control
    /// - shift
    /// - alt, option
    /// - super, meta, win, cmd
    pub fn parse(s: &str) -> Result<Self, HotkeyParseError> {
        let lower = s.to_lowercase();
        let parts: Vec<&str> = lower.split('+').map(|p| p.trim()).collect();

        if parts.is_empty() {
            return Err(HotkeyParseError::Empty);
        }

        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut super_key = false;
        let mut main_key: Option<Key> = None;

        for part in parts {
            match part {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" | "option" => alt = true,
                "super" | "meta" | "win" | "cmd" => super_key = true,
                key_str => {
                    main_key = Some(parse_key(key_str)?);
                }
            }
        }

        let key = main_key.ok_or(HotkeyParseError::NoMainKey)?;

        Ok(Hotkey {
            ctrl,
            shift,
            alt,
            super_key,
            key,
        })
    }

    /// Check if all required modifiers and the main key are currently pressed.
    ///
    /// This handles both left and right variants of modifier keys (e.g., ControlLeft/ControlRight).
    pub fn is_pressed(&self, pressed: &HashSet<Key>) -> bool {
        let ctrl_ok = !self.ctrl
            || pressed.contains(&Key::ControlLeft)
            || pressed.contains(&Key::ControlRight);
        let shift_ok =
            !self.shift || pressed.contains(&Key::ShiftLeft) || pressed.contains(&Key::ShiftRight);
        let alt_ok = !self.alt || pressed.contains(&Key::Alt);
        let super_ok = !self.super_key
            || pressed.contains(&Key::MetaLeft)
            || pressed.contains(&Key::MetaRight);
        let key_ok = pressed.contains(&self.key);

        ctrl_ok && shift_ok && alt_ok && super_ok && key_ok
    }

    /// Convert the hotkey to a normalized string representation.
    ///
    /// Returns modifiers in consistent order (Ctrl, Alt, Shift, Super)
    /// followed by the main key name.
    ///
    /// Examples:
    /// - "ctrl+alt+w" → "Ctrl+Alt+W"
    /// - "super+shift+r" → "Shift+Super+R"
    pub fn to_normalized_string(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.super_key {
            parts.push("Super");
        }
        parts.push(key_to_string(&self.key));
        parts.join("+")
    }
}

/// Parse a single key string into an rdev Key.
///
/// Handles both simple format ("w") and Tauri format ("keyw").
pub fn parse_key(s: &str) -> Result<Key, HotkeyParseError> {
    // Handle "KeyX" format from Tauri (e.g., "keyw" -> "w")
    let s = if s.starts_with("key") && s.len() == 4 {
        &s[3..] // Extract just the letter
    } else {
        s
    };

    key_mappings!(s;
        "a" => KeyA, "b" => KeyB, "c" => KeyC, "d" => KeyD, "e" => KeyE,
        "f" => KeyF, "g" => KeyG, "h" => KeyH, "i" => KeyI, "j" => KeyJ,
        "k" => KeyK, "l" => KeyL, "m" => KeyM, "n" => KeyN, "o" => KeyO,
        "p" => KeyP, "q" => KeyQ, "r" => KeyR, "s" => KeyS, "t" => KeyT,
        "u" => KeyU, "v" => KeyV, "w" => KeyW, "x" => KeyX, "y" => KeyY,
        "z" => KeyZ,
        "0" => Num0, "1" => Num1, "2" => Num2, "3" => Num3, "4" => Num4,
        "5" => Num5, "6" => Num6, "7" => Num7, "8" => Num8, "9" => Num9,
        "f1" => F1, "f2" => F2, "f3" => F3, "f4" => F4, "f5" => F5,
        "f6" => F6, "f7" => F7, "f8" => F8, "f9" => F9, "f10" => F10,
        "f11" => F11, "f12" => F12,
        "space" => Space,
        "enter" | "return" => Return,
        "escape" | "esc" => Escape,
        "tab" => Tab,
        "backspace" => Backspace,
        "delete" | "del" => Delete,
        "insert" | "ins" => Insert,
        "home" => Home,
        "end" => End,
        "pageup" | "pgup" => PageUp,
        "pagedown" | "pgdn" => PageDown,
        "up" => UpArrow,
        "down" => DownArrow,
        "left" => LeftArrow,
        "right" => RightArrow,
    )
}

/// Convert an rdev Key to its display string.
pub fn key_to_string(key: &Key) -> &'static str {
    key_to_str!(key;
        KeyA => "A", KeyB => "B", KeyC => "C", KeyD => "D", KeyE => "E",
        KeyF => "F", KeyG => "G", KeyH => "H", KeyI => "I", KeyJ => "J",
        KeyK => "K", KeyL => "L", KeyM => "M", KeyN => "N", KeyO => "O",
        KeyP => "P", KeyQ => "Q", KeyR => "R", KeyS => "S", KeyT => "T",
        KeyU => "U", KeyV => "V", KeyW => "W", KeyX => "X", KeyY => "Y",
        KeyZ => "Z",
        Num0 => "0", Num1 => "1", Num2 => "2", Num3 => "3", Num4 => "4",
        Num5 => "5", Num6 => "6", Num7 => "7", Num8 => "8", Num9 => "9",
        F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5",
        F6 => "F6", F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10",
        F11 => "F11", F12 => "F12",
        Space => "Space",
        Return => "Enter",
        Escape => "Escape",
        Tab => "Tab",
        Backspace => "Backspace",
        Delete => "Delete",
        Insert => "Insert",
        Home => "Home",
        End => "End",
        PageUp => "PageUp",
        PageDown => "PageDown",
        UpArrow => "Up",
        DownArrow => "Down",
        LeftArrow => "Left",
        RightArrow => "Right",
    )
}
