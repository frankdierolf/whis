//! Record Command - Voice-to-Text Pipeline
//!
//! This module implements the main recording/transcription workflow using a clean
//! pipeline architecture. It captures audio from the microphone, transcribes it
//! using configured providers, optionally post-processes the text, and outputs
//! the result.
//!
//! # Architecture
//!
//! Microphone input uses progressive transcription:
//!
//! ```text
//! ┌─────────────┐      ┌──────────────┐      ┌─────────────┐      ┌────────────┐
//! │   Record    │  →   │  Progressive │  →   │   Process   │  →   │   Output   │
//! │  (modes/)   │      │  Transcribe  │      │ (pipeline/) │      │(pipeline/) │
//! └─────────────┘      └──────────────┘      └─────────────┘      └────────────┘
//!     ↓                      ↓                     ↓                    ↓
//! Microphone          Sequential chunks       LLM Cleanup         Clipboard/
//!                     to provider API         Preset Transform    Stdout
//!                     ~90s chunks
//! ```
//!
//! # Pipeline Phases
//!
//! 1. **Record Phase** (`modes/`): Capture audio from microphone with VAD
//!
//! 2. **Transcribe Phase**: Progressive transcription
//!    - Audio chunked into ~90s segments with overlap
//!    - Chunks transcribed sequentially (cloud or local)
//!    - Results merged with overlap deduplication
//!
//! 3. **Process Phase** (`pipeline/process.rs`): Enhance transcript
//!    - Apply LLM post-processing (grammar, filler words)
//!    - Transform with output presets
//!
//! 4. **Output Phase** (`pipeline/output.rs`): Deliver result
//!    - Copy to clipboard (default)
//!    - Print to stdout (--print flag)
//!
//! # Configuration
//!
//! The record command respects user settings from `~/.config/whis/config.toml`:
//! - Transcription provider and API keys
//! - Post-processing preferences
//! - VAD settings and hotkeys
//! - Clipboard method

mod modes;
mod pipeline;
mod types;

// Re-export public types for external use
pub use types::RecordConfig;

use anyhow::Result;

use crate::app;

/// Execute the record command with clean pipeline phases
pub fn run(config: RecordConfig) -> Result<()> {
    let quiet = config.is_quiet();

    // Create Tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;

    // Load transcription configuration (with optional language override)
    let transcription_config =
        app::load_transcription_config_with_language(config.language.clone())?;

    // Branch: file transcription vs microphone recording
    let transcription_result = if let Some(ref input_file) = config.input_file {
        // File transcription mode
        runtime.block_on(transcribe_file(input_file, &transcription_config, quiet))?
    } else {
        // Microphone: Record and transcribe concurrently (streaming)
        let mic_config = modes::MicrophoneConfig {
            duration: config.duration,
            no_vad: config.no_vad,
            provider: transcription_config.provider.clone(),
            will_post_process: config.post_process || config.preset.is_some(),
        };
        runtime.block_on(progressive_record_and_transcribe(
            mic_config,
            &transcription_config,
            quiet,
        ))?
    };

    // Phase 3: Post-process and apply presets
    let processing_cfg = pipeline::ProcessingConfig {
        enabled: config.post_process,
        preset: config.preset,
    };
    let processed_result = runtime.block_on(pipeline::process(
        transcription_result,
        &processing_cfg,
        quiet,
    ))?;

    // Print completion after all processing is done
    if !quiet {
        println!(" Done.");
    }

    // Phase 4: Output (print, file, type to window, or clipboard)
    let output_mode = if config.print {
        pipeline::OutputMode::Print
    } else if let Some(path) = config.output_path {
        pipeline::OutputMode::File(path)
    } else {
        pipeline::OutputMode::Clipboard
    };
    pipeline::output(processed_result, output_mode, config.format, quiet)?;

    Ok(())
}

/// Progressive recording + transcription (combines recording and transcription phases)
///
/// This function overlaps recording and transcription using the progressive
/// architecture. Audio is chunked during recording and transcribed immediately
/// (cloud providers transcribe chunks in parallel, local providers sequentially).
///
/// For realtime providers (deepgram-realtime, openai-realtime), audio is streamed
/// directly to WebSocket without chunking for lower latency.
async fn progressive_record_and_transcribe(
    mic_config: modes::MicrophoneConfig,
    transcription_config: &app::TranscriptionConfig,
    quiet: bool,
) -> Result<types::TranscriptionResult> {
    use tokio::sync::mpsc;
    #[cfg(feature = "local-transcription")]
    use whis_core::progressive_transcribe_local;
    use whis_core::{
        AudioRecorder, ChunkerConfig, ProgressiveChunker, Settings, TranscriptionProvider,
        WarmupConfig, progressive_transcribe_cloud, warmup_configured,
    };

    // Check if this is a realtime provider (for branching later)
    let is_realtime = whis_core::is_realtime_provider(&transcription_config.provider);

    // Create recorder
    let mut recorder = AudioRecorder::new()?;

    // Configure VAD (disabled for realtime - they handle silence detection)
    let settings = Settings::load_cli();
    let vad_enabled = settings.ui.vad.enabled && !mic_config.no_vad && !is_realtime;
    recorder.set_vad(vad_enabled, settings.ui.vad.threshold);

    // Preload models in background (same as batch mode)
    preload_models(&mic_config);

    // Warm up HTTP client and cloud connections in background
    // This overlaps with user speaking, so connection is ready when transcription starts
    {
        let provider = transcription_config.provider.to_string();
        let api_key = transcription_config.api_key.clone();

        // Load settings once for post-processing config
        let (post_processor, post_processor_api_key) = if mic_config.will_post_process {
            let settings = Settings::load_cli();
            let processor = match &settings.post_processing.processor {
                whis_core::PostProcessor::None => None,
                p => Some(p.to_string()),
            };
            let pp_api_key = if processor.is_some() {
                settings
                    .post_processing
                    .api_key(&settings.transcription.api_keys)
            } else {
                None
            };
            (processor, pp_api_key)
        } else {
            (None, None)
        };

        let config = WarmupConfig {
            provider: Some(provider),
            provider_api_key: Some(api_key),
            post_processor,
            post_processor_api_key,
        };

        tokio::spawn(async move {
            let _ = warmup_configured(&config).await;
        });
    }

    // Start streaming recording with configured device
    let device_name = settings.ui.microphone_device.clone();
    let mut audio_rx_bounded =
        recorder.start_recording_streaming_with_device(device_name.as_deref())?;

    // Create unbounded channel for chunker (adapter pattern)
    let (audio_tx_unbounded, audio_rx_unbounded) = mpsc::unbounded_channel();

    // Spawn adapter task to forward from bounded to unbounded channel
    tokio::spawn(async move {
        while let Some(samples) = audio_rx_bounded.recv().await {
            if audio_tx_unbounded.send(samples).is_err() {
                break; // Receiver dropped
            }
        }
    });

    // Branch based on provider type: realtime streaming vs chunked progressive
    let (transcription_task, chunker_task): (
        tokio::task::JoinHandle<anyhow::Result<String>>,
        Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
    ) = if is_realtime {
        // REALTIME PATH: Stream audio directly to WebSocket (no chunking)
        #[cfg(feature = "realtime")]
        {
            let realtime_backend = whis_core::get_realtime_backend(&transcription_config.provider)?;
            let api_key = transcription_config.api_key.clone();
            let language = transcription_config.language.clone();

            let task = tokio::spawn(async move {
                realtime_backend
                    .transcribe_stream(&api_key, audio_rx_unbounded, language)
                    .await
            });

            (task, None) // No chunker task for realtime
        }

        #[cfg(not(feature = "realtime"))]
        {
            anyhow::bail!(
                "Provider '{}' requires the 'realtime' feature (not enabled in this build)",
                transcription_config.provider.as_str()
            );
        }
    } else {
        // NON-REALTIME PATH: Use chunking + progressive transcription
        let (chunk_tx, chunk_rx) = mpsc::unbounded_channel();

        // Create chunker config from settings
        let target = settings.ui.chunk_duration_secs;
        let chunker_config = ChunkerConfig {
            target_duration_secs: target,
            min_duration_secs: target * 2 / 3,
            max_duration_secs: target * 4 / 3,
            vad_aware: vad_enabled,
        };

        // Spawn chunker task
        let mut chunker = ProgressiveChunker::new(chunker_config, chunk_tx);
        let chunker_task = tokio::spawn(async move {
            chunker
                .consume_stream(audio_rx_unbounded, None)
                .await
                .map_err(|e| anyhow::anyhow!(e))
        });

        // Spawn transcription task based on provider
        let transcription_task = {
            let provider = transcription_config.provider.clone();
            let api_key = transcription_config.api_key.clone();
            let language = transcription_config.language.clone();

            tokio::spawn(async move {
                #[cfg(feature = "local-transcription")]
                if provider == TranscriptionProvider::LocalParakeet {
                    // Local Parakeet progressive transcription
                    let model_path = Settings::load_cli()
                        .transcription
                        .parakeet_model_path()
                        .ok_or_else(|| anyhow::anyhow!("Parakeet model path not configured"))?;

                    return progressive_transcribe_local(&model_path, chunk_rx, None).await;
                }

                // Cloud provider progressive transcription
                progressive_transcribe_cloud(
                    &provider,
                    &api_key,
                    language.as_deref(),
                    chunk_rx,
                    None,
                )
                .await
            })
        };

        (transcription_task, Some(chunker_task))
    };

    // Wait for recording to complete (user input or duration)
    if let Some(dur) = mic_config.duration {
        // Timed recording
        if !quiet {
            if whis_core::verbose::is_verbose() {
                println!("Recording for {} seconds...", dur.as_secs());
            } else {
                print!("Recording for {} seconds...", dur.as_secs());
                use std::io::Write;
                std::io::stdout().flush()?;
            }
        }
        tokio::time::sleep(dur).await;
    } else {
        // Interactive mode
        if !quiet {
            println!("Press Enter to stop");
            if whis_core::verbose::is_verbose() {
                println!("Recording...");
            } else {
                print!("Recording...");
                use std::io::Write;
                std::io::stdout().flush()?;
            }
        }

        // Wait for user to stop (blocking operation)
        tokio::task::spawn_blocking(app::wait_for_stop).await??;
    }

    // Stop recording (closes audio stream, signals chunker/realtime to finish)
    recorder.stop_recording()?;

    // Wait for chunker to finish (only for non-realtime path)
    if let Some(chunker_task) = chunker_task {
        chunker_task.await??;
    }

    // Wait for transcription to finish
    if !quiet {
        app::print_status(" Transcribing...", Some(&transcription_config.provider));
    }

    let text = transcription_task.await??;

    Ok(types::TranscriptionResult { text })
}

/// Preload models in background to reduce latency (extracted from MicrophoneMode)
fn preload_models(config: &modes::MicrophoneConfig) {
    #[cfg(feature = "local-transcription")]
    {
        let settings = whis_core::Settings::load_cli();

        // Preload the configured local model (Whisper OR Parakeet, not both)
        match config.provider {
            whis_core::TranscriptionProvider::LocalWhisper => {
                if let Some(model_path) = settings.transcription.whisper_model_path() {
                    whis_core::whisper_preload_model(&model_path);
                }
            }
            whis_core::TranscriptionProvider::LocalParakeet => {
                if let Some(model_path) = settings.transcription.parakeet_model_path() {
                    whis_core::preload_parakeet(&model_path);
                }
            }
            _ => {} // Cloud providers don't need preload
        }
    }

    // Preload Ollama if post-processing enabled
    if config.will_post_process {
        let settings = whis_core::Settings::load_cli();
        if settings.post_processing.processor == whis_core::PostProcessor::Ollama {
            settings.services.ollama.preload();
        }
    }
}

/// Transcribe an audio file
async fn transcribe_file(
    input_file: &std::path::Path,
    transcription_config: &app::TranscriptionConfig,
    quiet: bool,
) -> Result<types::TranscriptionResult> {
    use whis_core::{TranscriptionProvider, http::get_http_client, provider::TranscriptionRequest};

    if !quiet {
        eprintln!(
            "Transcribing {}...",
            input_file.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    // Read audio file and convert to 16kHz mono samples
    let samples = modes::file::read_audio_file(input_file)?;

    // Handle local vs cloud providers differently
    let text = match &transcription_config.provider {
        #[cfg(feature = "local-transcription")]
        TranscriptionProvider::LocalParakeet => {
            let model_path = whis_core::Settings::load_cli()
                .transcription
                .parakeet_model_path()
                .ok_or_else(|| anyhow::anyhow!("Parakeet model path not configured"))?;

            tokio::task::spawn_blocking(move || {
                whis_core::provider::transcribe_raw_parakeet(&model_path, samples)
            })
            .await??
            .text
        }

        #[cfg(feature = "local-transcription")]
        TranscriptionProvider::LocalWhisper => {
            let model_path = transcription_config.api_key.clone();
            let language = transcription_config.language.clone();
            tokio::task::spawn_blocking(move || {
                whis_core::provider::transcribe_raw(&model_path, &samples, language.as_deref())
            })
            .await??
            .text
        }

        _ => {
            // Cloud providers: encode to MP3 and send
            let encoder = whis_core::audio::create_encoder();
            let mp3_data =
                encoder.encode_samples(&samples, whis_core::resample::WHISPER_SAMPLE_RATE)?;

            let client = get_http_client()?;
            let provider =
                whis_core::provider::registry().get_by_kind(&transcription_config.provider)?;

            let request = TranscriptionRequest {
                audio_data: mp3_data,
                language: transcription_config.language.clone(),
                filename: format!(
                    "{}.mp3",
                    input_file.file_stem().unwrap_or_default().to_string_lossy()
                ),
                mime_type: "audio/mpeg".to_string(),
                progress: None,
            };

            provider
                .transcribe_async(client, &transcription_config.api_key, request)
                .await?
                .text
        }
    };

    if !quiet {
        eprintln!("Done.");
    }

    Ok(types::TranscriptionResult { text })
}
