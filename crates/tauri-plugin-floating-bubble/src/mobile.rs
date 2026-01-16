use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

/// Initializes the mobile plugin.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<FloatingBubble<R>> {
    #[cfg(target_os = "ios")]
    return Err(crate::Error::UnsupportedPlatform);

    #[cfg(target_os = "android")]
    {
        let handle =
            api.register_android_plugin("ink.whis.floatingbubble", "FloatingBubblePlugin")?;
        Ok(FloatingBubble(handle))
    }
}

/// Access to the floating bubble APIs.
pub struct FloatingBubble<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> FloatingBubble<R> {
    /// Show the floating bubble with the given options.
    pub fn show(&self, options: BubbleOptions) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("showBubble", options)
            .map_err(Into::into)
    }

    /// Hide the floating bubble.
    pub fn hide(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("hideBubble", ())
            .map_err(Into::into)
    }

    /// Check if the bubble is currently visible.
    pub fn is_visible(&self) -> crate::Result<VisibilityResponse> {
        self.0
            .run_mobile_plugin("isBubbleVisible", ())
            .map_err(Into::into)
    }

    /// Request overlay permission.
    pub fn request_permission(&self) -> crate::Result<PermissionResponse> {
        self.0
            .run_mobile_plugin("requestOverlayPermission", ())
            .map_err(Into::into)
    }

    /// Check if overlay permission is granted.
    pub fn has_permission(&self) -> crate::Result<PermissionResponse> {
        self.0
            .run_mobile_plugin("hasOverlayPermission", ())
            .map_err(Into::into)
    }

    /// Set the bubble's visual state.
    pub fn set_state(&self, state: String) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("setBubbleState", StateOptions { state })
            .map_err(Into::into)
    }

    /// Request microphone permission (RECORD_AUDIO).
    pub fn request_microphone_permission(&self) -> crate::Result<PermissionResponse> {
        self.0
            .run_mobile_plugin("requestMicrophonePermission", ())
            .map_err(Into::into)
    }

    /// Check if microphone permission is granted.
    pub fn has_microphone_permission(&self) -> crate::Result<PermissionResponse> {
        self.0
            .run_mobile_plugin("hasMicrophonePermission", ())
            .map_err(Into::into)
    }
}
