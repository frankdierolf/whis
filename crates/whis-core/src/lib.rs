pub mod audio;
pub mod clipboard;
pub mod config;
pub mod settings;
pub mod transcribe;

pub use audio::{AudioChunk, AudioRecorder, AudioResult};
pub use clipboard::copy_to_clipboard;
pub use config::Config;
pub use settings::Settings;
pub use transcribe::{ChunkTranscription, parallel_transcribe, transcribe_audio};
