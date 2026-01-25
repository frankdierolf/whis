//! Background service for `whis start` mode
//!
//! Runs as a long-lived process that listens for hotkey events (direct mode)
//! or IPC commands (system shortcut mode) to control recording.
//!
//! # Toggle Mode (default, cli-push-to-talk = false)
//!
//! ```text
//! ┌─────────┐  press    ┌───────────┐  press    ┌──────────────┐
//! │  Idle   │ ────────► │ Recording │ ────────► │ Transcribing │
//! └─────────┘           └───────────┘           └──────────────┘
//!      ▲                                               │
//!      └───────────────────────────────────────────────┘
//!                        (auto return)
//! ```
//!
//! # Push-to-Talk Mode (cli-push-to-talk = true)
//!
//! ```text
//! ┌─────────┐  press    ┌───────────┐  release  ┌──────────────┐
//! │  Idle   │ ────────► │ Recording │ ────────► │ Transcribing │
//! └─────────┘           └───────────┘           └──────────────┘
//!      ▲                                               │
//!      └───────────────────────────────────────────────┘
//!                        (auto return)
//! ```
//!
//! # Architecture
//!
//! - Event-driven loop using `tokio::select!` (no polling, zero CPU when idle)
//! - Progressive transcription: audio chunks sent during recording
//! - Post-processing and clipboard copy on completion

use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::app::TranscriptionConfig;
use crate::hotkey::HotkeyEvent;
use crate::ipc::{IpcMessage, IpcResponse, IpcServer};
use whis_core::{
    AudioRecorder, OutputMethod, PostProcessor, Preset, Settings, TranscriptionProvider,
    autotype_text, copy_to_clipboard, post_process, resolve_post_processor_config,
};

// Type aliases to reduce complexity warnings
type TaskHandle<T> = Arc<Mutex<Option<tokio::task::JoinHandle<T>>>>;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ServiceState {
    Idle,
    Recording,
    Transcribing,
}

pub struct Service {
    state: Arc<Mutex<ServiceState>>,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    // Store handles for background tasks (progressive transcription)
    chunker_handle: TaskHandle<Result<(), String>>,
    transcription_handle: TaskHandle<Result<String>>,
    provider: TranscriptionProvider,
    api_key: String,
    language: Option<String>,
    recording_counter: Arc<Mutex<u32>>,
    preset: Option<Preset>,
    /// CLI override for output method (e.g., --autotype flag)
    output_method_override: Option<OutputMethod>,
}

impl Service {
    pub fn new(
        config: TranscriptionConfig,
        preset: Option<Preset>,
        output_method_override: Option<OutputMethod>,
    ) -> Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(ServiceState::Idle)),
            recorder: Arc::new(Mutex::new(None)),
            chunker_handle: Arc::new(Mutex::new(None)),
            transcription_handle: Arc::new(Mutex::new(None)),
            provider: config.provider,
            api_key: config.api_key,
            language: config.language,
            recording_counter: Arc::new(Mutex::new(0)),
            preset,
            output_method_override,
        })
    }

    /// Run the service main loop
    ///
    /// Uses `tokio::select!` for event-driven operation instead of polling,
    /// eliminating CPU usage when idle.
    pub async fn run(
        &self,
        mut hotkey_rx: Option<UnboundedReceiver<HotkeyEvent>>,
        push_to_talk: bool,
    ) -> Result<()> {
        // Create IPC server
        let mut ipc_server = IpcServer::new().context("Failed to create IPC server")?;

        // Configure model caching for local transcription in listen mode
        // This respects the user's model_memory settings for speed vs memory tradeoff
        #[cfg(feature = "local-transcription")]
        {
            let settings = whis_core::Settings::load();
            let keep_loaded = settings.ui.model_memory.keep_model_loaded;
            self.provider.set_keep_loaded(keep_loaded);
        }

        loop {
            tokio::select! {
                // Wait for IPC connection
                Some(mut conn) = ipc_server.accept() => {
                    match conn.receive() {
                        Ok(message) => {
                            let response = self.handle_message(message).await;
                            let _ = conn.send(response);
                        }
                        Err(e) => {
                            eprintln!("Error receiving message: {e}");
                            let _ = conn.send(IpcResponse::Error(e.to_string()));
                        }
                    }
                }

                // Wait for hotkey event (if hotkey is configured)
                Some(event) = async {
                    match &mut hotkey_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if push_to_talk {
                        // Push-to-talk mode: press starts, release stops
                        match event {
                            HotkeyEvent::Pressed => {
                                self.handle_start().await;
                            }
                            HotkeyEvent::Released => {
                                self.handle_stop().await;
                            }
                        }
                    } else {
                        // Toggle mode: only respond to press events
                        if event == HotkeyEvent::Pressed {
                            self.handle_toggle().await;
                        }
                    }
                }
            }
        }
    }

    /// Handle an IPC message
    async fn handle_message(&self, message: IpcMessage) -> IpcResponse {
        match message {
            IpcMessage::Toggle => self.handle_toggle().await,
            IpcMessage::Stop => {
                println!("Stop signal received");
                // Return Ok response before exiting
                tokio::spawn(async {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    std::process::exit(0);
                });
                IpcResponse::Success
            }
            IpcMessage::Status => {
                let state = *self.state.lock().unwrap();
                match state {
                    ServiceState::Idle => IpcResponse::Idle,
                    ServiceState::Recording => IpcResponse::Recording,
                    ServiceState::Transcribing => IpcResponse::Transcribing,
                }
            }
        }
    }

    /// Handle toggle command (start/stop recording)
    async fn handle_toggle(&self) -> IpcResponse {
        let current_state = *self.state.lock().unwrap();

        match current_state {
            ServiceState::Idle => {
                // Increment recording counter and start recording
                let count = {
                    let mut c = self.recording_counter.lock().unwrap();
                    *c += 1;
                    *c
                };
                match self.start_recording().await {
                    Ok(_) => {
                        println!("#{count} Recording...");
                        IpcResponse::Recording
                    }
                    Err(e) => {
                        println!("#{count} error: {e}");
                        IpcResponse::Error(e.to_string())
                    }
                }
            }
            ServiceState::Recording => {
                // Stop recording and transcribe
                *self.state.lock().unwrap() = ServiceState::Transcribing;
                let count = *self.recording_counter.lock().unwrap();

                println!("#{count} Transcribing...");

                match self.stop_and_transcribe(count).await {
                    Ok(_) => {
                        *self.state.lock().unwrap() = ServiceState::Idle;
                        println!(); // blank line between transcriptions
                        IpcResponse::Success
                    }
                    Err(e) => {
                        *self.state.lock().unwrap() = ServiceState::Idle;
                        println!("#{count} error: {e}");
                        println!();
                        IpcResponse::Error(e.to_string())
                    }
                }
            }
            ServiceState::Transcribing => {
                // Already transcribing, ignore
                IpcResponse::Transcribing
            }
        }
    }

    /// Handle hotkey press (start recording) - push-to-talk mode
    async fn handle_start(&self) {
        let current_state = *self.state.lock().unwrap();

        if current_state != ServiceState::Idle {
            return; // Only start if idle
        }

        // Increment recording counter and start recording
        let count = {
            let mut c = self.recording_counter.lock().unwrap();
            *c += 1;
            *c
        };
        match self.start_recording().await {
            Ok(_) => {
                println!("#{count} Recording...");
            }
            Err(e) => {
                println!("#{count} error: {e}");
            }
        }
    }

    /// Handle hotkey release (stop recording) - push-to-talk mode
    async fn handle_stop(&self) {
        let current_state = *self.state.lock().unwrap();

        if current_state != ServiceState::Recording {
            return; // Only stop if currently recording
        }

        // Stop recording and transcribe
        *self.state.lock().unwrap() = ServiceState::Transcribing;
        let count = *self.recording_counter.lock().unwrap();

        println!("#{count} Transcribing...");

        match self.stop_and_transcribe(count).await {
            Ok(_) => {
                *self.state.lock().unwrap() = ServiceState::Idle;
                println!(); // blank line between transcriptions
            }
            Err(e) => {
                *self.state.lock().unwrap() = ServiceState::Idle;
                println!("#{count} error: {e}");
                println!();
            }
        }
    }

    /// Start recording audio with progressive transcription
    async fn start_recording(&self) -> Result<()> {
        use tokio::sync::mpsc;
        use whis_core::{ChunkerConfig, ProgressiveChunker};

        let mut recorder = AudioRecorder::new()?;

        // Configure VAD from settings
        let settings = Settings::load();
        #[cfg(feature = "vad")]
        {
            recorder.set_vad(settings.ui.vad.enabled, settings.ui.vad.threshold);
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

        // Create channels for progressive chunking
        let (chunk_tx, chunk_rx) = mpsc::unbounded_channel();

        // Create chunker config from settings
        let vad_enabled = settings.ui.vad.enabled;
        let target = settings.ui.chunk_duration_secs;
        let chunker_config = ChunkerConfig {
            target_duration_secs: target,
            min_duration_secs: target * 2 / 3,
            max_duration_secs: target * 4 / 3,
            vad_aware: vad_enabled,
        };

        // Spawn chunker task
        let mut chunker = ProgressiveChunker::new(chunker_config, chunk_tx);
        let chunker_handle = tokio::spawn(async move {
            chunker
                .consume_stream(audio_rx_unbounded, None)
                .await
                .map_err(|e| e.to_string())
        });

        // Spawn transcription task based on provider
        let provider = self.provider.clone();
        let api_key = self.api_key.clone();
        let language = self.language.clone();

        let transcription_handle = tokio::spawn(async move {
            #[cfg(feature = "local-transcription")]
            if provider == TranscriptionProvider::LocalParakeet {
                // Local Parakeet progressive transcription
                let model_path = Settings::load()
                    .transcription
                    .parakeet_model_path()
                    .ok_or_else(|| anyhow::anyhow!("Parakeet model path not configured"))?;

                return whis_core::progressive_transcribe_local(&model_path, chunk_rx, None).await;
            }

            // Cloud provider progressive transcription
            whis_core::progressive_transcribe_cloud(
                &provider,
                &api_key,
                language.as_deref(),
                chunk_rx,
                None,
            )
            .await
        });

        // Preload models in background (same as before)
        #[cfg(feature = "local-transcription")]
        {
            match self.provider {
                TranscriptionProvider::LocalWhisper => {
                    if let Some(model_path) = settings.transcription.whisper_model_path() {
                        whis_core::whisper_preload_model(&model_path);
                    }
                }
                TranscriptionProvider::LocalParakeet => {
                    if let Some(model_path) = settings.transcription.parakeet_model_path() {
                        whis_core::preload_parakeet(&model_path);
                    }
                }
                _ => {} // Cloud providers don't need preload
            }
        }

        // Store recorder and task handles
        *self.recorder.lock().unwrap() = Some(recorder);
        *self.chunker_handle.lock().unwrap() = Some(chunker_handle);
        *self.transcription_handle.lock().unwrap() = Some(transcription_handle);
        *self.state.lock().unwrap() = ServiceState::Recording;

        Ok(())
    }

    /// Stop recording and await progressive transcription completion
    async fn stop_and_transcribe(&self, count: u32) -> Result<()> {
        // Get the recorder
        let mut recorder = self
            .recorder
            .lock()
            .unwrap()
            .take()
            .context("No active recording")?;

        // Stop recording (closes audio stream, signals chunker to finish)
        recorder.stop_recording()?;

        // Get task handles
        let chunker_handle = self
            .chunker_handle
            .lock()
            .unwrap()
            .take()
            .context("No chunker task running")?;

        let transcription_handle = self
            .transcription_handle
            .lock()
            .unwrap()
            .take()
            .context("No transcription task running")?;

        // Wait for chunker to finish processing all audio
        chunker_handle
            .await
            .context("Failed to join chunker task")?
            .map_err(|e| anyhow::anyhow!("Chunker task failed: {}", e))?;

        // Wait for transcription to finish
        let transcription = transcription_handle
            .await
            .context("Failed to join transcription task")??;

        // Apply post-processing if enabled or preset is provided
        let settings = Settings::load();
        let final_text = if settings.post_processing.enabled || self.preset.is_some() {
            match resolve_post_processor_config(&self.preset, &settings) {
                Ok((processor, api_key, model, prompt)) => {
                    // Re-warm Ollama model if needed
                    if processor == PostProcessor::Ollama && model.is_some() {
                        settings.services.ollama.preload();
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    }

                    println!("#{count} Post-processing...");

                    match post_process(
                        &transcription,
                        &processor,
                        &api_key,
                        &prompt,
                        model.as_deref(),
                    )
                    .await
                    {
                        Ok(processed) => {
                            println!("#{count} Done.");
                            processed
                        }
                        Err(e) => {
                            eprintln!("#{count} Post-processing failed: {e}");
                            println!("#{count} Done.");
                            transcription
                        }
                    }
                }
                Err(e) => {
                    eprintln!("#{count} Post-processing config error: {e}");
                    println!("#{count} Done.");
                    transcription
                }
            }
        } else {
            println!("#{count} Done.");
            transcription
        };

        // Output based on configured method (blocking operation)
        // Use CLI override if present, otherwise use settings from config file
        let clipboard_method = settings.ui.clipboard_backend.clone();
        let output_method = self
            .output_method_override
            .clone()
            .unwrap_or(settings.ui.output_method.clone());
        let autotype_backend = settings.ui.autotype_backend.clone();
        let autotype_delay_ms = settings.ui.autotype_delay_ms;

        tokio::task::spawn_blocking(move || {
            match output_method {
                OutputMethod::Clipboard => {
                    copy_to_clipboard(&final_text, clipboard_method)?;
                }
                OutputMethod::Autotype => {
                    autotype_text(&final_text, autotype_backend, autotype_delay_ms)?;
                }
                OutputMethod::Both => {
                    copy_to_clipboard(&final_text, clipboard_method)?;
                    autotype_text(&final_text, autotype_backend, autotype_delay_ms)?;
                }
            }
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("Failed to join task")??;

        Ok(())
    }
}
