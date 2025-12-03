use tauri::{WebviewWindowBuilder, WebviewUrl};

/// Show the main window when tray is not available
/// This provides a fallback UI for tray-less desktop environments
pub fn show_main_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::App("index.html".into())
    )
    .title("Whis")
    .inner_size(600.0, 400.0)
    .min_inner_size(400.0, 300.0)
    .resizable(true)
    .decorations(false)
    .transparent(true)
    .build()?;

    // Fix Wayland window dragging by unsetting GTK titlebar
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::GtkWindowExt;
        if let Ok(gtk_window) = window.gtk_window() {
            gtk_window.set_titlebar(Option::<&gtk::Widget>::None);
        }
    }

    window.show()?;

    Ok(())
}
