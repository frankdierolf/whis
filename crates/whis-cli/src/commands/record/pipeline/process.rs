//! Post-processing pipeline phase

use anyhow::Result;
use whis_core::{PostProcessor, Preset, Settings, post_process, resolve_post_processor_config};

use super::super::types::{ProcessedResult, TranscriptionResult};
use crate::app;

/// Post-processing configuration
pub struct ProcessingConfig {
    pub enabled: bool,
    pub preset: Option<Preset>,
}

/// Execute post-processing phase
pub async fn process(
    transcription: TranscriptionResult,
    config: &ProcessingConfig,
    quiet: bool,
) -> Result<ProcessedResult> {
    let mut text = transcription.text;

    // If post-processing is enabled OR a preset is provided, apply LLM processing
    if config.enabled || config.preset.is_some() {
        let settings = Settings::load();
        let (processor, api_key, model, prompt) =
            resolve_post_processor_config(&config.preset, &settings)?;

        // Re-warm Ollama model (in case it unloaded during long recording > keep_alive timeout)
        if processor == PostProcessor::Ollama && model.is_some() {
            settings.services.ollama.preload();
            // Brief pause to allow warmup to complete (runs in background thread)
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }

        if !quiet {
            app::print_status(" Post-processing...", None);
        }

        text = post_process(&text, &processor, &api_key, &prompt, model.as_deref()).await?;
    }

    Ok(ProcessedResult { text })
}
