use std::sync::Mutex;
use tauri::menu::MenuItem;
use whis_core::{AudioRecorder, ApiConfig};
use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordingState {
    Idle,
    Recording,
    Transcribing,
}

pub struct AppState {
    pub state: Mutex<RecordingState>,
    pub recorder: Mutex<Option<AudioRecorder>>,
    pub api_config: Mutex<Option<ApiConfig>>,
    pub record_menu_item: Mutex<Option<MenuItem<tauri::Wry>>>,
    pub settings: Mutex<Settings>,
    /// The actual shortcut binding from the XDG Portal (Wayland only)
    pub portal_shortcut: Mutex<Option<String>>,
    /// Error message if portal shortcut binding failed
    pub portal_bind_error: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            state: Mutex::new(RecordingState::Idle),
            recorder: Mutex::new(None),
            api_config: Mutex::new(None),
            record_menu_item: Mutex::new(None),
            settings: Mutex::new(settings),
            portal_shortcut: Mutex::new(None),
            portal_bind_error: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Settings::default())
    }
}