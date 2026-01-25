use crate::{app, hotkey, ipc, service};
use anyhow::Result;
use whis_core::Settings;
use whis_core::autotyping::OutputMethod;
use whis_core::settings::CliShortcutMode;
use whis_core::{Preset, resolve_post_processor_config};

pub fn run(autotype: bool, preset_name: Option<String>) -> Result<()> {
    // Check if service is already running
    if ipc::is_service_running() {
        eprintln!("Error: whis service is already running.");
        eprintln!("Use 'whis stop' to stop the existing service first.");
        std::process::exit(1);
    }

    // Load settings and transcription configuration
    let settings = Settings::load();

    // Determine output method override from CLI flag
    let output_method_override = if autotype {
        Some(OutputMethod::Autotype)
    } else {
        None
    };
    let config = app::load_transcription_config()?;

    // Load preset if specified
    let preset = preset_name
        .map(|name| Preset::load(&name).map(|(p, _source)| p))
        .transpose()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Validate post-processing configuration early (catches missing Ollama model, etc.)
    if preset.is_some() || settings.post_processing.enabled {
        resolve_post_processor_config(&preset, &settings)?;
    }

    // Create Tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;

    // Validate shortcuts before starting (check for conflicts with Desktop)
    settings.shortcuts.validate()?;

    // Based on cli_mode, decide how to run
    match settings.shortcuts.cli_mode {
        CliShortcutMode::Direct => {
            // Try to set up hotkey via evdev/rdev
            let shortcut = &settings.shortcuts.cli_key;
            let push_to_talk = settings.shortcuts.cli_push_to_talk;
            let output_method = output_method_override
                .as_ref()
                .unwrap_or(&settings.ui.output_method);
            match hotkey::setup(shortcut) {
                Ok((hotkey_rx, _guard)) => {
                    if push_to_talk {
                        println!(
                            "Listening. Hold {} to record (push-to-talk). Output: {}. Ctrl+C to stop.",
                            shortcut, output_method
                        );
                    } else {
                        println!(
                            "Listening. Press {} to toggle recording. Output: {}. Ctrl+C to stop.",
                            shortcut, output_method
                        );
                    }

                    runtime.block_on(async {
                        let service =
                            service::Service::new(config, preset, output_method_override)?;
                        tokio::select! {
                            result = service.run(Some(hotkey_rx), push_to_talk) => result,
                            _ = tokio::signal::ctrl_c() => {
                                println!("\nShutting down...");
                                Ok(())
                            }
                        }
                    })
                }
                Err(e) => {
                    eprintln!("Error: Could not set up hotkey: {}", e);
                    eprintln!();
                    eprintln!("To use direct hotkey capture, run:");
                    eprintln!("  sudo usermod -aG input $USER");
                    eprintln!("Then logout and login again.");
                    eprintln!();
                    eprintln!("Or switch to system mode:");
                    eprintln!("  whis config cli-mode system");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            // "system" mode (or any other value) - IPC only
            let output_method = output_method_override
                .as_ref()
                .unwrap_or(&settings.ui.output_method);
            println!(
                "Listening. Press your configured shortcut to record. Output: {}. Ctrl+C to stop.",
                output_method
            );

            runtime.block_on(async {
                let service = service::Service::new(config, preset, output_method_override)?;
                tokio::select! {
                    result = service.run(None, false) => result,
                    _ = tokio::signal::ctrl_c() => {
                        println!("\nShutting down...");
                        Ok(())
                    }
                }
            })
        }
    }
}
