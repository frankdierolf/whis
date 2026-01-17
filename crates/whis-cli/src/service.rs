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
//! - Polling loop checks IPC server + hotkey channel (non-blocking)
//! - Progressive transcription: audio chunks sent during recording
//! - Post-processing and clipboard copy on completion

use anyhow::{Context, Result};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use tokio::time::sleep;

use crate::app::TranscriptionConfig;
use crate::hotkey::HotkeyEvent;
use crate::ipc::{IpcMessage, IpcResponse, IpcServer};
use std::time::Duration;
use whis_core::{
    AudioRecorder, DEFAULT_POST_PROCESSING_PROMPT, Settings, TranscriptionProvider,
    copy_to_clipboard, post_process,
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
}

impl Service {
    pub fn new(config: TranscriptionConfig) -> Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(ServiceState::Idle)),
            recorder: Arc::new(Mutex::new(None)),
            chunker_handle: Arc::new(Mutex::new(None)),
            transcription_handle: Arc::new(Mutex::new(None)),
            provider: config.provider,
            api_key: config.api_key,
            language: config.language,
            recording_counter: Arc::new(Mutex::new(0)),
        })
    }

    /// Run the service main loop
    pub async fn run(
        &self,
        hotkey_rx: Option<Receiver<HotkeyEvent>>,
        push_to_talk: bool,
    ) -> Result<()> {
        // Create IPC server
        let ipc_server = IpcServer::new().context("Failed to create IPC server")?;

        // Configure model caching for local transcription in listen mode
        // This respects the user's model_memory settings for speed vs memory tradeoff
        #[cfg(feature = "local-transcription")]
        {
            let settings = whis_core::Settings::load();
            let keep_loaded = settings.ui.model_memory.keep_model_loaded;
            self.provider.set_keep_loaded(keep_loaded);
        }

        loop {
            // Check for incoming IPC connections (non-blocking)
            if let Some(mut conn) = ipc_server.try_accept()? {
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

            // Check for hotkey events
            if let Some(ref rx) = hotkey_rx {
                if let Ok(event) = rx.try_recv() {
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

            // Small sleep to prevent busy waiting
            sleep(Duration::from_millis(10)).await;
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

        // Print completion message immediately after transcription finishes
        println!("#{count} Done.");

        // Apply post-processing if enabled
        let settings = Settings::load();
        let final_text = if settings.post_processing.enabled {
            if let Some(post_processor_api_key) = settings
                .post_processing
                .api_key(&settings.transcription.api_keys)
            {
                println!("#{count} Post-processing...");

                let prompt = settings
                    .post_processing
                    .prompt
                    .as_deref()
                    .unwrap_or(DEFAULT_POST_PROCESSING_PROMPT);

                match post_process(
                    &post_processor_api_key,
                    &settings.post_processing.processor,
                    &transcription,
                    prompt,
                    None,
                )
                .await
                {
                    Ok(processed) => processed,
                    Err(_) => transcription, // Silently fallback in service mode
                }
            } else {
                transcription
            }
        } else {
            transcription
        };

        // Copy to clipboard (blocking operation)
        let clipboard_method = settings.ui.clipboard_backend.clone();
        tokio::task::spawn_blocking(move || copy_to_clipboard(&final_text, clipboard_method))
            .await
            .context("Failed to join task")??;

        Ok(())
    }
}
