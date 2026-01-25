//! Global Keyboard Shortcuts Module
//!
//! Provides cross-platform global keyboard shortcut support with multiple backends:
//! - **TauriPlugin**: X11, macOS, Windows (native shortcuts)
//! - **PortalGlobalShortcuts**: Wayland with XDG Desktop Portal (GNOME 48+, KDE, Hyprland)
//! - **ManualSetup**: Fallback to compositor configuration + IPC toggle
//!
//! ## Architecture
//!
//! ```text
//! shortcuts/
//! ├── backend.rs           - Backend detection & capability
//! ├── tauri_plugin.rs      - X11/macOS/Windows implementation
//! ├── portal/              - Wayland portal implementation
//! │   ├── mod.rs           - Portal setup & event listening
//! │   ├── binding.rs       - Shortcut binding & configuration
//! │   ├── registry.rs      - App ID registration
//! │   └── dconf.rs         - GNOME dconf integration
//! ├── ipc.rs               - Unix socket toggle server
//! ├── manual.rs            - Manual setup instructions
//! └── mod.rs               - Public API
//! ```

pub mod backend;
pub mod instructions;
pub mod ipc;
pub mod manual;
pub mod portal;
#[cfg(target_os = "linux")]
pub mod rdev_grab;
pub mod tauri_plugin;

// Re-export backend detection
pub use backend::{
    ShortcutBackend, ShortcutBackendInfo, ShortcutCapability, backend_info, detect_backend,
    portal_version,
};

// Re-export portal functions (Linux only)
#[cfg(target_os = "linux")]
pub use portal::{
    bind_shortcut_with_trigger, configure_with_preferred_trigger, open_configure_shortcuts,
    read_gnome_custom_shortcut, read_gnome_custom_shortcut_command,
    read_portal_shortcut_from_dconf, register_app_with_portal, setup_portal_shortcuts,
};

// Re-export tauri plugin functions
pub use tauri_plugin::{setup_tauri_shortcut, update_tauri_shortcut};

// Re-export rdev_grab functions (Linux only)
#[cfg(target_os = "linux")]
pub use rdev_grab::{RdevGrabGuard, setup_rdev_grab};

// Re-export IPC functions
pub use ipc::{send_toggle_command, start_ipc_listener};

// Re-export manual instructions
pub use manual::print_manual_setup_instructions;

// Re-export instructions for UI
pub use instructions::{get_config_path, get_config_snippet, get_instructions};

use tauri::{AppHandle, Manager};

/// Format platform name for display (e.g., "Wayland", "X11", "macOS")
fn platform_display_name(platform: &whis_core::platform::Platform) -> &'static str {
    match platform {
        whis_core::platform::Platform::MacOS => "macOS",
        whis_core::platform::Platform::Windows => "Windows",
        whis_core::platform::Platform::LinuxX11 => "X11",
        whis_core::platform::Platform::LinuxWayland => "Wayland",
    }
}

/// Setup shortcuts based on detected backend
pub fn setup_shortcuts(app: &tauri::App) {
    let capability = detect_backend();
    let state = app.state::<crate::state::AppState>();
    let settings = state.settings.lock().unwrap();
    let shortcut_str = settings.shortcuts.desktop_key.clone();
    drop(settings);

    let compositor_name = capability.platform_info.compositor.display_name();
    let platform_name = platform_display_name(&capability.platform_info.platform);
    println!(
        "Detected environment: {} ({})",
        compositor_name, platform_name
    );

    match capability.backend {
        ShortcutBackend::TauriPlugin => {
            if let Err(e) = setup_tauri_shortcut(app, &shortcut_str) {
                eprintln!("Shortcut setup failed: {e}");
                print_manual_setup_instructions(
                    &capability.platform_info.compositor,
                    &shortcut_str,
                );
            } else {
                println!("Global shortcut registered: {shortcut_str}");
            }
        }
        #[cfg(target_os = "linux")]
        ShortcutBackend::RdevGrab => {
            match setup_rdev_grab(app, &shortcut_str) {
                Ok(guard) => {
                    // Store the guard to keep the thread alive
                    state.rdev_guard.lock().unwrap().replace(guard);
                    // Clear any previous error
                    state.rdev_grab_error.lock().unwrap().take();
                    println!("Direct shortcut registered: {shortcut_str}");
                }
                Err(e) => {
                    // Store the error for UI display
                    state.rdev_grab_error.lock().unwrap().replace(e.to_string());
                    eprintln!("Direct shortcut unavailable (permission denied)");
                    print_manual_setup_instructions(
                        &capability.platform_info.compositor,
                        &shortcut_str,
                    );
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        ShortcutBackend::RdevGrab => {
            // RdevGrab only available on Linux
            print_manual_setup_instructions(&capability.platform_info.compositor, &shortcut_str);
        }
        #[cfg(target_os = "linux")]
        ShortcutBackend::PortalGlobalShortcuts => {
            let app_handle = app.handle().clone();
            let app_handle_for_state = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let toggle_handle = app_handle.clone();
                if let Err(e) = setup_portal_shortcuts(
                    shortcut_str,
                    move || {
                        let handle = toggle_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::recording::toggle_recording(handle);
                        });
                    },
                    app_handle_for_state,
                )
                .await
                {
                    eprintln!("Portal shortcut setup failed: {e}");
                }
            });
        }
        #[cfg(not(target_os = "linux"))]
        ShortcutBackend::PortalGlobalShortcuts => {
            // Portal shortcuts only available on Linux
            print_manual_setup_instructions(&capability.platform_info.compositor, &shortcut_str);
        }
        ShortcutBackend::ManualSetup => {
            print_manual_setup_instructions(&capability.platform_info.compositor, &shortcut_str);
        }
    }
}

/// Update shortcut. Returns Ok(true) if restart is needed, Ok(false) if applied immediately.
pub fn update_shortcut(
    app: &AppHandle,
    new_shortcut: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let capability = detect_backend();

    match capability.backend {
        ShortcutBackend::TauriPlugin => {
            update_tauri_shortcut(app, new_shortcut)?;
            Ok(false) // No restart needed
        }
        _ => {
            // For portals and CLI, dynamic updates require restart.
            println!("Shortcut saved. Restart required for changes to take effect.");
            Ok(true) // Restart needed
        }
    }
}
