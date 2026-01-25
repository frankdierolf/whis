<div align="center">
<img src="https://raw.githubusercontent.com/frankdierolf/whis/main/crates/whis-desktop/icons/128x128.png" alt="whis" width="80" height="80" />

<h3>whis-core</h3>
<p>
  Core library for whis voice-to-text functionality.
  <br />
  <a href="https://whis.ink">Website</a>
  ·
  <a href="https://github.com/frankdierolf/whis/tree/main/crates/whis-cli">CLI</a>
  ·
  <a href="https://github.com/frankdierolf/whis/tree/main/crates/whis-desktop">Desktop</a>
  ·
  <a href="https://github.com/frankdierolf/whis/tree/main/crates/whis-mobile">Mobile</a>
</p>
</div>

## Features

- **Audio recording** — capture microphone input
- **Transcription** — cloud providers or local models
- **Progressive chunking** — transcribe long recordings in real-time
- **LLM post-processing** — transform transcripts with custom prompts
- **Clipboard** — copy results to system clipboard
- **Config management** — persistent settings (platform-aware paths)

## Usage

```rust
use whis_core::{
    AudioRecorder, TranscriptionProvider, Settings,
    ProgressiveChunker, ChunkerConfig, ProgressiveChunk,
    progressive_transcribe_cloud, copy_to_clipboard, ClipboardMethod,
};
use tokio::sync::mpsc;

// Load settings and get provider config
let settings = Settings::load();
let provider = settings.transcription.provider.clone();
let api_key = settings.transcription.api_key_for(&provider).unwrap();

// Set up progressive chunking channel
let (chunk_tx, chunk_rx) = mpsc::unbounded_channel::<ProgressiveChunk>();
let chunker = ProgressiveChunker::new(ChunkerConfig::default(), chunk_tx);

// Record audio
let mut recorder = AudioRecorder::new()?;
recorder.start_recording()?;
// ... feed audio samples to chunker during recording ...
let recording = recorder.stop_recording()?;

// Transcribe progressively
let text = progressive_transcribe_cloud(
    &provider,
    &api_key,
    None, // language hint
    chunk_rx,
    None, // progress callback
).await?;

// Copy to clipboard
copy_to_clipboard(&text, ClipboardMethod::Auto)?;
```

For simpler use cases, see the CLI implementation in `whis-cli` which handles
the recording → chunking → transcription → clipboard pipeline.

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `embedded-encoder` | Yes | MP3 encoding via embedded LAME library |
| `clipboard` | Yes | Clipboard support via arboard/xclip/wl-copy |
| `local-transcription` | Yes | Local transcription via Whisper/Parakeet (requires model) |
| `vad` | Yes | Voice Activity Detection to skip silence |
| `realtime` | Yes | OpenAI/Deepgram Realtime API for streaming |
| `autotyping` | Yes | Type text directly into active window (wtype/xdotool/enigo) |

## Modules

| Module | Description |
|--------|-------------|
| `audio` | `AudioRecorder`, `ProgressiveChunker`, `RecordingData`, VAD processing |
| `transcription` | Progressive transcription, post-processing, Ollama integration |
| `provider` | Provider registry and `TranscriptionBackend` trait |
| `configuration` | `TranscriptionProvider` enum, presets, defaults |
| `settings` | User preferences (provider, API keys, language, hotkeys) |
| `clipboard` | System clipboard operations with multiple backends |
| `autotyping` | Type text into active window (platform-specific backends) |
| `model` | Whisper/Parakeet model management |
| `state` | Recording state machine |
| `verbose` | Debug logging utilities |

## License

MIT
