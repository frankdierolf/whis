//! Setup wizard for whis configuration
//!
//! Provides a streamlined setup experience through interactive prompts.
//!
//! # Wizard Flow
//!
//! ```text
//! ┌─────────────────────────┐
//! │ How do you want to      │
//! │ transcribe?             │
//! │  ├─► Cloud ──► cloud.rs │
//! │  └─► Local ──► local.rs │
//! └───────────┬─────────────┘
//!             ▼
//! ┌─────────────────────────┐
//! │ Configure post-         │
//! │ processing?             │
//! │  └─► post_processing.rs │
//! └───────────┬─────────────┘
//!             ▼
//! ┌─────────────────────────┐
//! │ Audio device + Shortcut │
//! │ (this module)           │
//! └─────────────────────────┘
//! ```
//!
//! # Sub-modules
//!
//! - `cloud` - Cloud provider API key setup
//! - `local` - Local model (Whisper/Parakeet) selection
//! - `post_processing` - Ollama or cloud LLM configuration
//! - `interactive` - UI helpers (prompts, selection menus)
//! - `provider_helpers` - Provider metadata (URLs, descriptions)

mod cloud;
mod interactive;
mod local;
mod post_processing;
mod provider_helpers;

use anyhow::Result;
use whis_core::settings::CliShortcutMode;
use whis_core::{Settings, TranscriptionProvider};

use crate::hotkey;

pub fn run() -> Result<()> {
    setup_wizard()
}

/// Unified setup wizard - guides user through all configuration
fn setup_wizard() -> Result<()> {
    let settings = Settings::load_cli();

    // Default to current provider type (Local if using local, else Cloud)
    let default = match settings.transcription.provider {
        TranscriptionProvider::LocalParakeet | TranscriptionProvider::LocalWhisper => 1,
        _ => 0,
    };

    let items = vec!["Cloud", "Local"];
    let choice = interactive::select("How do you want to transcribe?", &items, Some(default))?;

    let is_cloud = match choice {
        0 => {
            cloud::setup_transcription_cloud()?;
            true
        }
        1 => {
            local::setup_transcription_local()?;
            false
        }
        _ => unreachable!(),
    };

    post_processing::setup_post_processing_step(is_cloud)?;

    // Audio device selection step
    setup_audio_device_step()?;

    // Shortcut setup step
    setup_shortcut_step()?;

    interactive::info("Configuration saved! Run 'whis' to record and transcribe.");

    Ok(())
}

/// Setup shortcut mode (system or direct)
fn setup_shortcut_step() -> Result<()> {
    let mut settings = Settings::load_cli();

    let items = vec!["System shortcut", "Direct capture"];
    let default = if settings.shortcuts.cli_mode == CliShortcutMode::Direct {
        1
    } else {
        0
    };
    let choice = interactive::select("Recording trigger?", &items, Some(default))?;

    match choice {
        0 => {
            // System mode
            settings.shortcuts.cli_mode = CliShortcutMode::System;
            settings.save_cli()?;

            interactive::info("Add a shortcut in desktop settings: whis toggle");
        }
        1 => {
            // Direct capture mode
            settings.shortcuts.cli_mode = CliShortcutMode::Direct;

            // Get and validate hotkey
            let default_shortcut = &settings.shortcuts.cli_key;
            let normalized = loop {
                let input = interactive::input("Hotkey?", Some(default_shortcut))?;

                match hotkey::validate(&input) {
                    Ok(normalized) => {
                        settings.shortcuts.cli_key = input;
                        settings.save_cli()?;
                        break normalized;
                    }
                    Err(_) => {
                        interactive::error("Invalid hotkey. Examples: ctrl+alt+w, super+shift+r");
                        continue;
                    }
                }
            };

            // Check input group membership on Linux
            #[cfg(target_os = "linux")]
            {
                if is_in_input_group() {
                    interactive::info(&format!("Hotkey {} ready", normalized));
                } else {
                    interactive::info(&format!("Valid: {}", normalized));
                    interactive::error("You need permission first:");
                    interactive::info("  sudo usermod -aG input $USER");
                    interactive::info("Then logout and login again.");
                }
            }

            #[cfg(not(target_os = "linux"))]
            interactive::info(&format!("Hotkey {} ready", normalized));
        }
        _ => unreachable!(),
    }

    Ok(())
}

/// Setup audio device (microphone) selection
fn setup_audio_device_step() -> Result<()> {
    use whis_core::list_audio_devices;

    let devices = match list_audio_devices() {
        Ok(d) if !d.is_empty() => d,
        _ => {
            // No devices found - skip this step silently
            return Ok(());
        }
    };

    // Build selection list with "System Default" as first option
    let mut items: Vec<String> = vec!["System Default".to_string()];
    for device in &devices {
        // Use display_name if available, otherwise fall back to raw name
        let name = device.display_name.as_ref().unwrap_or(&device.name);
        items.push(name.to_string());
    }

    // Find current selection for default highlight
    let mut settings = Settings::load_cli();
    let default_idx = settings
        .ui
        .microphone_device
        .as_ref()
        .and_then(|current| devices.iter().position(|d| &d.name == current))
        .map(|i| i + 1)
        .unwrap_or(0);

    let choice = interactive::select("Microphone?", &items, Some(default_idx))?;

    settings.ui.microphone_device = if choice == 0 {
        None
    } else {
        // Store the raw name (for device lookup), not display name
        Some(devices[choice - 1].name.clone())
    };

    settings.save_cli()?;
    Ok(())
}

/// Check if current user is in the 'input' group (Linux only)
#[cfg(target_os = "linux")]
fn is_in_input_group() -> bool {
    use std::process::Command;

    Command::new("groups")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("input"))
        .unwrap_or(false)
}
