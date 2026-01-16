//! A Tauri plugin for displaying floating bubble overlays on Android.
//!
//! This plugin allows you to show a floating bubble that persists across apps,
//! similar to Facebook Messenger chat heads or Wispr Flow's voice input bubble.
//!
//! ## Platform Support
//!
//! - **Android**: Full support via `SYSTEM_ALERT_WINDOW` permission
//! - **iOS**: Not supported (platform limitation)
//! - **Desktop**: Not supported
//!
//! ## Usage
//!
//! ```rust,ignore
//! fn main() {
//!     tauri::Builder::default()
//!         .plugin(tauri_plugin_floating_bubble::init())
//!         .run(tauri::generate_context!())
//!         .expect("error while running tauri application");
//! }
//! ```

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::FloatingBubble;
#[cfg(mobile)]
use mobile::FloatingBubble;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the floating bubble APIs.
pub trait FloatingBubbleExt<R: Runtime> {
    fn floating_bubble(&self) -> &FloatingBubble<R>;
}

impl<R: Runtime, T: Manager<R>> crate::FloatingBubbleExt<R> for T {
    fn floating_bubble(&self) -> &FloatingBubble<R> {
        self.state::<FloatingBubble<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("floating-bubble")
        .invoke_handler(tauri::generate_handler![
            commands::show_bubble,
            commands::hide_bubble,
            commands::is_bubble_visible,
            commands::request_overlay_permission,
            commands::has_overlay_permission,
            commands::request_microphone_permission,
            commands::has_microphone_permission,
            commands::set_bubble_state,
            commands::handle_bubble_click,
            commands::handle_bubble_close,
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let floating_bubble = mobile::init(app, api)?;
            #[cfg(desktop)]
            let floating_bubble = desktop::init(app, api)?;
            app.manage(floating_bubble);
            Ok(())
        })
        .build()
}
