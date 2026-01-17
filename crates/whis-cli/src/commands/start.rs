use crate::{app, hotkey, ipc, service};
use anyhow::Result;
use whis_core::Settings;
use whis_core::settings::CliShortcutMode;

pub fn run() -> Result<()> {
    // Check if service is already running
    if ipc::is_service_running() {
        eprintln!("Error: whis service is already running.");
        eprintln!("Use 'whis stop' to stop the existing service first.");
        std::process::exit(1);
    }

    // Load settings and transcription configuration
    let settings = Settings::load();
    let config = app::load_transcription_config()?;

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
            match hotkey::setup(shortcut) {
                Ok((hotkey_rx, _guard)) => {
                    if push_to_talk {
                        println!(
                            "Listening. Hold {} to record (push-to-talk). Ctrl+C to stop.",
                            shortcut
                        );
                    } else {
                        println!(
                            "Listening. Press {} to toggle recording. Ctrl+C to stop.",
                            shortcut
                        );
                    }

                    runtime.block_on(async {
                        let service = service::Service::new(config)?;
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
            println!("Listening. Press your configured shortcut to record. Ctrl+C to stop.");

            runtime.block_on(async {
                let service = service::Service::new(config)?;
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
