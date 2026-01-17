use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a specific bubble state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateConfig {
    /// Icon resource name for this state (optional).
    /// If not provided, uses the default icon from BubbleOptions.
    /// Example: "ic_recording"
    pub icon_resource_name: Option<String>,
}

/// Options for configuring the floating bubble.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BubbleOptions {
    /// Size of the bubble in dp (density-independent pixels).
    /// Default: 60
    #[serde(default = "default_size")]
    pub size: i32,

    /// Initial X position of the bubble.
    /// Default: 0 (left edge)
    #[serde(default)]
    pub start_x: i32,

    /// Initial Y position of the bubble.
    /// Default: 100
    #[serde(default = "default_start_y")]
    pub start_y: i32,

    /// Default icon resource name (used when no state-specific icon is provided).
    /// Android drawable resource name (without "R.drawable." prefix).
    /// If not specified, uses the plugin's default icon.
    /// Example: "ic_my_app_logo"
    #[serde(default)]
    pub icon_resource_name: Option<String>,

    /// Background color (hex string, e.g., "#1C1C1C").
    /// Default: "#1C1C1C" (dark)
    #[serde(default = "default_background_color")]
    pub background: String,

    /// State configuration mapping.
    /// Keys are arbitrary state names, values define icon for that state.
    /// Example: { "idle": { iconResourceName: "ic_idle" }, "active": { iconResourceName: "ic_active" } }
    #[serde(default)]
    pub states: HashMap<String, StateConfig>,

    /// Notification configuration for different states.
    /// Allows customizing the foreground service notification text.
    #[serde(default)]
    pub notifications: Option<NotificationConfig>,
}

fn default_size() -> i32 {
    60
}

fn default_start_y() -> i32 {
    100
}

fn default_background_color() -> String {
    "#1C1C1C".to_string()
}

/// Response from visibility check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibilityResponse {
    pub visible: bool,
}

/// Response from permission check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponse {
    pub granted: bool,
}

/// Options for setting bubble state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateOptions {
    /// The state name to set. Must be a key in the states map provided to showBubble.
    pub state: String,
}

/// Content for a notification.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationContent {
    /// Title of the notification.
    pub title: String,
    /// Text/body of the notification.
    pub text: String,
}

/// Configuration for notifications at different states.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationConfig {
    /// Notification content for each state.
    /// Keys are state names (e.g., "idle", "capturing", "processing").
    pub state_notifications: HashMap<String, NotificationContent>,
}
