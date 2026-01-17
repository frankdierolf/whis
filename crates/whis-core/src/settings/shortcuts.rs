//! Keyboard shortcuts configuration for CLI and Desktop applications.
//!
//! This module provides separate shortcut keys for CLI and Desktop to prevent
//! conflicts when both apps are running simultaneously.

use serde::{Deserialize, Serialize};

/// CLI keyboard shortcut triggering mode.
///
/// Determines how the CLI (`whis` command) listens for the recording hotkey.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CliShortcutMode {
    /// Desktop environment handles the hotkey.
    ///
    /// Configure a keyboard shortcut in your desktop settings
    /// (GNOME Settings → Keyboard → Shortcuts) to run: `whis toggle`
    #[default]
    System,

    /// CLI captures the hotkey directly.
    ///
    /// The CLI will listen for the global keyboard shortcut specified
    /// in `cli_key`. Requires input group membership on Linux.
    Direct,
}

impl CliShortcutMode {
    /// Returns the string representation for config display.
    pub fn as_str(&self) -> &'static str {
        match self {
            CliShortcutMode::System => "system",
            CliShortcutMode::Direct => "direct",
        }
    }
}

impl std::fmt::Display for CliShortcutMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CliShortcutMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "system" => Ok(CliShortcutMode::System),
            "direct" => Ok(CliShortcutMode::Direct),
            _ => Err(format!(
                "Invalid shortcut mode: '{}'. Use 'system' or 'direct'",
                s
            )),
        }
    }
}

fn default_shortcut() -> String {
    crate::configuration::DEFAULT_SHORTCUT.to_string()
}

/// Settings for keyboard shortcuts.
///
/// CLI and Desktop have separate shortcut keys to prevent conflicts
/// when both applications are running.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsSettings {
    /// CLI keyboard shortcut triggering mode.
    ///
    /// - `system`: Configure hotkey in desktop settings to run "whis toggle"
    /// - `direct`: CLI captures the hotkey globally (requires permissions)
    #[serde(default)]
    pub cli_mode: CliShortcutMode,

    /// CLI keyboard shortcut (e.g., "Ctrl+Alt+W").
    ///
    /// Only used when `cli_mode` is `direct`.
    #[serde(default = "default_shortcut")]
    pub cli_key: String,

    /// Desktop keyboard shortcut (e.g., "Ctrl+Alt+W").
    ///
    /// Used by whis-desktop application.
    #[serde(default = "default_shortcut")]
    pub desktop_key: String,

    /// Push-to-talk mode for CLI direct hotkey.
    ///
    /// When enabled, recording starts when the hotkey is pressed and stops
    /// when released. When disabled (default), the hotkey toggles recording.
    /// Only used when `cli_mode` is `direct`.
    #[serde(default)]
    pub cli_push_to_talk: bool,
}

impl Default for ShortcutsSettings {
    fn default() -> Self {
        Self {
            cli_mode: CliShortcutMode::default(),
            cli_key: default_shortcut(),
            desktop_key: default_shortcut(),
            cli_push_to_talk: false,
        }
    }
}

impl ShortcutsSettings {
    /// Validate shortcuts settings.
    ///
    /// Returns an error if CLI is in direct mode and both keys are the same,
    /// as this would cause both apps to trigger simultaneously.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.cli_mode == CliShortcutMode::Direct && self.cli_key == self.desktop_key {
            anyhow::bail!(
                "Shortcut conflict: CLI and Desktop cannot use '{}' when cli_mode is 'direct'.\n\
                 Fix with one of:\n\
                 - whis config cli-key <different-key>\n\
                 - whis config desktop-key <different-key>\n\
                 - whis config cli-mode system",
                self.cli_key
            );
        }
        Ok(())
    }
}
