#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Handle --toggle command: send toggle to running instance and exit
    if args.contains(&"--toggle".to_string()) || args.contains(&"-t".to_string()) {
        if let Err(e) = whis_desktop::shortcuts::send_toggle_command() {
            eprintln!("Failed to toggle: {e}");
            std::process::exit(1);
        }
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
        println!("    -h, --help      Print this help message");
        println!();
        println!("GLOBAL SHORTCUT:");
        println!("    Ctrl+Shift+R    Toggle recording (X11/Portal only)");
        println!();
        println!("For Wayland without portal support, configure your compositor");
        println!("to run 'whis-desktop --toggle' on your preferred shortcut.");
        return;
    }

    // Start the GUI application
    whis_desktop::run();
}
