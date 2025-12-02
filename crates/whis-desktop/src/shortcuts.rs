use serde::Serialize;
use std::env;
use std::str::FromStr;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Backend for global keyboard shortcuts
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShortcutBackend {
    /// Tauri plugin - works on X11, macOS, Windows
    TauriPlugin,
    /// XDG Portal GlobalShortcuts - works on Wayland with GNOME 48+, KDE, Hyprland
    PortalGlobalShortcuts,
    /// Manual setup - user configures compositor to run `whis-desktop --toggle`
    ManualSetup,
}

/// Information about shortcut capability on current system
pub struct ShortcutCapability {
    pub backend: ShortcutBackend,
    pub compositor: String,
}

/// Backend info for frontend consumption
#[derive(Debug, Clone, Serialize)]
pub struct ShortcutBackendInfo {
    pub backend: String,
    pub requires_restart: bool,
    pub compositor: String,
    pub portal_version: u32,
}

/// Get the GlobalShortcuts portal version (0 if unavailable)
pub fn portal_version() -> u32 {
    std::process::Command::new("busctl")
        .args([
            "--user",
            "get-property",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.GlobalShortcuts",
            "version",
        ])
        .output()
        .ok()
        .and_then(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            // Output format: "u 1" or "u 2"
            output.split_whitespace().last()?.parse().ok()
        })
        .unwrap_or(0)
}

/// Get backend info for the frontend
pub fn backend_info() -> ShortcutBackendInfo {
    let capability = detect_backend();
    let portal_version = if capability.backend == ShortcutBackend::PortalGlobalShortcuts {
        portal_version()
    } else {
        0
    };

    ShortcutBackendInfo {
        backend: format!("{:?}", capability.backend),
        requires_restart: !matches!(capability.backend, ShortcutBackend::TauriPlugin),
        compositor: capability.compositor,
        portal_version,
    }
}

/// Detect the best shortcut backend for the current environment
pub fn detect_backend() -> ShortcutCapability {
    let session_type = env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland_display = env::var("WAYLAND_DISPLAY").is_ok();

    // Check if running on Wayland
    if session_type == "wayland" || wayland_display {
        if is_portal_available() {
            ShortcutCapability {
                backend: ShortcutBackend::PortalGlobalShortcuts,
                compositor: detect_compositor(),
            }
        } else {
            ShortcutCapability {
                backend: ShortcutBackend::ManualSetup,
                compositor: detect_compositor(),
            }
        }
    } else {
        // X11 or other - use Tauri plugin
        ShortcutCapability {
            backend: ShortcutBackend::TauriPlugin,
            compositor: "X11".into(),
        }
    }
}

/// Check if GlobalShortcuts portal is available via D-Bus
fn is_portal_available() -> bool {
    std::process::Command::new("busctl")
        .args([
            "--user",
            "introspect",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("GlobalShortcuts"))
        .unwrap_or(false)
}

/// Detect the current desktop compositor
fn detect_compositor() -> String {
    env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "Unknown".into())
}

/// Register app_id with the xdg-desktop-portal Registry
/// This is required for native (non-Flatpak) apps to use portal features like GlobalShortcuts.
/// Without this, the portal uses cgroup-based detection which fails when running from terminal.
pub async fn register_app_with_portal() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::collections::HashMap;
    use zbus::Connection;

    println!("Registering app_id 'ink.whis.Whis' with portal...");

    let connection = Connection::session().await?;

    // Call org.freedesktop.host.portal.Registry.Register
    let result: Result<(), zbus::Error> = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.host.portal.Registry"),
            "Register",
            &("ink.whis.Whis", HashMap::<String, zbus::zvariant::Value>::new()),
        )
        .await
        .map(|_: zbus::Message| ());

    match result {
        Ok(_) => {
            println!("Successfully registered app_id with portal");
            Ok(())
        }
        Err(e) => {
            // Registry might not be available (older portals), continue anyway
            println!("Portal Registry registration failed (may be unavailable): {e}");
            // Don't return error - this is optional for newer portals
            Ok(())
        }
    }
}

/// Read the actual portal shortcut from dconf (GNOME)
/// Returns the shortcut in format like "Ctrl+Alt+M" if found
pub fn read_portal_shortcut_from_dconf() -> Option<String> {
    // Run: dconf dump /org/gnome/settings-daemon/global-shortcuts/
    let output = std::process::Command::new("dconf")
        .args(["dump", "/org/gnome/settings-daemon/global-shortcuts/"])
        .output()
        .ok()?;

    let dump = String::from_utf8_lossy(&output.stdout);

    // Look for toggle-recording in any app section
    // Format: shortcuts=[('toggle-recording', {'shortcuts': <['<Control><Alt>m']>, ...})]
    for line in dump.lines() {
        if line.contains("toggle-recording") && line.contains("shortcuts") {
            // Parse the GVariant format: <['<Control><Alt>m']>
            if let Some(start) = line.find("<['") {
                if let Some(end) = line[start..].find("']>") {
                    let raw = &line[start + 3..start + end];
                    // Convert <Control><Alt>m to Ctrl+Alt+M
                    return Some(convert_gvariant_shortcut(raw));
                }
            }
        }
    }
    None
}

/// Convert GVariant shortcut format to human-readable format
/// e.g., "<Control><Alt>m" -> "Ctrl+Alt+M"
fn convert_gvariant_shortcut(raw: &str) -> String {
    let converted = raw
        .replace("<Control>", "Ctrl+")
        .replace("<Alt>", "Alt+")
        .replace("<Shift>", "Shift+")
        .replace("<Super>", "Super+");

    // Uppercase the final key and handle trailing +
    if let Some(last_plus) = converted.rfind('+') {
        let (modifiers, key) = converted.split_at(last_plus + 1);
        format!("{}{}", modifiers, key.to_uppercase())
    } else {
        converted.to_uppercase()
    }
}

/// Setup global shortcuts using the XDG Portal (for Wayland with GNOME 48+, KDE)
pub async fn setup_portal_shortcuts<F>(
    shortcut_str: String,
    on_toggle: F,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn() + Send + Sync + 'static,
{
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    // IMPORTANT: Register app_id with portal BEFORE making any portal calls
    // This is required for native apps to have a valid app_id for GlobalShortcuts
    if let Err(e) = register_app_with_portal().await {
        eprintln!("Warning: Portal registration failed: {e}");
    }

    // Try to read existing shortcut from dconf first (works even if portal bind fails)
    if let Some(existing) = read_portal_shortcut_from_dconf() {
        println!("Found existing portal shortcut in dconf: {existing}");
        let state = app_handle.state::<crate::state::AppState>();
        *state.portal_shortcut.lock().unwrap() = Some(existing);
    }

    let shortcuts = GlobalShortcuts::new().await?;
    let session = shortcuts.create_session().await?;

    // Check for existing shortcuts first
    if let Ok(list_request) = shortcuts.list_shortcuts(&session).await {
        if let Ok(list_response) = list_request.response() {
            let existing = list_response.shortcuts();
            if let Some(s) = existing.iter().find(|s| s.id() == "toggle-recording") {
                let trigger = s.trigger_description().to_string();
                println!("Found existing portal shortcut in session: {trigger}");
                let state = app_handle.state::<crate::state::AppState>();
                *state.portal_shortcut.lock().unwrap() = Some(trigger);
                // Skip binding, just listen for activations
                let mut activated = shortcuts.receive_activated().await?;
                while let Some(event) = activated.next().await {
                    if event.shortcut_id() == "toggle-recording" {
                        println!("Portal shortcut triggered!");
                        on_toggle();
                    }
                }
                return Ok(());
            }
        }
    }

    // Define the toggle-recording shortcut
    let shortcut = NewShortcut::new("toggle-recording", "Toggle voice recording")
        .preferred_trigger(Some(shortcut_str.as_str()));

    // Try to bind - pass None for parent window (GNOME may show dialog to user)
    // Note: GNOME shows a configuration dialog that user must interact with
    match shortcuts.bind_shortcuts(&session, &[shortcut], None).await {
        Ok(request) => {
            match request.response() {
                Ok(bind_response) => {
                    if let Some(bound) = bind_response
                        .shortcuts()
                        .iter()
                        .find(|s| s.id() == "toggle-recording")
                    {
                        let trigger = bound.trigger_description().to_string();
                        if !trigger.is_empty() {
                            println!("Portal bound shortcut: {trigger}");
                            let state = app_handle.state::<crate::state::AppState>();
                            *state.portal_shortcut.lock().unwrap() = Some(trigger);
                        }
                    }
                    println!("Portal shortcuts registered. Listening for activations...");
                }
                Err(e) => {
                    eprintln!("Portal bind response failed: {e}");
                    eprintln!("Will use dconf shortcut if available");
                }
            }
        }
        Err(e) => {
            eprintln!("Portal bind_shortcuts failed: {e}");
            eprintln!("Will use dconf shortcut if available");
        }
    }

    // Listen for activations (this should still work even if bind failed)
    let mut activated = shortcuts.receive_activated().await?;
    while let Some(event) = activated.next().await {
        if event.shortcut_id() == "toggle-recording" {
            println!("Portal shortcut triggered!");
            on_toggle();
        }
    }

    Ok(())
}

/// Open the system's shortcut configuration dialog (Portal v2+ only)
/// Falls back to direct binding on Portal v1
pub async fn open_configure_shortcuts(
    app_handle: AppHandle,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    bind_shortcut_with_trigger(None, app_handle).await
}

/// Bind a shortcut with an optional preferred trigger from in-app key capture
/// Works on Portal v1 and v2. On v2, also opens the configuration dialog.
/// Returns the actual binding after success.
pub async fn bind_shortcut_with_trigger(
    preferred_trigger: Option<&str>,
    app_handle: AppHandle,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};

    // IMPORTANT: Register app_id with portal BEFORE making any portal calls
    // This is required for native apps to have a valid app_id for GlobalShortcuts
    if let Err(e) = register_app_with_portal().await {
        eprintln!("Warning: Portal registration failed: {e}");
    }

    let version = portal_version();
    println!("Portal GlobalShortcuts version: {version}");

    let shortcuts = GlobalShortcuts::new().await?;
    let session = shortcuts.create_session().await?;

    // Check for existing shortcuts first (XDG spec: can only bind once per session)
    if let Ok(list_request) = shortcuts.list_shortcuts(&session).await {
        if let Ok(list_response) = list_request.response() {
            let existing = list_response.shortcuts();
            if !existing.is_empty() {
                println!("Found {} existing shortcut(s) in session", existing.len());
                if let Some(s) = existing.iter().find(|s| s.id() == "toggle-recording") {
                    let trigger = s.trigger_description().to_string();
                    println!("Using existing shortcut: {trigger}");
                    let state = app_handle.state::<crate::state::AppState>();
                    *state.portal_shortcut.lock().unwrap() = Some(trigger.clone());
                    return Ok(Some(trigger));
                }
            }
        }
    }

    // Bind our shortcut ID with optional preferred trigger
    let mut shortcut = NewShortcut::new("toggle-recording", "Toggle voice recording");
    if let Some(trigger) = preferred_trigger {
        // Convert from Tauri format (Ctrl+Shift+R) to XDG format (<Control><Shift>r)
        let xdg_trigger = convert_to_xdg_format(trigger);
        println!("Requesting preferred trigger: {trigger} (XDG: {xdg_trigger})");
        shortcut = shortcut.preferred_trigger(Some(xdg_trigger.as_str()));
    }

    println!("Binding shortcut...");

    // Try to bind the shortcut (GNOME shows a dialog that user must interact with)
    let bind_result = shortcuts.bind_shortcuts(&session, &[shortcut], None).await;

    match bind_result {
        Ok(request) => {
            match request.response() {
                Ok(bind_response) => {
                    // Get the actual trigger that was bound
                    let trigger = bind_response
                        .shortcuts()
                        .iter()
                        .find(|s| s.id() == "toggle-recording")
                        .map(|s| s.trigger_description().to_string());

                    // On Portal v2+, also open configure dialog for confirmation
                    if version >= 2 {
                        println!("Portal v2: Opening configure dialog...");
                        let _ = shortcuts.configure_shortcuts(&session, None, None).await;

                        // Re-query after configure in case user changed it
                        if let Ok(list_request) = shortcuts.list_shortcuts(&session).await {
                            if let Ok(list_response) = list_request.response() {
                                let updated_trigger = list_response
                                    .shortcuts()
                                    .iter()
                                    .find(|s| s.id() == "toggle-recording")
                                    .map(|s| s.trigger_description().to_string());

                                if let Some(ref t) = updated_trigger {
                                    let state = app_handle.state::<crate::state::AppState>();
                                    *state.portal_shortcut.lock().unwrap() = Some(t.clone());
                                    println!("Portal shortcut configured to: {t}");
                                    return Ok(updated_trigger);
                                }
                            }
                        }
                    }

                    // Update AppState with bound trigger
                    if let Some(ref t) = trigger {
                        let state = app_handle.state::<crate::state::AppState>();
                        *state.portal_shortcut.lock().unwrap() = Some(t.clone());
                        println!("Portal shortcut bound to: {t}");
                    }

                    Ok(trigger)
                }
                Err(e) => {
                    Err(format!("Portal bind failed: {e}. The shortcut may conflict with an existing binding.").into())
                }
            }
        }
        Err(e) => {
            Err(format!("Portal request failed: {e}").into())
        }
    }
}

/// Legacy alias for configure_with_preferred_trigger
pub async fn configure_with_preferred_trigger(
    preferred_trigger: Option<&str>,
    app_handle: AppHandle,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    bind_shortcut_with_trigger(preferred_trigger, app_handle).await
}

/// Convert Tauri/human-readable shortcut format to XDG portal format
/// e.g., "Ctrl+Shift+R" -> "<Control><Shift>r"
fn convert_to_xdg_format(shortcut: &str) -> String {
    let parts: Vec<&str> = shortcut.split('+').collect();
    let mut result = String::new();

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => result.push_str("<Control>"),
            "shift" => result.push_str("<Shift>"),
            "alt" => result.push_str("<Alt>"),
            "super" | "meta" | "win" => result.push_str("<Super>"),
            key => result.push_str(&key.to_lowercase()),
        }
    }

    result
}

/// Setup global shortcuts using Tauri plugin (for X11, macOS, Windows)
pub fn setup_tauri_shortcut(app: &tauri::App, shortcut_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();
    
    // Attempt to parse the shortcut
    let shortcut = Shortcut::from_str(shortcut_str).map_err(|e| format!("Invalid shortcut: {e}"))?;

    // Initialize plugin with generic handler
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |_app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    println!("Tauri shortcut triggered!");
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::tray::toggle_recording_public(handle);
                    });
                }
            })
            .build(),
    )?;

    // Register the shortcut
    app.global_shortcut().register(shortcut)?;
    println!("Tauri global shortcut registered: {shortcut_str}");

    Ok(())
}

/// Setup shortcuts based on detected backend
pub fn setup_shortcuts(app: &tauri::App) {
    let capability = detect_backend();
    let state = app.state::<crate::state::AppState>();
    let settings = state.settings.lock().unwrap();
    let shortcut_str = settings.shortcut.clone();
    drop(settings);

    println!(
        "Detected environment: {} (backend: {:?})",
        capability.compositor, capability.backend
    );

    match capability.backend {
        ShortcutBackend::TauriPlugin => {
            if let Err(e) = setup_tauri_shortcut(app, &shortcut_str) {
                eprintln!("Failed to setup Tauri shortcut: {e}");
                eprintln!("Falling back to manual setup mode");
                print_manual_setup_instructions(&capability.compositor, &shortcut_str);
            }
        }
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
                            crate::tray::toggle_recording_public(handle);
                        });
                    },
                    app_handle_for_state,
                )
                .await
                {
                    eprintln!("Portal shortcuts failed: {e}");
                    eprintln!("Falling back to CLI mode");
                }
            });
        }
        ShortcutBackend::ManualSetup => {
            print_manual_setup_instructions(&capability.compositor, &shortcut_str);
        }
    }
}

/// Update shortcut. Returns Ok(true) if restart is needed, Ok(false) if applied immediately.
pub fn update_shortcut(app: &AppHandle, new_shortcut: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let capability = detect_backend();

    match capability.backend {
        ShortcutBackend::TauriPlugin => {
            // Unregister all existing shortcuts
            app.global_shortcut().unregister_all()?;

            // Parse and register new one
            let shortcut = Shortcut::from_str(new_shortcut).map_err(|e| format!("Invalid shortcut: {e}"))?;
            app.global_shortcut().register(shortcut)?;
            println!("Updated Tauri global shortcut to: {new_shortcut}");
            Ok(false) // No restart needed
        },
        _ => {
            // For portals and CLI, dynamic updates require restart.
            println!("Shortcut saved. Restart required for changes to take effect.");
            Ok(true) // Restart needed
        }
    }
}


fn print_manual_setup_instructions(compositor: &str, shortcut: &str) {
    println!();
    println!("=== Global Shortcuts Not Available ===");
    println!("Compositor: {compositor}");
    println!();
    println!("To use a keyboard shortcut, configure your compositor:");
    println!();
    match compositor.to_lowercase().as_str() {
        s if s.contains("gnome") => {
            println!("GNOME: Settings → Keyboard → Custom Shortcuts");
            println!("  Name: Whis Toggle Recording");
            println!("  Command: whis-desktop --toggle");
            println!("  Shortcut: {shortcut}");
        }
        s if s.contains("kde") || s.contains("plasma") => {
            println!("KDE: System Settings → Shortcuts → Custom Shortcuts");
            println!("  Command: whis-desktop --toggle");
        }
        s if s.contains("sway") => {
            println!("Sway: Add to ~/.config/sway/config:");
            println!("  bindsym {} exec whis-desktop --toggle", shortcut.to_lowercase());
        }
        s if s.contains("hyprland") => {
            println!("Hyprland: Add to ~/.config/hypr/hyprland.conf:");
            println!("  bind = {}, exec, whis-desktop --toggle", shortcut.replace("+", ", "));
        }
        _ => {
            println!("Configure your compositor to run: whis-desktop --toggle");
        }
    }
    println!();
}

/// Send toggle command to running instance via Unix socket
pub fn send_toggle_command() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let socket_path = socket_path();

    match UnixStream::connect(&socket_path) {
        Ok(mut stream) => {
            stream.write_all(b"toggle")?;
            println!("Toggle command sent");
            Ok(())
        }
        Err(e) => {
            eprintln!("Could not connect to running instance: {e}");
            eprintln!("Is whis-desktop running?");
            Err(e.into())
        }
    }
}

/// Start listening for IPC commands
pub fn start_ipc_listener(app_handle: AppHandle) {
    let socket_path = socket_path();

    // Remove old socket if exists
    let _ = std::fs::remove_file(&socket_path);

    std::thread::spawn(move || {
        use std::io::Read;
        use std::os::unix::net::UnixListener;

        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to create IPC socket: {e}");
                return;
            }
        };

        println!("IPC listener started at {socket_path}");

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 64];
                    if let Ok(n) = stream.read(&mut buf) {
                        let cmd = String::from_utf8_lossy(&buf[..n]);
                        if cmd.trim() == "toggle" {
                            println!("IPC: toggle command received");
                            let handle = app_handle.clone();
                            // Dispatch to Tauri's async runtime - the IPC thread has no Tokio runtime
                            tauri::async_runtime::spawn(async move {
                                crate::tray::toggle_recording_public(handle);
                            });
                        }
                    }
                }
                Err(e) => eprintln!("IPC connection error: {e}"),
            }
        }
    });
}

fn socket_path() -> String {
    let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    format!("{runtime_dir}/whis-desktop.sock")
}