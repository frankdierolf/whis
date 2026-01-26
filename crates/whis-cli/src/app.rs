use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{IsTerminal, Write};
use std::thread;
use std::time::Duration;
use whis_core::{Settings, TranscriptionProvider};

/// Configuration for transcription, including provider, API key, and language
pub struct TranscriptionConfig {
    pub provider: TranscriptionProvider,
    pub api_key: String,
    pub language: Option<String>,
}

/// Load transcription config with optional language override
pub fn load_transcription_config_with_language(
    language_override: Option<String>,
) -> Result<TranscriptionConfig> {
    // Check if settings file exists (fresh install detection)
    let settings_path = Settings::cli_path();
    let is_fresh_install = !settings_path.exists();

    let settings = Settings::load_cli();
    let provider = settings.transcription.provider.clone();

    // Use override if provided, otherwise use configured language
    let language = language_override.or_else(|| settings.transcription.language.clone());

    // Handle different provider types:
    // - Cloud providers: require API key
    // - LocalWhisper: requires model path
    let api_key = match &provider {
        TranscriptionProvider::LocalWhisper => {
            // Local whisper: use model path
            match settings.transcription.whisper_model_path() {
                Some(path) => path,
                None => {
                    eprintln!("Error: No whisper model path configured.");
                    eprintln!("(Required for local Whisper transcription)");
                    eprintln!("\nSet the model path with:");
                    eprintln!(
                        "  whis config --whisper-model-path ~/.local/share/whis/models/ggml-small.bin\n"
                    );
                    eprintln!("Or set the LOCAL_WHISPER_MODEL_PATH environment variable.");
                    eprintln!("\nTip: Run 'whis setup local' for guided setup.");
                    std::process::exit(1);
                }
            }
        }
        TranscriptionProvider::LocalParakeet => {
            // Local parakeet: use model path
            match settings.transcription.parakeet_model_path() {
                Some(path) => path,
                None => {
                    eprintln!("Error: No parakeet model path configured.");
                    eprintln!("(Required for local Parakeet transcription)");
                    eprintln!("\nSet the model path with:");
                    eprintln!(
                        "  whis config --parakeet-model-path ~/.local/share/whis/models/parakeet/parakeet-tdt-0.6b-v3-int8\n"
                    );
                    eprintln!("Or set the LOCAL_PARAKEET_MODEL_PATH environment variable.");
                    eprintln!("\nTip: Run 'whis setup local' for guided setup.");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            // Cloud providers: require API key
            match settings.transcription.api_key_for(&provider) {
                Some(key) => key,
                None => {
                    if is_fresh_install {
                        // Fresh install: suggest running setup
                        eprintln!("Error: No transcription provider configured.");
                        eprintln!("\nRun 'whis setup' to get started.");
                    } else {
                        // Configured but missing key for current provider
                        eprintln!("Error: No {} API key configured.", provider.display_name());
                        eprintln!("(Required for {} transcription)", provider.display_name());
                        eprintln!("\nSet your key with:");
                        eprintln!(
                            "  whis config {}-api-key YOUR_KEY\n",
                            provider.as_str().to_lowercase().replace('_', "-")
                        );
                        eprintln!(
                            "Or set the {} environment variable.",
                            provider.api_key_env_var()
                        );
                    }
                    std::process::exit(1);
                }
            }
        }
    };

    Ok(TranscriptionConfig {
        provider,
        api_key, // For local-whisper this is model path
        language,
    })
}

/// Load transcription config using configured language
pub fn load_transcription_config() -> Result<TranscriptionConfig> {
    load_transcription_config_with_language(None)
}

/// Wait for user to stop recording via Enter key.
/// In TTY mode: waits for Enter key press.
/// In non-TTY mode: blocks indefinitely (use --duration for timed recording).
pub fn wait_for_stop() -> Result<()> {
    std::io::stdout().flush()?;

    if std::io::stdin().is_terminal() {
        // TTY mode: wait for Enter key
        enable_raw_mode()?;

        loop {
            // Check for Enter key with timeout (50ms polling)
            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key_event) = event::read()?
                && key_event.code == KeyCode::Enter
            {
                break;
            }
        }

        disable_raw_mode()?;
    } else {
        // Non-TTY mode: wait indefinitely
        // Use --duration for timed recording in non-interactive environments
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    }

    Ok(())
}

/// Print text with a typewriter effect
/// When delay_ms is 0, prints instantly (no animation)
pub fn typewriter(text: &str, delay_ms: u64) {
    if delay_ms == 0 {
        print!("{}", text);
        std::io::stdout().flush().ok();
        return;
    }
    for c in text.chars() {
        print!("{}", c);
        std::io::stdout().flush().ok();
        thread::sleep(Duration::from_millis(delay_ms));
    }
}

/// Print status message with optional typewriter animation
///
/// Automatically disables animation for real-time providers or when --verbose is used.
/// This ensures status messages don't slow down the perceived performance of fast providers.
/// In verbose mode, prints on its own line to avoid interleaving with verbose messages.
pub fn print_status(message: &str, provider: Option<&TranscriptionProvider>) {
    if whis_core::verbose::is_verbose() {
        // Verbose mode: print on own line to avoid interleaving
        println!("{}", message.trim());
        return;
    }

    let use_animation = !is_realtime_provider(provider);

    if use_animation {
        // Typewriter animation for batch providers (25ms per character)
        typewriter(message, 25);
    } else {
        // Instant output for real-time providers (no newline, continues on same line)
        print!("{}", message);
        std::io::stdout().flush().ok();
    }
}

/// Check if provider is a real-time streaming variant
///
/// Real-time providers are so fast (~150ms) that the typewriter animation (475ms for
/// " Transcribing...") actually makes the output feel slower. For these providers,
/// we use instant status messages to match their speed.
fn is_realtime_provider(provider: Option<&TranscriptionProvider>) -> bool {
    matches!(
        provider,
        Some(TranscriptionProvider::OpenAIRealtime) | Some(TranscriptionProvider::DeepgramRealtime)
    )
}
