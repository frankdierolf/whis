//! Cloud transcription provider setup
//!
//! Handles API key configuration for cloud providers:
//! - OpenAI (standard + realtime streaming)
//! - Deepgram (standard + realtime streaming)
//! - Mistral, Groq, ElevenLabs
//!
//! # Flow
//!
//! 1. Select provider (with [configured] markers for existing keys)
//! 2. Choose method (standard vs streaming) for OpenAI/Deepgram
//! 3. Enter/confirm API key with format validation
//! 4. Save to settings

use anyhow::{Result, anyhow};
use whis_core::{Settings, TranscriptionProvider};

use super::interactive;
use super::provider_helpers::{api_key_url, cloud_providers, provider_description};

/// Prompt for streaming method selection (Standard vs Streaming)
///
/// Returns the realtime variant if streaming selected, otherwise the base provider.
fn select_streaming_method(
    base: TranscriptionProvider,
    realtime: TranscriptionProvider,
    current: &TranscriptionProvider,
) -> Result<TranscriptionProvider> {
    let methods = vec!["Standard - Progressive", "Streaming - Real-time"];
    let default_method = if *current == realtime { 1 } else { 0 };
    let choice = interactive::select("Which method?", &methods, Some(default_method))?;
    Ok(if choice == 1 { realtime } else { base })
}

/// Prompt for and validate an API key
pub fn prompt_and_validate_key(provider: &TranscriptionProvider) -> Result<String> {
    // Validation loop with secure password input
    loop {
        let api_key = interactive::password(&format!("{} API key", provider.display_name()))?;

        // Validate key format
        let validation_result = match provider {
            TranscriptionProvider::OpenAI | TranscriptionProvider::OpenAIRealtime => {
                if !api_key.starts_with("sk-") {
                    Err(anyhow!("Invalid OpenAI key format. Keys start with 'sk-'"))
                } else {
                    Ok(())
                }
            }
            TranscriptionProvider::Groq => {
                if !api_key.starts_with("gsk_") {
                    Err(anyhow!("Invalid Groq key format. Keys start with 'gsk_'"))
                } else {
                    Ok(())
                }
            }
            _ => {
                if api_key.len() < 20 {
                    Err(anyhow!("API key seems too short"))
                } else {
                    Ok(())
                }
            }
        };

        match validation_result {
            Ok(_) => return Ok(api_key),
            Err(e) => interactive::error(&e.to_string()),
        }
    }
}

/// Streamlined cloud transcription setup (no post-processing config)
/// Used by the unified wizard
pub fn setup_transcription_cloud() -> Result<()> {
    let mut settings = Settings::load_cli();
    let providers = cloud_providers();

    // Build provider display items: with markers for selection, just name for confirmation
    let (items, clean_items): (Vec<String>, Vec<String>) = providers
        .iter()
        .map(|provider| {
            let display = format!(
                "{:<10} - {}",
                provider.display_name(),
                provider_description(provider)
            );
            // Check if this provider or its realtime variant is configured
            let marker = if settings.transcription.has_configured_api_key(provider)
                || (*provider == TranscriptionProvider::OpenAI
                    && settings.transcription.provider == TranscriptionProvider::OpenAIRealtime)
                || (*provider == TranscriptionProvider::Deepgram
                    && settings.transcription.provider == TranscriptionProvider::DeepgramRealtime)
            {
                " [configured]"
            } else if settings.transcription.api_key_for(provider).is_some() {
                " [available]"
            } else {
                ""
            };
            (
                format!("{}{}", display, marker),
                provider.display_name().to_string(),
            )
        })
        .unzip();

    // Default to current provider if configured
    // Treat realtime variants same as base provider for default selection
    let default = providers.iter().position(|p| {
        *p == settings.transcription.provider
            || (*p == TranscriptionProvider::OpenAI
                && settings.transcription.provider == TranscriptionProvider::OpenAIRealtime)
            || (*p == TranscriptionProvider::Deepgram
                && settings.transcription.provider == TranscriptionProvider::DeepgramRealtime)
    });

    // Fallback: if on local provider, find first cloud provider with configured API key
    let default = default.or_else(|| {
        providers
            .iter()
            .position(|p| settings.transcription.api_key_for(p).is_some())
    });

    let choice = interactive::select_clean("Which provider?", &items, &clean_items, default)?;
    let mut provider = providers[choice].clone();

    // If OpenAI or Deepgram selected, ask for method (Standard vs Streaming)
    provider = match provider {
        TranscriptionProvider::OpenAI => select_streaming_method(
            TranscriptionProvider::OpenAI,
            TranscriptionProvider::OpenAIRealtime,
            &settings.transcription.provider,
        )?,
        TranscriptionProvider::Deepgram => select_streaming_method(
            TranscriptionProvider::Deepgram,
            TranscriptionProvider::DeepgramRealtime,
            &settings.transcription.provider,
        )?,
        _ => provider,
    };

    // Check if API key already exists for this provider
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
        // No existing key - prompt for new one
        interactive::info(&format!(
            "Get your API key from: {}",
            api_key_url(&provider)
        ));
        let api_key = prompt_and_validate_key(&provider)?;
        settings.transcription.set_api_key(&provider, api_key);
    }

    settings.transcription.provider = provider;
    settings.save_cli()?;

    Ok(())
}
