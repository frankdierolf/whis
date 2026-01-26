//! Post-processing setup (Ollama, OpenAI, Mistral)

use anyhow::{Result, anyhow};
use std::io::Write;
use whis_core::{PostProcessor, Settings, TranscriptionProvider, ollama};

use super::cloud::prompt_and_validate_key;
use super::interactive;
use super::provider_helpers::{PP_PROVIDERS, api_key_url};

/// Display a progress bar for ollama model pulls (with bracket notation prefix)
fn display_progress(downloaded: u64, total: u64) {
    let progress = if total > 0 {
        (downloaded * 100 / total) as usize
    } else {
        0
    };

    let bar_width = 20;
    let filled = (bar_width * progress) / 100;

    eprint!("\r[i] [");
    for i in 0..bar_width {
        if i < filled {
            eprint!("=");
        } else {
            eprint!(" ");
        }
    }
    eprint!("] {}%", progress);

    std::io::stderr().flush().ok();
}

/// Interactive Ollama model selection
/// Shows installed models + recommended options, allows pulling new models
pub fn select_ollama_model(url: &str, current_model: Option<&str>) -> Result<String> {
    // Get installed models from Ollama
    let installed = ollama::list_models(url).unwrap_or_default();
    let installed_names: Vec<&str> = installed.iter().map(|m| m.name.as_str()).collect();

    // Build display items (with markers) and clean items (just model names)
    let mut items = Vec::new();
    let mut clean_items = Vec::new();
    let mut model_data: Vec<Option<(String, bool)>> = Vec::new(); // (name, needs_download)

    // Installed section (with prefix, no separator)
    if !installed.is_empty() {
        for model in &installed {
            let is_current = current_model
                .map(|c| {
                    // Handle both exact match and version tag differences
                    model.name == c || model.name.starts_with(&format!("{}:", c))
                })
                .unwrap_or(false);

            let size = if model.size > 0 {
                format!(" ({})", model.size_str())
            } else {
                String::new()
            };

            let markers = if is_current {
                " [current]".to_string()
            } else {
                String::new()
            };

            // Use [Installed] prefix instead of separator
            items.push(format!("[Installed] {}{}{}", model.name, size, markers));
            clean_items.push(model.name.clone());
            model_data.push(Some((model.name.clone(), false)));
        }
    }

    // Recommended section (not installed)
    let not_installed: Vec<_> = ollama::OLLAMA_MODEL_OPTIONS
        .iter()
        .filter(|(name, _, _)| {
            !installed_names
                .iter()
                .any(|n| n.starts_with(name.split(':').next().unwrap_or(name)))
        })
        .collect();

    if !not_installed.is_empty() {
        for (name, size, desc) in not_installed {
            // Use [Available] prefix instead of separator
            items.push(format!("[Available] {} ({}) - {}", name, size, desc));
            clean_items.push(name.to_string());
            model_data.push(Some((name.to_string(), true)));
        }
    }

    // Custom option
    items.push("[Custom] Enter custom model name".to_string());
    clean_items.push("Custom".to_string());
    model_data.push(None); // Custom trigger

    // Find default index with robust fallback chain
    let default = if let Some(current) = current_model {
        // Try exact match first
        let exact_match = model_data
            .iter()
            .position(|m| m.as_ref().map(|(n, _)| n.as_str()) == Some(current));

        if exact_match.is_some() {
            exact_match
        } else {
            // Try prefix match (handles version tags like :latest)
            let current_base = current.split(':').next().unwrap_or(current);
            let prefix_match = model_data.iter().position(|m| {
                if let Some((name, _)) = m {
                    let name_base = name.split(':').next().unwrap_or(name);
                    name_base == current_base || name.starts_with(&format!("{}:", current))
                } else {
                    false
                }
            });

            // Fallback to first model if no match
            prefix_match.or_else(|| model_data.iter().position(|m| m.is_some()))
        }
    } else {
        // No current model - select recommended model (ministral:3b)
        let recommended = model_data.iter().position(|m| {
            if let Some((name, _)) = m {
                name.starts_with("ministral:3b")
            } else {
                false
            }
        });

        // Fallback to first model if recommended not found
        recommended.or_else(|| model_data.iter().position(|m| m.is_some()))
    };

    // Ensure default is always Some (safety net)
    let default = default.or(Some(0));

    // Interactive select
    let choice = interactive::select_clean("Which Ollama model?", &items, &clean_items, default)?;

    // Handle selection
    match &model_data[choice] {
        Some((model_name, needs_download)) => {
            // Selected a model from the list
            if *needs_download {
                interactive::info(&format!("Pulling model '{}'...", model_name));
                ollama::pull_model_with_progress(url, model_name, display_progress)?;

                // Verify pull succeeded
                if !ollama::has_model(url, model_name)? {
                    return Err(anyhow!(
                        "Failed to pull model '{}'. Check your connection.",
                        model_name
                    ));
                }

                eprintln!(); // Newline after progress bar (only on success)
                interactive::info(&format!("Model '{}' ready!", model_name));
            }
            Ok(model_name.clone())
        }
        None => {
            // Either separator or custom model
            if items[choice].contains("custom") {
                // Custom model input - loop until valid
                loop {
                    let model_name =
                        interactive::input("Enter model name (e.g., llama3.2:1b)", None)?;

                    if model_name.is_empty() {
                        interactive::error("Model name cannot be empty");
                        continue;
                    }

                    // Check if model exists, pull if needed
                    if !ollama::has_model(url, &model_name)? {
                        interactive::info(&format!("Pulling model '{}'...", model_name));
                        ollama::pull_model_with_progress(url, &model_name, display_progress)?;

                        // Verify pull succeeded
                        if !ollama::has_model(url, &model_name)? {
                            interactive::error(&format!(
                                "Model '{}' not found. Try a different name.",
                                model_name
                            ));
                            continue;
                        }

                        eprintln!(); // Newline after progress bar (only on success)
                        interactive::info(&format!("Model '{}' ready!", model_name));
                    }

                    return Ok(model_name);
                }
            } else {
                // Separator selected - shouldn't happen with proper navigation
                Err(anyhow!("Invalid selection"))
            }
        }
    }
}

/// Independent post-processing step (called after transcription setup in wizard)
pub fn setup_post_processing_step(_prefer_cloud: bool) -> Result<()> {
    let mut settings = Settings::load_cli();

    // Default to current processor setting
    let default = match settings.post_processing.processor {
        PostProcessor::OpenAI | PostProcessor::Mistral => 0, // Cloud
        PostProcessor::Ollama => 1,
        PostProcessor::None => 2, // Skip
    };

    let options = vec!["Cloud", "Ollama", "Skip"];
    let choice = interactive::select("Configure post-processing?", &options, Some(default))?;

    match choice {
        0 => setup_cloud_post_processing(&mut settings)?,
        1 => {
            // Ollama setup with model selection
            let ollama_url = ollama::DEFAULT_OLLAMA_URL;

            // Check if Ollama is installed
            if !ollama::is_ollama_installed() {
                interactive::ollama_not_installed();
                interactive::info(
                    "You can run 'whis setup post-processing' later to configure Ollama.",
                );
                return Ok(());
            }

            // Start Ollama if not running
            ollama::ensure_ollama_running(ollama_url)?;

            // Select model (shows installed models + recommended options)
            let current_model = settings.services.ollama.model.as_deref();
            let model = select_ollama_model(ollama_url, current_model)?;

            settings.post_processing.processor = PostProcessor::Ollama;
            settings.services.ollama.url = Some(ollama_url.to_string());
            settings.services.ollama.model = Some(model);
        }
        2 => {
            settings.post_processing.processor = PostProcessor::None;
        }
        _ => unreachable!(),
    }

    settings.save_cli()?;
    Ok(())
}

/// Setup cloud post-processing (OpenAI or Mistral)
fn setup_cloud_post_processing(settings: &mut Settings) -> Result<()> {
    // Build provider items: with markers for selection, clean for confirmation
    let (items, clean_items): (Vec<String>, Vec<String>) = PP_PROVIDERS
        .iter()
        .map(|provider| {
            let base = provider.display_name().to_string();
            let marker = if settings.transcription.has_configured_api_key(provider) {
                " [configured]"
            } else if settings.transcription.api_key_for(provider).is_some() {
                " [available]"
            } else {
                ""
            };
            (format!("{}{}", base, marker), base)
        })
        .unzip();

    // Default to current processor if it matches a PP provider
    let default = match settings.post_processing.processor {
        PostProcessor::OpenAI => PP_PROVIDERS
            .iter()
            .position(|p| *p == TranscriptionProvider::OpenAI),
        PostProcessor::Mistral => PP_PROVIDERS
            .iter()
            .position(|p| *p == TranscriptionProvider::Mistral),
        _ => Some(0),
    };

    let choice = interactive::select_clean("Which provider?", &items, &clean_items, default)?;
    let provider = PP_PROVIDERS[choice].clone();

    // Check if API key already exists
    if let Some(existing_key) = settings.transcription.api_key_for(&provider) {
        let is_configured = settings.transcription.has_configured_api_key(&provider);

        let keep = interactive::select("Keep current key?", &["Yes", "No"], Some(0))? == 0;

        if keep {
            // If key is env-only, save it to settings
            if !is_configured {
                settings.transcription.set_api_key(&provider, existing_key);
                interactive::info("API key saved to settings");
            }
        } else {
            interactive::info(&format!(
                "Get your API key from: {}",
                api_key_url(&provider)
            ));
            let api_key = prompt_and_validate_key(&provider)?;
            settings.transcription.set_api_key(&provider, api_key);
        }
    } else {
        interactive::info(&format!(
            "Get your API key from: {}",
            api_key_url(&provider)
        ));
        let api_key = prompt_and_validate_key(&provider)?;
        settings.transcription.set_api_key(&provider, api_key);
    }

    settings.post_processing.processor = match provider {
        TranscriptionProvider::OpenAI => PostProcessor::OpenAI,
        TranscriptionProvider::Mistral => PostProcessor::Mistral,
        _ => unreachable!(),
    };

    Ok(())
}
