//! Transcription Pipeline
//!
//! Orchestrates the full transcription pipeline:
//! 1. Finalize recording (encode audio)
//! 2. Transcribe audio (single or parallel chunks)
//! 3. Post-process transcription (optional)
//! 4. Copy to clipboard
//! 5. Emit completion event

use crate::state::{AppState, RecordingState};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use whis_core::{
    AutotypeBackend, ClipboardMethod, DEFAULT_POST_PROCESSING_PROMPT, OutputMethod,
    PostProcessConfig, PostProcessor, TranscriptionProvider, autotype_text, copy_to_clipboard,
    ollama, post_process, warn,
};
#[cfg(feature = "local-transcription")]
use whis_core::{unload_parakeet, whisper_unload_model};

/// Output text based on configured output method
fn output_text(
    text: &str,
    output_method: &OutputMethod,
    clipboard_method: &ClipboardMethod,
    autotype_backend: &AutotypeBackend,
    autotype_delay_ms: Option<u32>,
) -> Result<(), String> {
    match output_method {
        OutputMethod::Clipboard => {
            copy_to_clipboard(text, clipboard_method.clone()).map_err(|e| e.to_string())?;
        }
        OutputMethod::Autotype => {
            autotype_text(text, autotype_backend.clone(), autotype_delay_ms)
                .map_err(|e| e.to_string())?;
        }
        OutputMethod::Both => {
            copy_to_clipboard(text, clipboard_method.clone()).map_err(|e| e.to_string())?;
            autotype_text(text, autotype_backend.clone(), autotype_delay_ms)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Stop recording and run the full transcription pipeline (progressive mode)
/// Guarantees state cleanup on both success and failure
pub async fn stop_and_transcribe(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Stop recording (closes audio stream, signals chunker/transcription to finish)
    {
        let mut recorder = state.recorder.lock().unwrap().take();
        if let Some(ref mut rec) = recorder {
            rec.stop_recording().map_err(|e| e.to_string())?;
        }
    }

    // Update state to transcribing
    {
        *state.state.lock().unwrap() = RecordingState::Transcribing;
    }
    println!("Transcribing...");

    // Run transcription with guaranteed state cleanup on any error
    let result = do_progressive_transcription(app, &state).await;

    // Always reset state, regardless of success or failure
    {
        *state.state.lock().unwrap() = RecordingState::Idle;
    }

    result
}

/// Progressive transcription logic - receives result from background task
async fn do_progressive_transcription(app: &AppHandle, state: &AppState) -> Result<(), String> {
    // Receive transcription result from background task
    let rx = {
        let mut rx_guard = state.transcription_rx.lock().unwrap();
        rx_guard
            .take()
            .ok_or("No progressive transcription in progress")?
    };

    // Wait for transcription to complete (rx_guard dropped, so this is Send-safe)
    let transcription = rx
        .await
        .map_err(|_| "Transcription task dropped unexpectedly".to_string())?
        .map_err(|e| format!("Transcription failed: {e}"))?;

    // Extract post-processing config and output settings from settings
    let (post_process_config, clipboard_method, output_method, autotype_backend, autotype_delay_ms) = {
        let settings = state.settings.lock().unwrap();
        let clipboard_method = settings.ui.clipboard_backend.clone();
        let output_method = settings.ui.output_method.clone();
        let autotype_backend = settings.ui.autotype_backend.clone();
        let autotype_delay_ms = settings.ui.autotype_delay_ms;
        let post_process_config = if settings.post_processing.enabled
            && settings.post_processing.processor != PostProcessor::None
        {
            let processor = settings.post_processing.processor.clone();
            let prompt = settings
                .post_processing
                .prompt
                .clone()
                .unwrap_or_else(|| DEFAULT_POST_PROCESSING_PROMPT.to_string());
            let ollama_model = settings.services.ollama.model.clone();
            let ollama_keep_alive = settings.services.ollama.keep_alive();

            let api_key_or_url = if processor.requires_api_key() {
                settings
                    .post_processing
                    .api_key_from_settings(&settings.transcription.api_keys)
            } else if processor == PostProcessor::Ollama {
                let ollama_url = settings
                    .services
                    .ollama
                    .url()
                    .unwrap_or_else(|| ollama::DEFAULT_OLLAMA_URL.to_string());
                Some(ollama_url)
            } else {
                None
            };

            api_key_or_url.map(|key_or_url| PostProcessConfig {
                processor,
                prompt,
                api_key_or_url: key_or_url,
                ollama_model,
                ollama_keep_alive,
            })
        } else {
            None
        };
        (
            post_process_config,
            clipboard_method,
            output_method,
            autotype_backend,
            autotype_delay_ms,
        )
    };

    // Apply post-processing if configured
    let final_text = if let Some(config) = post_process_config {
        if config.processor == PostProcessor::Ollama {
            let url_for_check = config.api_key_or_url.clone();
            let ollama_result = tauri::async_runtime::spawn_blocking(move || {
                ollama::ensure_ollama_running(&url_for_check)
            })
            .await
            .map_err(|e| format!("Task join failed: {e}"))?;

            if let Err(e) = ollama_result {
                let warning = format!("Ollama: {e}");
                warn!("Post-processing: {warning}");
                let _ = app.emit("post-process-warning", &warning);

                // Output based on configured method
                output_text(
                    &transcription,
                    &output_method,
                    &clipboard_method,
                    &autotype_backend,
                    autotype_delay_ms,
                )?;

                println!(
                    "Done (unprocessed): {}",
                    &transcription[..transcription.len().min(50)]
                );
                let _ = app.emit("transcription-complete", &transcription);
                return Ok(());
            }

            // Re-warm Ollama model (in case it unloaded during long recording > keep_alive timeout)
            if config.ollama_model.is_some() {
                {
                    let settings = state.settings.lock().unwrap();
                    settings.services.ollama.preload();
                }
                // Brief pause to allow warmup to complete (runs in background thread)
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        }

        println!("Post-processing...");
        let _ = app.emit("post-process-started", ());

        let model = if config.processor == PostProcessor::Ollama {
            config.ollama_model.as_deref()
        } else {
            None
        };

        match post_process(
            &transcription,
            &config.processor,
            &config.api_key_or_url,
            &config.prompt,
            model,
        )
        .await
        {
            Ok(processed) => processed,
            Err(e) => {
                let warning = e.to_string();
                warn!("Post-processing: {warning}");
                let _ = app.emit("post-process-warning", &warning);
                transcription
            }
        }
    } else {
        transcription
    };

    // Output based on configured method
    output_text(
        &final_text,
        &output_method,
        &clipboard_method,
        &autotype_backend,
        autotype_delay_ms,
    )?;

    println!("Done: {}", &final_text[..final_text.len().min(50)]);

    // Emit event to frontend
    let _ = app.emit("transcription-complete", &final_text);

    // Schedule idle model unload (if configured)
    schedule_idle_model_unload(app, state);

    Ok(())
}

/// Schedule automatic model unload after idle timeout
///
/// If keep_model_loaded is true and unload_after_minutes > 0, spawns a background
/// task that will unload the model after the configured idle period.
/// The task can be cancelled if a new recording starts.
fn schedule_idle_model_unload(app: &AppHandle, state: &AppState) {
    // Read settings to determine if we should schedule an unload
    let (keep_loaded, unload_minutes, provider) = {
        let settings = state.settings.lock().unwrap();
        let config = state.transcription_config.lock().unwrap();
        let provider = config.as_ref().map(|c| c.provider.clone());
        (
            settings.ui.model_memory.keep_model_loaded,
            settings.ui.model_memory.unload_after_minutes,
            provider,
        )
    };

    // Only schedule if:
    // 1. keep_model_loaded is true (otherwise model is already unloaded)
    // 2. unload_minutes > 0 (0 means "never auto-unload")
    // 3. We have a local provider
    #[cfg(feature = "local-transcription")]
    if keep_loaded
        && unload_minutes > 0
        && let Some(provider) = provider
        && matches!(
            provider,
            TranscriptionProvider::LocalWhisper | TranscriptionProvider::LocalParakeet
        )
    {
        let duration = Duration::from_secs(u64::from(unload_minutes) * 60);
        let app_handle = app.clone();

        // Spawn the delayed unload task
        let task = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(duration).await;

            // After sleep, unload the model
            let state = app_handle.state::<AppState>();

            // Re-check settings in case user changed them while we were waiting
            let should_unload = {
                let settings = state.settings.lock().unwrap();
                settings.ui.model_memory.keep_model_loaded
                    && settings.ui.model_memory.unload_after_minutes > 0
            };

            if should_unload {
                println!(
                    "Auto-unloading model after {} minutes of inactivity",
                    unload_minutes
                );
                match provider {
                    TranscriptionProvider::LocalWhisper => {
                        whisper_unload_model();
                    }
                    TranscriptionProvider::LocalParakeet => {
                        unload_parakeet();
                    }
                    _ => {}
                }
            }
        });

        // Store the task handle so we can cancel if a new recording starts
        state.set_idle_unload_handle(task);
    }

    // Suppress unused variable warnings when local-transcription feature is disabled
    #[cfg(not(feature = "local-transcription"))]
    {
        let _ = (keep_loaded, unload_minutes, provider);
        let _ = app;
    }
}
