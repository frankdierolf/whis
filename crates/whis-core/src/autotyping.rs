//! Cross-platform autotyping via virtual keyboard.
//!
//! This module provides the ability to type text directly into the active window
//! by simulating keyboard input. Multiple backends are supported for different
//! platforms and display servers.
//!
//! # Backends
//!
//! - **Tools** - Linux: shell-out to wtype/dotool/ydotool (Wayland) or xdotool/ydotool (X11)
//! - **Enigo** - macOS, Windows (cross-platform input simulation)
//!
//! # Platform Support
//!
//! | Platform | Backend |
//! |----------|---------|
//! | Wayland  | wtype → dotool → ydotool (fallback chain) |
//! | X11      | xdotool → ydotool (fallback chain) |
//! | macOS    | enigo |
//! | Windows  | enigo |
//!
//! # Permissions
//!
//! - **macOS**: Requires Accessibility permission in System Preferences
//! - **Windows**: Cannot type into elevated (admin) windows
//! - **Linux**: External tools must be installed (wtype, xdotool, etc.)
//!
//! # Usage
//!
//! ```ignore
//! use whis_core::autotyping::{autotype_text, AutotypeBackend};
//!
//! // Auto-detect best backend for current platform
//! autotype_text("Hello, world!", AutotypeBackend::Auto, None)?;
//!
//! // With inter-key delay for slow applications
//! autotype_text("Hello", AutotypeBackend::Auto, Some(10))?;
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::platform::{Platform, detect_platform};

/// Status of autotyping tool availability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutotypeToolStatus {
    /// Tools that are installed and available
    pub available: Vec<String>,
    /// Recommended tool for the current environment
    pub recommended: Option<String>,
    /// Installation hint for the recommended tool
    pub install_hint: Option<String>,
}

/// Check which autotype tools are available on the system
///
/// Returns status information including:
/// - Which tools are installed
/// - Which tool is recommended for the current platform/compositor
/// - Installation instructions if no tools are available
pub fn get_autotype_tool_status() -> AutotypeToolStatus {
    #[cfg(target_os = "linux")]
    {
        let platform_info = detect_platform();
        let mut available = Vec::new();

        // Check which tools are available
        if which_exists("ydotool") {
            available.push("ydotool".to_string());
        }
        if which_exists("wtype") {
            available.push("wtype".to_string());
        }
        if which_exists("dotool") {
            available.push("dotool".to_string());
        }
        if which_exists("xdotool") {
            available.push("xdotool".to_string());
        }

        // Determine recommended tool and install hint based on platform
        let (recommended, install_hint) = match platform_info.platform {
            Platform::LinuxWayland => {
                // On Wayland, ydotool works everywhere (including GNOME/Mutter)
                // wtype only works on wlroots compositors
                let is_wlroots = matches!(
                    platform_info.compositor,
                    crate::platform::Compositor::Sway
                        | crate::platform::Compositor::Hyprland
                        | crate::platform::Compositor::Wlroots
                );

                if is_wlroots && available.contains(&"wtype".to_string()) {
                    // wtype is available and works on this compositor
                    (None, None)
                } else if available.contains(&"ydotool".to_string()) {
                    // ydotool is available
                    (None, None)
                } else {
                    // Need to recommend a tool
                    (
                        Some("ydotool".to_string()),
                        Some(
                            "sudo apt install ydotool && sudo systemctl enable --now ydotool"
                                .to_string(),
                        ),
                    )
                }
            }
            Platform::LinuxX11 => {
                if available.contains(&"xdotool".to_string())
                    || available.contains(&"ydotool".to_string())
                {
                    (None, None)
                } else {
                    (
                        Some("xdotool".to_string()),
                        Some("sudo apt install xdotool".to_string()),
                    )
                }
            }
            Platform::MacOS | Platform::Windows => {
                // enigo handles these platforms, no external tools needed
                (None, None)
            }
        };

        AutotypeToolStatus {
            available,
            recommended,
            install_hint,
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        // On macOS/Windows, we use enigo which doesn't need external tools
        AutotypeToolStatus {
            available: vec![],
            recommended: None,
            install_hint: None,
        }
    }
}

/// Backend for autotyping text into the active window
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AutotypeBackend {
    /// Auto-detect based on platform:
    /// - Linux → Tools (wtype/dotool/ydotool or xdotool/ydotool)
    /// - macOS/Windows → Enigo
    #[default]
    Auto,
    /// Force external CLI tools (Linux only)
    Tools,
    /// Force enigo (macOS, Windows, X11 fallback)
    Enigo,
}

/// How to output transcribed text
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutputMethod {
    /// Copy to clipboard only (default, current behavior)
    #[default]
    Clipboard,
    /// Type directly into active window
    Autotype,
    /// Both clipboard and autotype to window
    Both,
}

impl std::fmt::Display for OutputMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputMethod::Clipboard => write!(f, "clipboard"),
            OutputMethod::Autotype => write!(f, "autotype to window"),
            OutputMethod::Both => write!(f, "clipboard + autotype to window"),
        }
    }
}

/// Type text into the active window using the specified backend
///
/// # Arguments
///
/// * `text` - The text to type
/// * `backend` - Which autotyping backend to use
/// * `delay_ms` - Optional delay between keystrokes (milliseconds)
///
/// # Errors
///
/// Returns an error if the autotyping backend fails or is not available
/// for the current platform.
pub fn autotype_text(text: &str, backend: AutotypeBackend, delay_ms: Option<u32>) -> Result<()> {
    crate::verbose!("Autotyping {} chars to active window", text.len());
    crate::verbose!("Backend: {:?}, Delay: {:?}ms", backend, delay_ms);

    match backend {
        AutotypeBackend::Auto => autotype_auto(text, delay_ms),
        AutotypeBackend::Tools => autotype_via_tools(text, delay_ms),
        AutotypeBackend::Enigo => autotype_via_enigo(text, delay_ms),
    }
}

/// Auto-detect the best autotyping backend for the current platform
fn autotype_auto(text: &str, delay_ms: Option<u32>) -> Result<()> {
    let platform_info = detect_platform();
    crate::verbose!(
        "Auto-detecting autotyping backend for {:?}",
        platform_info.platform
    );

    match platform_info.platform {
        Platform::LinuxWayland | Platform::LinuxX11 => {
            crate::verbose!("Using external tools for {:?}", platform_info.platform);
            autotype_via_tools(text, delay_ms)
        }
        Platform::MacOS | Platform::Windows => {
            crate::verbose!("Using enigo for {:?}", platform_info.platform);
            autotype_via_enigo(text, delay_ms)
        }
    }
}

/// Type text using external CLI tools (Linux)
///
/// On Wayland: tries wtype → dotool → ydotool
/// On X11: tries xdotool → ydotool
#[cfg(target_os = "linux")]
fn autotype_via_tools(text: &str, delay_ms: Option<u32>) -> Result<()> {
    let platform_info = detect_platform();

    match platform_info.platform {
        Platform::LinuxWayland => autotype_via_wayland_tools(text, delay_ms),
        Platform::LinuxX11 => autotype_via_x11_tools(text, delay_ms),
        _ => anyhow::bail!("Tools backend only available on Linux"),
    }
}

#[cfg(not(target_os = "linux"))]
fn autotype_via_tools(_text: &str, _delay_ms: Option<u32>) -> Result<()> {
    anyhow::bail!("Tools backend is only available on Linux")
}

/// Try Wayland tools in order: wtype → dotool → ydotool
#[cfg(target_os = "linux")]
fn autotype_via_wayland_tools(text: &str, delay_ms: Option<u32>) -> Result<()> {
    use std::process::Command;

    let mut errors: Vec<String> = Vec::new();

    // Try wtype first (supports delay, but only works on wlroots compositors)
    if which_exists("wtype") {
        crate::verbose!("Using wtype for Wayland autotyping");
        let mut cmd = Command::new("wtype");

        if let Some(delay) = delay_ms {
            cmd.arg("-d").arg(delay.to_string());
        }

        // wtype reads from argument
        cmd.arg("--").arg(text);

        let output = cmd.output().context("Failed to execute wtype")?;

        if output.status.success() {
            crate::verbose!("wtype succeeded");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        crate::verbose!("wtype failed: {}", stderr);
        errors.push(format!(
            "wtype: {}",
            if stderr.is_empty() {
                "failed with no output".to_string()
            } else {
                stderr
            }
        ));
    }

    // Try dotool second
    if which_exists("dotool") {
        crate::verbose!("Using dotool for Wayland autotyping");

        // dotool reads from stdin
        let mut cmd = Command::new("dotool");
        cmd.stdin(std::process::Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn dotool")?;

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            // dotool command format: type <text>
            writeln!(stdin, "type {}", text).context("Failed to write to dotool stdin")?;
        }

        let output = child
            .wait_with_output()
            .context("Failed to wait for dotool")?;

        if output.status.success() {
            crate::verbose!("dotool succeeded");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        crate::verbose!("dotool failed: {}", stderr);
        errors.push(format!(
            "dotool: {}",
            if stderr.is_empty() {
                "failed with no output".to_string()
            } else {
                stderr
            }
        ));
    }

    // Try ydotool last (works on all compositors via uinput, but requires daemon)
    if which_exists("ydotool") {
        crate::verbose!("Using ydotool for Wayland autotyping");
        let mut cmd = Command::new("ydotool");
        cmd.arg("type");

        if let Some(delay) = delay_ms {
            // ydotool uses microseconds for --key-delay
            cmd.arg("--key-delay").arg((delay * 1000).to_string());
        }

        cmd.arg("--").arg(text);

        let output = cmd.output().context("Failed to execute ydotool")?;

        if output.status.success() {
            crate::verbose!("ydotool succeeded");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        crate::verbose!("ydotool failed: {}", stderr);
        errors.push(format!(
            "ydotool: {}",
            if stderr.is_empty() {
                "failed with no output".to_string()
            } else {
                stderr
            }
        ));
    }

    // Provide helpful error message based on what we found
    if errors.is_empty() {
        anyhow::bail!(
            "No Wayland autotyping tool found. Install one of:\n\
             - ydotool (recommended, works on all compositors including GNOME)\n\
             - wtype (wlroots-only: Sway, Hyprland)\n\
             - dotool\n\n\
             For ydotool: sudo apt install ydotool && sudo systemctl enable --now ydotool"
        )
    } else {
        anyhow::bail!(
            "Autotyping failed. Tools tried:\n  {}\n\n\
             If using GNOME/Mutter, install ydotool (wtype only works on wlroots compositors):\n\
             sudo apt install ydotool && sudo systemctl enable --now ydotool",
            errors.join("\n  ")
        )
    }
}

/// Try X11 tools in order: xdotool → ydotool
#[cfg(target_os = "linux")]
fn autotype_via_x11_tools(text: &str, delay_ms: Option<u32>) -> Result<()> {
    use std::process::Command;

    let mut errors: Vec<String> = Vec::new();

    // Try xdotool first
    if which_exists("xdotool") {
        crate::verbose!("Using xdotool for X11 autotyping");
        let mut cmd = Command::new("xdotool");
        cmd.arg("type");

        if let Some(delay) = delay_ms {
            cmd.arg("--delay").arg(delay.to_string());
        }

        cmd.arg("--").arg(text);

        let output = cmd.output().context("Failed to execute xdotool")?;

        if output.status.success() {
            crate::verbose!("xdotool succeeded");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        crate::verbose!("xdotool failed: {}", stderr);
        errors.push(format!(
            "xdotool: {}",
            if stderr.is_empty() {
                "failed with no output".to_string()
            } else {
                stderr
            }
        ));
    }

    // Try ydotool (works on X11 too via uinput)
    if which_exists("ydotool") {
        crate::verbose!("Using ydotool for X11 autotyping");
        let mut cmd = Command::new("ydotool");
        cmd.arg("type");

        if let Some(delay) = delay_ms {
            cmd.arg("--key-delay").arg((delay * 1000).to_string());
        }

        cmd.arg("--").arg(text);

        let output = cmd.output().context("Failed to execute ydotool")?;

        if output.status.success() {
            crate::verbose!("ydotool succeeded");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        crate::verbose!("ydotool failed: {}", stderr);
        errors.push(format!(
            "ydotool: {}",
            if stderr.is_empty() {
                "failed with no output".to_string()
            } else {
                stderr
            }
        ));
    }

    // Provide helpful error message based on what we found
    if errors.is_empty() {
        anyhow::bail!(
            "No X11 autotyping tool found. Install one of:\n\
             - xdotool (recommended for X11)\n\
             - ydotool (works everywhere)\n\n\
             For xdotool: sudo apt install xdotool"
        )
    } else {
        anyhow::bail!(
            "Autotyping failed. Tools tried:\n  {}\n\n\
             For ydotool, ensure the daemon is running:\n\
             sudo systemctl enable --now ydotool",
            errors.join("\n  ")
        )
    }
}

/// Check if a command exists in PATH
#[cfg(target_os = "linux")]
fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Type text using enigo (cross-platform input simulation)
///
/// - X11: Uses XTest extension
/// - macOS: Uses CoreGraphics/CGEvent (requires Accessibility permission)
/// - Windows: Uses SendInput API (cannot type into elevated windows)
fn autotype_via_enigo(text: &str, delay_ms: Option<u32>) -> Result<()> {
    use enigo::{Enigo, Keyboard, Settings};

    crate::verbose!("Initializing enigo");

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Failed to initialize enigo: {}", e))?;

    if let Some(delay) = delay_ms {
        crate::verbose!("Typing with {}ms delay between characters", delay);
        // Type character by character with delay
        for c in text.chars() {
            enigo
                .text(&c.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to type character '{}': {}", c, e))?;
            std::thread::sleep(std::time::Duration::from_millis(delay as u64));
        }
    } else {
        enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("Failed to type text: {}", e))?;
    }

    crate::verbose!("enigo succeeded");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autotype_backend_default() {
        assert_eq!(AutotypeBackend::default(), AutotypeBackend::Auto);
    }

    #[test]
    fn test_output_method_default() {
        assert_eq!(OutputMethod::default(), OutputMethod::Clipboard);
    }

    #[test]
    fn test_autotype_backend_serde() {
        let backend: AutotypeBackend = serde_json::from_str(r#""auto""#).unwrap();
        assert_eq!(backend, AutotypeBackend::Auto);

        let backend: AutotypeBackend = serde_json::from_str(r#""tools""#).unwrap();
        assert_eq!(backend, AutotypeBackend::Tools);

        let backend: AutotypeBackend = serde_json::from_str(r#""enigo""#).unwrap();
        assert_eq!(backend, AutotypeBackend::Enigo);
    }

    #[test]
    fn test_output_method_serde() {
        let method: OutputMethod = serde_json::from_str(r#""clipboard""#).unwrap();
        assert_eq!(method, OutputMethod::Clipboard);

        let method: OutputMethod = serde_json::from_str(r#""autotype""#).unwrap();
        assert_eq!(method, OutputMethod::Autotype);

        let method: OutputMethod = serde_json::from_str(r#""both""#).unwrap();
        assert_eq!(method, OutputMethod::Both);
    }

    #[test]
    fn test_output_method_display() {
        assert_eq!(OutputMethod::Clipboard.to_string(), "clipboard");
        assert_eq!(OutputMethod::Autotype.to_string(), "autotype to window");
        assert_eq!(
            OutputMethod::Both.to_string(),
            "clipboard + autotype to window"
        );
    }
}
