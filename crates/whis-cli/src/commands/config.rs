use anyhow::{Context, Result, anyhow};
use whis_core::defaults::{DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_URL};
use whis_core::settings::CliShortcutMode;
use whis_core::{PostProcessor, Preset, Settings, TranscriptionProvider};

use crate::ui::mask_key;

/// Supported configuration keys
const VALID_KEYS: &[&str] = &[
    "provider",
    "language",
    "openai-api-key",
    "mistral-api-key",
    "groq-api-key",
    "deepgram-api-key",
    "elevenlabs-api-key",
    "whisper-model-path",
    "parakeet-model-path",
    "post-processor",
    "post-processing-prompt",
    "ollama-url",
    "ollama-model",
    "microphone-device",
    "cli-mode",
    "cli-key",
    "cli-push-to-talk",
    "desktop-key",
    "vad",
    "vad-threshold",
    "chunk-size",
];

pub fn run(key: Option<String>, value: Option<String>, list: bool, path: bool) -> Result<()> {
    // Handle --path flag
    if path {
        println!("{}", Settings::cli_path().display());
        return Ok(());
    }

    // Handle --list flag
    if list {
        return show_all_settings();
    }

    // Handle get/set operations
    if let Some(key_str) = key {
        let key_normalized = key_str.to_lowercase();

        // Validate key
        if !VALID_KEYS.contains(&key_normalized.as_str()) {
            eprintln!("Error: Unknown configuration key '{}'", key_str);
            eprintln!();
            eprintln!("Valid keys:");
            for k in VALID_KEYS {
                eprintln!("  {}", k);
            }
            eprintln!();
            eprintln!("Run 'whis config --list' to see current values");
            std::process::exit(1);
        }

        if let Some(val) = value {
            // Set operation
            set_config(&key_normalized, &val)
        } else {
            // Get operation
            get_config(&key_normalized)
        }
    } else {
        // No arguments - show usage
        show_usage();
        std::process::exit(1);
    }
}

fn set_config(key: &str, value: &str) -> Result<()> {
    let mut settings = Settings::load_cli();
    let value_trimmed = value.trim();

    match key {
        "provider" => {
            let provider = value_trimmed
                .parse::<TranscriptionProvider>()
                .map_err(|e| anyhow!("{}", e))?;
            settings.transcription.provider = provider;
            println!("provider = {}", value_trimmed);
        }
        "language" => {
            if value_trimmed.to_lowercase() == "auto" {
                settings.transcription.language = None;
                println!("language = auto-detect");
            } else {
                let lang_lower = value_trimmed.to_lowercase();
                if lang_lower.len() != 2 || !lang_lower.chars().all(|c| c.is_ascii_lowercase()) {
                    anyhow::bail!(
                        "Invalid language code. Use ISO-639-1 format (e.g., 'en', 'de', 'fr') or 'auto'"
                    );
                }
                settings.transcription.language = Some(lang_lower.clone());
                println!("language = {}", lang_lower);
            }
        }
        "openai-api-key" => {
            if !value_trimmed.starts_with("sk-") {
                anyhow::bail!("Invalid key format. OpenAI keys start with 'sk-'");
            }
            settings
                .transcription
                .set_api_key(&TranscriptionProvider::OpenAI, value_trimmed.to_string());
            println!("openai-api-key = {}", mask_key(value_trimmed));
        }
        "mistral-api-key" => {
            validate_api_key(value_trimmed, "Mistral")?;
            settings
                .transcription
                .set_api_key(&TranscriptionProvider::Mistral, value_trimmed.to_string());
            println!("mistral-api-key = {}", mask_key(value_trimmed));
        }
        "groq-api-key" => {
            if !value_trimmed.starts_with("gsk_") {
                anyhow::bail!("Invalid key format. Groq keys start with 'gsk_'");
            }
            settings
                .transcription
                .set_api_key(&TranscriptionProvider::Groq, value_trimmed.to_string());
            println!("groq-api-key = {}", mask_key(value_trimmed));
        }
        "deepgram-api-key" => {
            validate_api_key(value_trimmed, "Deepgram")?;
            settings
                .transcription
                .set_api_key(&TranscriptionProvider::Deepgram, value_trimmed.to_string());
            println!("deepgram-api-key = {}", mask_key(value_trimmed));
        }
        "elevenlabs-api-key" => {
            validate_api_key(value_trimmed, "ElevenLabs")?;
            settings.transcription.set_api_key(
                &TranscriptionProvider::ElevenLabs,
                value_trimmed.to_string(),
            );
            println!("elevenlabs-api-key = {}", mask_key(value_trimmed));
        }
        "whisper-model-path" => {
            if value_trimmed.is_empty() {
                anyhow::bail!("Invalid whisper model path: cannot be empty");
            }
            let expanded_path = expand_home_dir(value_trimmed);
            settings.transcription.local_models.whisper_path = Some(expanded_path.clone());
            println!("whisper-model-path = {}", expanded_path);
        }
        "parakeet-model-path" => {
            if value_trimmed.is_empty() {
                anyhow::bail!("Invalid parakeet model path: cannot be empty");
            }
            let expanded_path = expand_home_dir(value_trimmed);
            settings.transcription.local_models.parakeet_path = Some(expanded_path.clone());
            println!("parakeet-model-path = {}", expanded_path);
        }
        "post-processor" => {
            let processor = value_trimmed
                .parse::<PostProcessor>()
                .map_err(|e| anyhow!("{}", e))?;
            settings.post_processing.processor = processor;
            println!("post-processor = {}", value_trimmed);
        }
        "post-processing-prompt" => {
            if value_trimmed.is_empty() {
                anyhow::bail!("Invalid post-processing prompt: cannot be empty");
            }
            settings.post_processing.prompt = Some(value_trimmed.to_string());
            println!(
                "post-processing-prompt = {}",
                truncate_prompt(value_trimmed)
            );
        }
        "ollama-url" => {
            if value_trimmed.is_empty() {
                anyhow::bail!("Invalid Ollama URL: cannot be empty");
            }
            settings.services.ollama.url = Some(value_trimmed.to_string());
            println!("ollama-url = {}", value_trimmed);
        }
        "ollama-model" => {
            if value_trimmed.is_empty() {
                anyhow::bail!("Invalid Ollama model: cannot be empty");
            }
            settings.services.ollama.model = Some(value_trimmed.to_string());
            println!("ollama-model = {}", value_trimmed);
        }
        "microphone-device" => {
            if value_trimmed.to_lowercase() == "default" || value_trimmed.is_empty() {
                settings.ui.microphone_device = None;
                println!("microphone-device = System Default");
            } else {
                settings.ui.microphone_device = Some(value_trimmed.to_string());
                println!("microphone-device = {}", value_trimmed);
            }
        }
        "vad" => {
            let enabled = value_trimmed
                .parse::<bool>()
                .context("Invalid value. Use 'true' or 'false'")?;
            settings.ui.vad.enabled = enabled;
            println!("vad = {}", enabled);
        }
        "vad-threshold" => {
            let threshold = value_trimmed
                .parse::<f32>()
                .context("Invalid threshold. Use a number between 0.0 and 1.0")?;
            if !(0.0..=1.0).contains(&threshold) {
                anyhow::bail!("Invalid VAD threshold: must be between 0.0 and 1.0");
            }
            settings.ui.vad.threshold = threshold;
            println!("vad-threshold = {:.2}", threshold);
        }
        "chunk-size" => {
            let size = value_trimmed
                .parse::<u64>()
                .context("Invalid chunk size. Use a number of seconds (e.g., 30, 60, 90)")?;
            if !(10..=300).contains(&size) {
                anyhow::bail!("Invalid chunk size: must be between 10 and 300 seconds");
            }
            settings.ui.chunk_duration_secs = size;
            println!("chunk-size = {}s", size);
        }
        "cli-mode" => {
            let mode: CliShortcutMode = value_trimmed
                .parse()
                .map_err(|e: String| anyhow!("{}", e))?;
            settings.shortcuts.cli_mode = mode;
            // Validate before saving (check for conflicts)
            settings.shortcuts.validate()?;
            println!("cli-mode = {}", mode);
        }
        "cli-key" => {
            if value_trimmed.is_empty() {
                anyhow::bail!("Invalid CLI shortcut key: cannot be empty");
            }
            settings.shortcuts.cli_key = value_trimmed.to_string();
            // Validate before saving (check for conflicts)
            settings.shortcuts.validate()?;
            println!("cli-key = {}", value_trimmed);
        }
        "desktop-key" => {
            if value_trimmed.is_empty() {
                anyhow::bail!("Invalid Desktop shortcut key: cannot be empty");
            }
            settings.shortcuts.desktop_key = value_trimmed.to_string();
            // Validate before saving (check for conflicts)
            settings.shortcuts.validate()?;
            println!("desktop-key = {}", value_trimmed);
        }
        "cli-push-to-talk" => {
            let enabled = value_trimmed
                .parse::<bool>()
                .context("Invalid value. Use 'true' or 'false'")?;
            settings.shortcuts.cli_push_to_talk = enabled;
            println!("cli-push-to-talk = {}", enabled);
        }
        _ => unreachable!("Key validation should prevent this"),
    }

    settings.save_cli()?;
    Ok(())
}

fn get_config(key: &str) -> Result<()> {
    let settings = Settings::load_cli();

    match key {
        "provider" => println!("{}", settings.transcription.provider),
        "language" => println!(
            "{}",
            settings.transcription.language.as_deref().unwrap_or("auto")
        ),
        "openai-api-key" => print_api_key(&settings, &TranscriptionProvider::OpenAI),
        "mistral-api-key" => print_api_key(&settings, &TranscriptionProvider::Mistral),
        "groq-api-key" => print_api_key(&settings, &TranscriptionProvider::Groq),
        "deepgram-api-key" => print_api_key(&settings, &TranscriptionProvider::Deepgram),
        "elevenlabs-api-key" => print_api_key(&settings, &TranscriptionProvider::ElevenLabs),
        "whisper-model-path" => {
            if let Some(path) = &settings.transcription.local_models.whisper_path {
                println!("{}", path);
            } else {
                println!("(not set, using $LOCAL_WHISPER_MODEL_PATH)");
            }
        }
        "parakeet-model-path" => {
            if let Some(path) = &settings.transcription.local_models.parakeet_path {
                println!("{}", path);
            } else {
                println!("(not set, using $LOCAL_PARAKEET_MODEL_PATH)");
            }
        }
        "post-processor" => println!("{}", settings.post_processing.processor),
        "post-processing-prompt" => {
            if let Some(prompt) = &settings.post_processing.prompt {
                println!("{}", prompt);
            } else {
                println!("(default)");
            }
        }
        "ollama-url" => {
            if let Some(url) = &settings.services.ollama.url {
                println!("{}", url);
            } else {
                println!("{}", DEFAULT_OLLAMA_URL);
            }
        }
        "ollama-model" => {
            if let Some(model) = &settings.services.ollama.model {
                println!("{}", model);
            } else {
                println!("{}", DEFAULT_OLLAMA_MODEL);
            }
        }
        "microphone-device" => {
            if let Some(device) = &settings.ui.microphone_device {
                println!("{}", device);
            } else {
                println!("System Default");
            }
        }
        "vad" => println!("{}", settings.ui.vad.enabled),
        "vad-threshold" => println!("{:.2}", settings.ui.vad.threshold),
        "chunk-size" => println!("{}s", settings.ui.chunk_duration_secs),
        "cli-mode" => println!("{}", settings.shortcuts.cli_mode),
        "cli-key" => println!("{}", settings.shortcuts.cli_key),
        "cli-push-to-talk" => println!("{}", settings.shortcuts.cli_push_to_talk),
        "desktop-key" => println!("{}", settings.shortcuts.desktop_key),
        _ => unreachable!("Key validation should prevent this"),
    }

    Ok(())
}

fn print_api_key(settings: &Settings, provider: &TranscriptionProvider) {
    if let Some(key) = settings.transcription.api_key_for(provider) {
        println!("{}", mask_key(&key));
    } else {
        println!("(not set, using ${})", provider.api_key_env_var());
    }
}

fn show_all_settings() -> Result<()> {
    let settings = Settings::load_cli();

    println!("Configuration file: {}", Settings::cli_path().display());
    println!();

    println!("[Transcription]");
    println!("provider = {}", settings.transcription.provider);
    println!(
        "language = {}",
        settings.transcription.language.as_deref().unwrap_or("auto")
    );

    for provider in TranscriptionProvider::all() {
        let key_name = format!(
            "{}-api-key",
            provider.to_string().to_lowercase().replace('_', "-")
        );
        let key_status = if settings.transcription.has_configured_api_key(provider) {
            // Key is in settings.json
            mask_key(&settings.transcription.api_key_for(provider).unwrap())
        } else if settings.transcription.api_key_for(provider).is_some() {
            // Key only in environment variable
            format!("(from ${})", provider.api_key_env_var())
        } else {
            // No key at all
            "(not set)".to_string()
        };
        println!("{} = {}", key_name, key_status);
    }

    println!();
    println!("[Local Models]");
    if let Some(path) = &settings.transcription.local_models.whisper_path {
        println!("whisper-model-path = {}", path);
    } else {
        println!("whisper-model-path = (not set, using $LOCAL_WHISPER_MODEL_PATH)");
    }

    if let Some(path) = &settings.transcription.local_models.parakeet_path {
        println!("parakeet-model-path = {}", path);
    } else {
        println!("parakeet-model-path = (not set, using $LOCAL_PARAKEET_MODEL_PATH)");
    }

    println!();
    println!("[Post-Processing]");
    println!("post-processor = {}", settings.post_processing.processor);
    if let Some(prompt) = &settings.post_processing.prompt {
        println!("post-processing-prompt = {}", truncate_prompt(prompt));
    } else {
        println!("post-processing-prompt = (default)");
    }

    println!();
    println!("[Services]");
    if let Some(url) = &settings.services.ollama.url {
        println!("ollama-url = {}", url);
    } else {
        println!("ollama-url = {}", DEFAULT_OLLAMA_URL);
    }
    if let Some(model) = &settings.services.ollama.model {
        println!("ollama-model = {}", model);
    } else {
        println!("ollama-model = {}", DEFAULT_OLLAMA_MODEL);
    }

    println!();
    println!("[Audio]");
    if let Some(device) = &settings.ui.microphone_device {
        println!("microphone-device = {}", device);
    } else {
        println!("microphone-device = System Default");
    }

    println!();
    println!("[Voice Activity Detection]");
    println!("vad = {}", settings.ui.vad.enabled);
    println!("vad-threshold = {:.2}", settings.ui.vad.threshold);

    println!();
    println!("[Audio Chunking]");
    println!("chunk-size = {}s", settings.ui.chunk_duration_secs);

    println!();
    println!("[Shortcuts]");
    println!("cli-mode = {}", settings.shortcuts.cli_mode);
    println!("cli-key = {}", settings.shortcuts.cli_key);
    println!("cli-push-to-talk = {}", settings.shortcuts.cli_push_to_talk);
    println!("desktop-key = {}", settings.shortcuts.desktop_key);

    println!();
    println!("[Presets]");
    println!("Available presets: {}", Preset::all_names().join(", "));

    Ok(())
}

fn show_usage() {
    eprintln!("Usage:");
    eprintln!("  whis config <key> <value>    Set a configuration value");
    eprintln!("  whis config <key>            Get a configuration value");
    eprintln!("  whis config --list           List all configuration");
    eprintln!("  whis config --path           Show configuration file path");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  whis config provider openai");
    eprintln!("  whis config openai-api-key sk-...");
    eprintln!("  whis config language en");
    eprintln!("  whis config post-processor ollama");
    eprintln!("  whis config vad true");
    eprintln!("  whis config chunk-size 30");
    eprintln!();
    eprintln!("Run 'whis config --list' to see all available keys and current values");
}

fn expand_home_dir(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
}

fn validate_api_key(key: &str, provider_name: &str) -> Result<()> {
    if key.is_empty() {
        anyhow::bail!("Invalid {} API key: cannot be empty", provider_name);
    }
    if key.len() < 20 {
        anyhow::bail!("Invalid {} API key: key appears too short", provider_name);
    }
    Ok(())
}

fn truncate_prompt(prompt: &str) -> String {
    if prompt.len() > 50 {
        format!("{}...", &prompt[..47])
    } else {
        prompt.to_string()
    }
}
