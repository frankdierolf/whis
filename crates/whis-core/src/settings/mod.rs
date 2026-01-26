//! Application Settings Module
//!
//! This module provides a hierarchical settings structure organized by concern:
//!
//! # Architecture
//!
//! ```text
//! Settings (Aggregate Root)
//!   ├── Transcription  - Provider, API keys, local models
//!   ├── PostProcessing - LLM processor, prompts
//!   ├── Services       - Ollama, external services
//!   ├── Shortcuts      - CLI and Desktop keyboard shortcuts
//!   └── UI             - Clipboard, microphone, VAD, presets, bubble
//! ```
//!
//! # Usage
//!
//! ## CLI Usage
//!
//! ```rust,no_run
//! use whis_core::settings::Settings;
//!
//! // Load settings from CLI config file
//! let settings = Settings::load_cli();
//!
//! // Access nested settings
//! println!("Provider: {}", settings.transcription.provider);
//! println!("Post-processor: {:?}", settings.post_processing.processor);
//!
//! // Modify and save
//! let mut settings = settings;
//! settings.transcription.provider = whis_core::config::TranscriptionProvider::Mistral;
//! settings.save_cli().expect("Failed to save settings");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Desktop Usage
//!
//! Desktop uses Tauri's plugin-store for settings persistence. See whis-desktop
//! for implementation details.
//!
//! # File Locations
//!
//! - **CLI:** `~/.config/whis-cli/settings.json` with 0600 permissions
//! - **Desktop:** Managed by Tauri plugin-store

mod post_processing;
mod services;
mod shortcuts;
mod store_adapter;
mod transcription;
mod ui;

pub use post_processing::PostProcessingSettings;
pub use services::{OllamaConfig, ServicesSettings};
pub use shortcuts::{CliShortcutMode, ShortcutsSettings};
pub use transcription::{LocalModelsConfig, TranscriptionSettings};
pub use ui::{BubbleSettings, ModelMemorySettings, UiSettings, VadSettings};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application settings (aggregate root).
///
/// Settings are organized hierarchically by concern:
/// - `transcription`: Provider configuration and API keys
/// - `post_processing`: LLM post-processing settings
/// - `services`: External service configuration (Ollama, etc.)
/// - `shortcuts`: CLI and Desktop keyboard shortcuts
/// - `ui`: User interface preferences
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub transcription: TranscriptionSettings,
    pub post_processing: PostProcessingSettings,
    pub services: ServicesSettings,
    pub shortcuts: ShortcutsSettings,
    pub ui: UiSettings,
}

impl Settings {
    /// Get the CLI settings file path (~/.config/whis-cli/settings.json).
    pub fn cli_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("whis-cli")
            .join("settings.json")
    }

    /// Load settings from CLI config file.
    ///
    /// Returns default settings if file doesn't exist or cannot be parsed.
    /// On parse failure, creates a numbered backup (backup, backup.1, backup.2, etc.)
    /// to preserve the original file before defaults are applied.
    pub fn load_cli() -> Self {
        let path = Self::cli_path();
        if let Ok(content) = fs::read_to_string(&path) {
            match serde_json::from_str(&content) {
                Ok(settings) => return settings,
                Err(e) => {
                    eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                    eprintln!("Schema may have changed. Creating backup...");

                    // Create numbered backup (backup, backup.1, backup.2, etc.)
                    let backup_base = path.with_extension("json.backup");
                    let backup_path = if !backup_base.exists() {
                        backup_base
                    } else {
                        (1..)
                            .map(|n| path.with_extension(format!("json.backup.{}", n)))
                            .find(|p| !p.exists())
                            .unwrap_or(backup_base)
                    };

                    if let Err(backup_err) = fs::copy(&path, &backup_path) {
                        eprintln!("Failed to create backup: {}", backup_err);
                    } else {
                        eprintln!("Backup saved to: {}", backup_path.display());
                    }
                }
            }
        }
        Self::default()
    }

    /// Save settings to CLI config file with 0600 permissions.
    ///
    /// On Unix, creates the file with mode 0600 from the start to avoid
    /// a race condition where the file might briefly be world-readable.
    pub fn save_cli(&self) -> Result<()> {
        use std::io::Write;

        let path = Self::cli_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(content.as_bytes())?;
        }

        #[cfg(not(unix))]
        {
            fs::write(&path, &content)?;
        }

        Ok(())
    }

    /// Validate all settings.
    ///
    /// Checks that:
    /// - Transcription provider is properly configured
    /// - Post-processing (if enabled) has required credentials
    /// - Shortcuts don't conflict (CLI direct mode with same key as desktop)
    pub fn validate(&self) -> Result<()> {
        self.transcription.validate()?;
        self.post_processing
            .validate(&self.transcription.api_keys)?;
        self.shortcuts.validate()?;
        Ok(())
    }
}
