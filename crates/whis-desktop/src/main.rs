#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::IsTerminal;

fn main() {
    // Set app_id for Wayland - must be done BEFORE GTK init
    // This is required for GNOME GlobalShortcuts portal to accept our requests
    #[cfg(target_os = "linux")]
    {
        // Set the program name which GTK uses as app_id on Wayland
        // Must match the .desktop file name (without extension)
        gtk::glib::set_prgname(Some("ink.whis.Whis"));
        gtk::glib::set_application_name("Whis");
    }

    let args: Vec<String> = std::env::args().collect();

    // Handle --toggle command: send toggle to running instance and exit
    if args.contains(&"--toggle".to_string()) || args.contains(&"-t".to_string()) {
        if let Err(e) = whis_desktop::shortcuts::send_toggle_command() {
            eprintln!("Failed to toggle: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Handle --install: create .desktop file for proper app_id on Wayland
    if args.contains(&"--install".to_string()) {
        install_desktop_file();
        return;
    }

    // Handle --uninstall: remove .desktop file and icons
    if args.contains(&"--uninstall".to_string()) {
        uninstall_desktop_file();
        return;
    }

    // Handle --help
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        println!("whis-desktop - Voice to text desktop application");
        println!();
        println!("USAGE:");
        println!("    whis-desktop [OPTIONS]");
        println!();
        println!("OPTIONS:");
        println!("    -t, --toggle    Toggle recording in running instance");
        println!("        --install   Install desktop file and icons for app menu");
        println!("        --uninstall Remove desktop file and icons");
        println!("    -h, --help      Print this help message");
        println!();
        println!("GLOBAL SHORTCUT:");
        println!("    Ctrl+Shift+R    Toggle recording (X11/Portal only)");
        println!();
        println!("For Wayland without portal support, configure your compositor");
        println!("to run 'whis-desktop --toggle' on your preferred shortcut.");
        return;
    }

    // Warn if launched from terminal as AppImage (shortcuts won't work on Wayland)
    if is_appimage_from_terminal() {
        let appimage = std::env::var("APPIMAGE").unwrap();
        eprintln!("⚠️  For global shortcuts to work on Wayland, install Whis as an app:");
        eprintln!("   {appimage} --install");
        eprintln!("   Then launch from your app menu.\n");
    }

    // Start the GUI application
    whis_desktop::run();
}

/// Check if we're running as an AppImage launched from a terminal
fn is_appimage_from_terminal() -> bool {
    std::env::var("APPIMAGE").is_ok() && std::io::stderr().is_terminal()
}

/// Install a .desktop file for proper app menu integration
fn install_desktop_file() {
    let exec_path = std::env::var("APPIMAGE")
        .unwrap_or_else(|_| "whis-desktop".to_string());

    let desktop_content = format!(
        r#"[Desktop Entry]
Name=Whis
Comment=Voice-to-text transcription with global shortcuts
Exec={exec_path}
Icon=ink.whis.Whis
Terminal=false
Type=Application
Categories=Utility;Audio;
Keywords=voice;transcription;whisper;speech;
StartupWMClass=ink.whis.Whis
StartupNotify=true
"#
    );

    let data_dir = get_data_dir();

    // Install desktop file
    let desktop_dir = data_dir.join("applications");
    if let Err(e) = std::fs::create_dir_all(&desktop_dir) {
        eprintln!("Failed to create directory: {e}");
        std::process::exit(1);
    }

    let desktop_path = desktop_dir.join("ink.whis.Whis.desktop");
    if let Err(e) = std::fs::write(&desktop_path, desktop_content) {
        eprintln!("Failed to write desktop file: {e}");
        std::process::exit(1);
    }

    // Update desktop database so GNOME discovers the new app
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&desktop_dir)
        .output();

    println!("✓ Installed: {}", desktop_path.display());

    // Install icons
    if let Err(e) = install_icons(&data_dir) {
        eprintln!("⚠ Failed to install icons: {e}");
    } else {
        println!("✓ Installed icons to ~/.local/share/icons/hicolor/");
    }

    println!("\nLaunch Whis from your app menu for global shortcuts to work.");
    println!("(You may need to log out and back in for the icon to appear)");
}

/// Install icons to XDG icon directories
fn install_icons(data_dir: &std::path::Path) -> std::io::Result<()> {
    let icon_dir = data_dir.join("icons/hicolor");

    // Icon sizes with embedded data
    let icons: &[(&str, &[u8])] = &[
        ("32x32", include_bytes!("../icons/32x32.png")),
        ("48x48", include_bytes!("../icons/48x48.png")),
        ("64x64", include_bytes!("../icons/64x64.png")),
        ("128x128", include_bytes!("../icons/128x128.png")),
        ("256x256", include_bytes!("../icons/256x256.png")),
        ("512x512", include_bytes!("../icons/512x512.png")),
    ];

    for (size, data) in icons {
        let size_dir = icon_dir.join(size).join("apps");
        std::fs::create_dir_all(&size_dir)?;
        std::fs::write(size_dir.join("ink.whis.Whis.png"), data)?;
    }

    // Install scalable SVG
    let scalable_dir = icon_dir.join("scalable/apps");
    std::fs::create_dir_all(&scalable_dir)?;
    std::fs::write(
        scalable_dir.join("ink.whis.Whis.svg"),
        include_bytes!("../icons/icon.svg"),
    )?;

    // Update icon cache so GNOME can find the icons
    // -f = force, -t = ignore missing index.theme (needed for user dirs)
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["-f", "-t"])
        .arg(&icon_dir)
        .output();

    Ok(())
}

/// Uninstall desktop file and icons
fn uninstall_desktop_file() {
    let data_dir = get_data_dir();

    // Remove desktop file
    let desktop_path = data_dir.join("applications/ink.whis.Whis.desktop");
    if desktop_path.exists() {
        if let Err(e) = std::fs::remove_file(&desktop_path) {
            eprintln!("Failed to remove desktop file: {e}");
        } else {
            println!("✓ Removed: {}", desktop_path.display());
        }
    } else {
        println!("Desktop file not found (already uninstalled?)");
    }

    // Remove icons
    let icon_dir = data_dir.join("icons/hicolor");
    let mut removed = 0;
    for size in ["32x32", "48x48", "64x64", "128x128", "256x256", "512x512"] {
        let icon_path = icon_dir.join(size).join("apps/ink.whis.Whis.png");
        if std::fs::remove_file(&icon_path).is_ok() {
            removed += 1;
        }
    }
    if std::fs::remove_file(icon_dir.join("scalable/apps/ink.whis.Whis.svg")).is_ok() {
        removed += 1;
    }

    if removed > 0 {
        println!("✓ Removed {removed} icon(s)");
    }
}

/// Get XDG_DATA_HOME or default to ~/.local/share
fn get_data_dir() -> std::path::PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            std::path::PathBuf::from(home).join(".local/share")
        })
}
