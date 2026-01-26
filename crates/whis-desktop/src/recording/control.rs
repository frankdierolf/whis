//! Recording Control
//!
//! Handles starting and stopping audio recording with state management.

use super::config::load_transcription_config;
use crate::state::{AppState, RecordingState};
use tauri::AppHandle;
use tokio::sync::{mpsc, oneshot};
#[cfg(feature = "local-transcription")]
use whis_core::progressive_transcribe_local;
use whis_core::{
    AudioRecorder, ChunkerConfig, PostProcessor, ProgressiveChunker, TranscriptionProvider, info,
    progressive_transcribe_cloud,
};

/// Start recording with progressive transcription (default mode)
///
/// Starts streaming audio recording and spawns background tasks for:
/// - Progressive audio chunking (90s target, VAD-aware) for non-realtime providers
/// - WebSocket streaming for realtime providers (deepgram-realtime, openai-realtime)
/// - Transcription during recording (parallel for cloud providers, sequential for local providers)
///
/// The transcription result will be available via the oneshot channel
/// stored in AppState when recording completes.
pub fn start_recording_sync(_app: &AppHandle, state: &AppState) -> Result<(), String> {
    // Cancel any pending idle model unload (user is recording again)
    state.cancel_idle_unload();

    // Load transcription config if not already loaded
    let (provider, api_key, language) = {
        let mut config_guard = state.transcription_config.lock().unwrap();
        if config_guard.is_none() {
            *config_guard = Some(load_transcription_config(state)?);
        }
        let config = config_guard.as_ref().unwrap();
        (
            config.provider.clone(),
            config.api_key.clone(),
            config.language.clone(),
        )
    };

    // Check if this is a realtime provider (for branching later)
    let is_realtime = whis_core::is_realtime_provider(&provider);

    // Extract all needed settings values in a single lock acquisition
    let settings = state.settings.lock().unwrap();
    let vad_enabled = settings.ui.vad.enabled && !is_realtime;
    let vad_threshold = settings.ui.vad.threshold;
    let device_name = settings.ui.microphone_device.clone();
    let chunk_duration = settings.ui.chunk_duration_secs;
    #[cfg(feature = "local-transcription")]
    let keep_loaded = settings.ui.model_memory.keep_model_loaded;
    #[cfg(feature = "local-transcription")]
    let whisper_model_path = settings.transcription.whisper_model_path();
    #[cfg(feature = "local-transcription")]
    let parakeet_model_path = settings.transcription.parakeet_model_path();
    let ollama_preload = settings.post_processing.processor == PostProcessor::Ollama;
    let ollama_settings = settings.services.ollama.clone();
    drop(settings);

    // Configure model memory settings for local transcription
    #[cfg(feature = "local-transcription")]
    provider.set_keep_loaded(keep_loaded);

    // Create recorder and start streaming
    let mut recorder = AudioRecorder::new().map_err(|e| e.to_string())?;
    recorder.set_vad(vad_enabled, vad_threshold);

    // Start streaming recording
    let mut audio_rx_bounded = recorder
        .start_recording_streaming_with_device(device_name.as_deref())
        .map_err(|e| e.to_string())?;

    // Create unbounded channel adapter (used by both realtime and chunked paths)
    let (audio_tx_unbounded, audio_rx_unbounded) = mpsc::unbounded_channel();
    tauri::async_runtime::spawn(async move {
        while let Some(samples) = audio_rx_bounded.recv().await {
            if audio_tx_unbounded.send(samples).is_err() {
                break;
            }
        }
    });

    // Create oneshot channel for transcription result
    let (result_tx, result_rx) = oneshot::channel();

    // Preload models in background to reduce latency
    #[cfg(feature = "local-transcription")]
    match provider {
        TranscriptionProvider::LocalWhisper => {
            if let Some(ref model_path) = whisper_model_path {
                whis_core::whisper_preload_model(model_path);
            }
        }
        TranscriptionProvider::LocalParakeet => {
            if let Some(ref model_path) = parakeet_model_path {
                whis_core::preload_parakeet(model_path);
            }
        }
        _ => {} // Cloud providers don't need preload
    }

    // Preload Ollama if post-processing enabled
    if ollama_preload {
        ollama_settings.preload();
    }

    // Warm HTTP client for cloud providers to reduce first-request latency
    let _ = whis_core::warmup_http_client();

    // Branch based on provider type: realtime streaming vs chunked progressive
    if is_realtime {
        // REALTIME PATH: Stream audio directly to WebSocket (no chunking)
        #[cfg(feature = "realtime")]
        {
            let realtime_backend =
                whis_core::get_realtime_backend(&provider).map_err(|e| e.to_string())?;

            tauri::async_runtime::spawn(async move {
                let result = realtime_backend
                    .transcribe_stream(&api_key, audio_rx_unbounded, language)
                    .await
                    .map_err(|e| e.to_string());
                let _ = result_tx.send(result);
            });

            info!("Recording started (realtime streaming mode)");
        }

        #[cfg(not(feature = "realtime"))]
        {
            return Err(format!(
                "Provider '{}' requires the 'realtime' feature (not enabled in this build)",
                provider.as_str()
            ));
        }
    } else {
        // NON-REALTIME PATH: Use chunking + progressive transcription
        let (chunk_tx, chunk_rx) = mpsc::unbounded_channel();

        // Create chunker config from pre-extracted settings
        let chunker_config = ChunkerConfig {
            target_duration_secs: chunk_duration,
            min_duration_secs: chunk_duration * 2 / 3,
            max_duration_secs: chunk_duration * 4 / 3,
            vad_aware: vad_enabled,
        };

        // Spawn chunker task
        let mut chunker = ProgressiveChunker::new(chunker_config, chunk_tx);
        tauri::async_runtime::spawn(async move {
            let _ = chunker.consume_stream(audio_rx_unbounded, None).await;
        });

        // Spawn transcription task
        tauri::async_runtime::spawn(async move {
            let result: Result<String, String> = {
                #[cfg(feature = "local-transcription")]
                if provider == TranscriptionProvider::LocalParakeet {
                    match parakeet_model_path {
                        Some(model_path) => {
                            progressive_transcribe_local(&model_path, chunk_rx, None)
                                .await
                                .map_err(|e| e.to_string())
                        }
                        None => Err("Parakeet model path not configured".to_string()),
                    }
                } else {
                    progressive_transcribe_cloud(
                        &provider,
                        &api_key,
                        language.as_deref(),
                        chunk_rx,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())
                }

                #[cfg(not(feature = "local-transcription"))]
                progressive_transcribe_cloud(
                    &provider,
                    &api_key,
                    language.as_deref(),
                    chunk_rx,
                    None,
                )
                .await
                .map_err(|e| e.to_string())
            };

            let _ = result_tx.send(result);
        });

        info!("Recording started (progressive mode)");
    }

    // Store receiver for later retrieval
    *state.transcription_rx.lock().unwrap() = Some(result_rx);
    *state.recorder.lock().unwrap() = Some(recorder);
    *state.state.lock().unwrap() = RecordingState::Recording;

    Ok(())
}
