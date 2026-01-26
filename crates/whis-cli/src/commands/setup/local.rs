//! Local (on-device) transcription setup
//!
//! Handles model selection and download for local transcription engines:
//! - Parakeet (NVIDIA NeMo, fast, English-optimized)
//! - Whisper (OpenAI, multilingual)
//!
//! # Flow
//!
//! 1. Select engine (Parakeet/Whisper) with [current] marker
//! 2. Select model variant with [installed]/[current] markers
//! 3. Download model if not present
//! 4. Save to settings

use anyhow::Result;
use whis_core::{Settings, TranscriptionProvider, model};

use super::interactive;

#[cfg(feature = "local-transcription")]
use whis_core::model::{ModelType, ParakeetModel, WhisperModel};

/// Streamlined local transcription setup (no post-processing config)
/// Used by the unified wizard
pub fn setup_transcription_local() -> Result<()> {
    let mut settings = Settings::load_cli();

    // Determine current engine and show with [current] marker during selection
    let current_engine = match settings.transcription.provider {
        TranscriptionProvider::LocalParakeet => Some(1),
        TranscriptionProvider::LocalWhisper => Some(2),
        _ => None,
    };

    let (items, clean_items): (Vec<String>, Vec<String>) = vec![
        (
            format!(
                "Parakeet{}",
                if current_engine == Some(1) {
                    " [current]"
                } else {
                    ""
                }
            ),
            "Parakeet".to_string(),
        ),
        (
            format!(
                "Whisper{}",
                if current_engine == Some(2) {
                    " [current]"
                } else {
                    ""
                }
            ),
            "Whisper".to_string(),
        ),
    ]
    .into_iter()
    .unzip();

    let default = current_engine.map(|e| e - 1).unwrap_or(0);
    let engine_choice = interactive::select_clean(
        "Which transcription engine?",
        &items,
        &clean_items,
        Some(default),
    )? + 1;

    let (provider, model_path) = match engine_choice {
        1 => {
            // Parakeet - show model options (matching Whisper pattern)

            // Get current model if any
            let current_model = settings
                .transcription
                .local_models
                .parakeet_path
                .as_ref()
                .and_then(|p| {
                    ParakeetModel
                        .models()
                        .iter()
                        .find(|m| ParakeetModel.default_path(m.name) == *p)
                        .map(|m| m.name)
                });

            // Build selection items with markers, clean items without
            let (items, clean_items): (Vec<String>, Vec<String>) = ParakeetModel
                .models()
                .iter()
                .map(|model| {
                    let path = ParakeetModel.default_path(model.name);
                    let installed = if ParakeetModel.verify(&path) {
                        " [installed]"
                    } else {
                        ""
                    };
                    let current = if current_model == Some(model.name) {
                        " [current]"
                    } else {
                        ""
                    };
                    (
                        format!("{}{}{}", model.name, installed, current),
                        model.name.to_string(),
                    )
                })
                .unzip();

            // Default to current model or first (recommended)
            let default_idx = current_model
                .and_then(|name| ParakeetModel.models().iter().position(|m| m.name == name))
                .unwrap_or(0);

            let model_choice = interactive::select_clean(
                "Which Parakeet model?",
                &items,
                &clean_items,
                Some(default_idx),
            )?;
            let model = &ParakeetModel.models()[model_choice];

            let path = ParakeetModel.default_path(model.name);
            if !ParakeetModel.verify(&path) {
                model::download::download(&ParakeetModel, model.name, &path)?;
            }

            (TranscriptionProvider::LocalParakeet, path)
        }
        2 => {
            // Whisper - show model options
            // Get current model if any
            let current_model = settings
                .transcription
                .local_models
                .whisper_path
                .as_ref()
                .and_then(|p| {
                    WhisperModel
                        .models()
                        .iter()
                        .find(|m| WhisperModel.default_path(m.name) == *p)
                        .map(|m| m.name)
                });

            // Build selection items with markers, clean items without
            let (items, clean_items): (Vec<String>, Vec<String>) = WhisperModel
                .models()
                .iter()
                .map(|model| {
                    let path = WhisperModel.default_path(model.name);
                    let installed = if WhisperModel.verify(&path) {
                        " [installed]"
                    } else {
                        ""
                    };
                    let current = if current_model == Some(model.name) {
                        " [current]"
                    } else {
                        ""
                    };
                    (
                        format!(
                            "{} - {}{}{}",
                            model.name, model.description, installed, current
                        ),
                        model.name.to_string(),
                    )
                })
                .unzip();

            // Default to current model or "small" (index 2)
            let default_idx = current_model
                .and_then(|name| WhisperModel.models().iter().position(|m| m.name == name))
                .unwrap_or(2);

            let model_choice = interactive::select_clean(
                "Which Whisper model?",
                &items,
                &clean_items,
                Some(default_idx),
            )?;
            let model = &WhisperModel.models()[model_choice];

            let path = WhisperModel.default_path(model.name);
            if !WhisperModel.verify(&path) {
                interactive::info(&format!("Downloading {}...", model.name));
                model::download::download(&WhisperModel, model.name, &path)?;
            }

            (TranscriptionProvider::LocalWhisper, path)
        }
        _ => unreachable!(),
    };

    // Save transcription config
    settings.transcription.provider = provider.clone();
    match &provider {
        TranscriptionProvider::LocalParakeet => {
            settings.transcription.local_models.parakeet_path =
                Some(model_path.to_string_lossy().to_string());
        }
        TranscriptionProvider::LocalWhisper => {
            settings.transcription.local_models.whisper_path =
                Some(model_path.to_string_lossy().to_string());
        }
        _ => {}
    }
    settings.save_cli()?;

    Ok(())
}
