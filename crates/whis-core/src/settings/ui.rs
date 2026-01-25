//! User interface and recording settings.
//!
//! This module contains settings for:
//! - Audio recording configuration (microphone, VAD, chunking)
//! - Output handling (clipboard backend, presets)
//! - Desktop-specific features (floating bubble overlay)
//!
//! Note: Keyboard shortcuts are now in the `shortcuts` module.

use serde::{Deserialize, Serialize};

#[cfg(feature = "clipboard")]
use crate::clipboard::ClipboardMethod;

#[cfg(feature = "autotyping")]
use crate::autotyping::{AutotypeBackend, OutputMethod};

/// Settings for UI behavior and device configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    /// Clipboard backend for pasting transcriptions.
    ///
    /// - `auto`: Auto-detect the best option for your system (recommended)
    ///   - Flatpak: uses wl-copy
    ///   - X11: uses xclip
    ///   - Wayland: uses arboard
    /// - `xclip`: Force X11 xclip (for X11 systems)
    /// - `wl-copy`: Force Wayland wl-copy (for Wayland systems)
    /// - `arboard`: Force cross-platform Rust clipboard library
    ///
    /// Change this if transcription pasting doesn't work correctly.
    #[cfg(feature = "clipboard")]
    #[serde(default)]
    pub clipboard_backend: ClipboardMethod,

    /// Selected microphone device name.
    ///
    /// - `null`: Use system default microphone
    /// - `"Device Name"`: Use specific microphone by name
    ///
    /// Run `whis setup` to see available devices and select one.
    #[serde(default)]
    pub microphone_device: Option<String>,

    /// Voice Activity Detection (VAD) settings.
    ///
    /// When enabled, whis will skip silence during recording,
    /// reducing transcription time and improving accuracy.
    #[serde(default)]
    pub vad: VadSettings,

    /// Currently active output preset name.
    ///
    /// Presets define post-processing transformations like
    /// "professional email", "casual chat", etc.
    ///
    /// - `null`: No preset active (raw transcription)
    /// - `"preset_name"`: Apply named preset from ~/.config/whis/presets/
    #[serde(default)]
    pub active_preset: Option<String>,

    /// Audio chunk duration for progressive transcription (seconds).
    ///
    /// During recording, audio is split into chunks and transcribed
    /// progressively. This setting controls chunk size:
    ///
    /// - Lower (30s): Faster perceived response, but less context
    /// - Default (90s): Good balance of speed and accuracy
    /// - Higher (120s+): Better accuracy for complex speech
    ///
    /// Valid range: 10-300 seconds
    #[serde(default = "default_chunk_duration")]
    pub chunk_duration_secs: u64,

    /// Floating bubble overlay settings (desktop only).
    ///
    /// Shows a small floating indicator during recording.
    /// Experimental feature.
    #[serde(default)]
    pub bubble: BubbleSettings,

    /// Model memory management settings.
    ///
    /// Controls when local transcription models are loaded/unloaded.
    /// Helps balance transcription speed vs memory usage.
    #[serde(default)]
    pub model_memory: ModelMemorySettings,

    /// Output method for transcribed text.
    ///
    /// - `clipboard`: Copy to clipboard only (default, current behavior)
    /// - `autotype`: Type directly into active window
    /// - `both`: Both copy to clipboard and autotype to window
    ///
    /// Autotype simulates keyboard input to paste text directly.
    /// Useful when clipboard pasting doesn't work (e.g., some terminals).
    #[cfg(feature = "autotyping")]
    #[serde(default)]
    pub output_method: OutputMethod,

    /// Backend for autotyping text into the active window.
    ///
    /// - `auto`: Auto-detect based on platform (recommended)
    ///   - Wayland: uses wtype/dotool/ydotool (external tools)
    ///   - X11: uses xdotool/ydotool (external tools)
    ///   - macOS/Windows: uses enigo (pure Rust)
    /// - `tools`: Force external CLI tools (Linux only)
    /// - `enigo`: Force cross-platform input simulation
    ///
    /// Only used when `output_method` is `autotype` or `both`.
    #[cfg(feature = "autotyping")]
    #[serde(default)]
    pub autotype_backend: AutotypeBackend,

    /// Delay between keystrokes when autotyping to window (milliseconds).
    ///
    /// Some applications drop input if keys are sent too fast.
    /// Set this to add a delay between each character.
    ///
    /// - `null`: No delay (fastest, works for most apps)
    /// - `10-50`: Slight delay for slower apps
    /// - `100+`: For very slow input handlers
    #[cfg(feature = "autotyping")]
    #[serde(default)]
    pub autotype_delay_ms: Option<u32>,
}

fn default_chunk_duration() -> u64 {
    crate::configuration::DEFAULT_CHUNK_DURATION_SECS
}

/// Voice Activity Detection configuration.
///
/// VAD automatically detects speech and skips silence,
/// which can significantly reduce transcription time
/// for recordings with pauses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadSettings {
    /// Enable Voice Activity Detection.
    ///
    /// When enabled, silence is skipped during recording,
    /// reducing the amount of audio sent for transcription.
    #[serde(default)]
    pub enabled: bool,

    /// Speech probability threshold (0.0-1.0).
    ///
    /// - Lower (0.3): More sensitive, may include background noise
    /// - Default (0.5): Balanced sensitivity
    /// - Higher (0.7): Less sensitive, may cut off soft speech
    ///
    /// Adjust if VAD is cutting off speech or including too much silence.
    #[serde(default)]
    pub threshold: f32,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            enabled: crate::configuration::DEFAULT_VAD_ENABLED,
            threshold: crate::configuration::DEFAULT_VAD_THRESHOLD,
        }
    }
}

/// Floating bubble overlay settings (experimental).
///
/// The bubble is a small floating indicator that shows
/// recording status. Desktop only.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BubbleSettings {
    /// Enable floating bubble overlay.
    #[serde(default)]
    pub enabled: bool,

    /// Custom bubble position (x, y) set by user dragging.
    #[serde(default)]
    pub custom_position: Option<(f64, f64)>,
}

/// Model memory management settings.
///
/// Controls when local transcription models (Whisper/Parakeet) are
/// loaded and unloaded from memory. These settings help balance
/// transcription speed vs memory usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMemorySettings {
    /// Keep transcription model loaded between recordings.
    ///
    /// When true, the model stays in RAM/VRAM between recordings for
    /// faster subsequent transcriptions (no ~3s reload delay).
    /// When false, model is unloaded after each transcription to free memory.
    ///
    /// Default: true (matches CLI daemon behavior for fast UX)
    #[serde(default = "default_keep_model_loaded")]
    pub keep_model_loaded: bool,

    /// Auto-unload after N minutes of inactivity.
    ///
    /// Only applies when `keep_model_loaded` is true.
    /// After this many minutes without a recording, the model is
    /// automatically unloaded to free memory.
    ///
    /// - 0: Never auto-unload (keep loaded until app closes)
    /// - 5, 10, 30, 60: Unload after idle timeout
    ///
    /// Default: 10 minutes
    #[serde(default = "default_unload_after_minutes")]
    pub unload_after_minutes: u32,
}

fn default_keep_model_loaded() -> bool {
    crate::configuration::DEFAULT_KEEP_MODEL_LOADED
}

fn default_unload_after_minutes() -> u32 {
    crate::configuration::DEFAULT_MODEL_UNLOAD_MINUTES
}

impl Default for ModelMemorySettings {
    fn default() -> Self {
        Self {
            keep_model_loaded: default_keep_model_loaded(),
            unload_after_minutes: default_unload_after_minutes(),
        }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            #[cfg(feature = "clipboard")]
            clipboard_backend: ClipboardMethod::default(),
            microphone_device: None,
            vad: VadSettings::default(),
            active_preset: None,
            chunk_duration_secs: crate::configuration::DEFAULT_CHUNK_DURATION_SECS,
            bubble: BubbleSettings::default(),
            model_memory: ModelMemorySettings::default(),
            #[cfg(feature = "autotyping")]
            output_method: OutputMethod::default(),
            #[cfg(feature = "autotyping")]
            autotype_backend: AutotypeBackend::default(),
            #[cfg(feature = "autotyping")]
            autotype_delay_ms: None,
        }
    }
}
