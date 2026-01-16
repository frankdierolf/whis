const COMMANDS: &[&str] = &[
    "show_bubble",
    "hide_bubble",
    "is_bubble_visible",
    "request_overlay_permission",
    "has_overlay_permission",
    "request_microphone_permission",
    "has_microphone_permission",
    "set_bubble_state",
    "handle_bubble_click",
    "handle_bubble_close",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
