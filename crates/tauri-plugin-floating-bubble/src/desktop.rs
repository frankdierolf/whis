use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

/// Initializes the desktop plugin stub.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<FloatingBubble<R>> {
    Ok(FloatingBubble(PhantomData))
}

/// Access to the floating bubble APIs (desktop stub - not supported).
pub struct FloatingBubble<R: Runtime>(PhantomData<R>);

// SAFETY: FloatingBubble is a stub that holds no actual data, only PhantomData for type checking.
unsafe impl<R: Runtime> Send for FloatingBubble<R> {}
unsafe impl<R: Runtime> Sync for FloatingBubble<R> {}

impl<R: Runtime> FloatingBubble<R> {
    pub fn show(&self, _options: BubbleOptions) -> crate::Result<()> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn hide(&self) -> crate::Result<()> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn is_visible(&self) -> crate::Result<VisibilityResponse> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn request_permission(&self) -> crate::Result<PermissionResponse> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn has_permission(&self) -> crate::Result<PermissionResponse> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn set_state(&self, _state: String) -> crate::Result<()> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn bring_to_foreground(&self) -> crate::Result<()> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn request_microphone_permission(&self) -> crate::Result<PermissionResponse> {
        Err(crate::Error::UnsupportedPlatform)
    }

    pub fn has_microphone_permission(&self) -> crate::Result<PermissionResponse> {
        Err(crate::Error::UnsupportedPlatform)
    }
}
