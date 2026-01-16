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

/// Floating bubble overlay position.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BubblePosition {
    /// Disabled (default)
    #[default]
    None,
    /// Top of screen
    Top,
    /// Center of screen
    Center,
    /// Bottom of screen
    Bottom,
}

/// Floating bubble overlay settings (experimental).
///
/// The bubble is a small floating indicator that shows
/// recording status. Desktop only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BubbleSettings {
    /// Enable floating bubble overlay.
    #[serde(default)]
    pub enabled: bool,

    /// Bubble position on screen.
    #[serde(default)]
    pub position: BubblePosition,

    /// Custom bubble position (x, y) set by user dragging.
    /// When set, overrides the `position` preset.
    #[serde(default)]
    pub custom_position: Option<(f64, f64)>,
}

impl Default for BubbleSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            position: BubblePosition::None,
            custom_position: None,
        }
    }
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
        }
    }
}
