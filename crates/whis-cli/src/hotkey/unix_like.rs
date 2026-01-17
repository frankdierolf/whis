use anyhow::Result;
use std::sync::mpsc::Receiver;
use whis_core::hotkey::Hotkey;

use super::HotkeyEvent;

#[cfg(target_os = "linux")]
use rdev::grab;

#[cfg(target_os = "macos")]
use rdev::{Event, EventType, Key, listen};

#[cfg(target_os = "macos")]
use std::collections::HashSet;

#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
use whis_core::hotkey::lock_or_recover;

pub struct HotkeyGuard;

pub fn setup(hotkey_str: &str) -> Result<(Receiver<HotkeyEvent>, HotkeyGuard)> {
    let hotkey = Hotkey::parse(hotkey_str).map_err(|e| anyhow::anyhow!(e))?;
    let (tx, rx) = std::sync::mpsc::channel();
    let tx_release = tx.clone();

    std::thread::spawn(move || {
        if let Err(e) = listen_for_hotkey(
            hotkey,
            move || {
                let _ = tx.send(HotkeyEvent::Pressed);
            },
            move || {
                let _ = tx_release.send(HotkeyEvent::Released);
            },
        ) {
            eprintln!("Hotkey error: {e}");
        }
    });

    Ok((rx, HotkeyGuard))
}

/// Listen for a hotkey and call callbacks on press/release (push-to-talk mode)
/// This function blocks and runs until an error occurs
pub fn listen_for_hotkey<FPress, FRelease>(
    hotkey: Hotkey,
    on_press: FPress,
    on_release: FRelease,
) -> Result<()>
where
    FPress: Fn() + Send + 'static,
    FRelease: Fn() + Send + 'static,
{
    // Linux: Use shared grab callback from whis-core
    #[cfg(target_os = "linux")]
    {
        let callback = whis_core::hotkey::create_grab_callback(hotkey, on_press, on_release);

        if let Err(e) = grab(callback) {
            anyhow::bail!(
                "Failed to grab keyboard: {e:?}\n\nLinux setup required:\n  sudo usermod -aG input $USER\n  echo 'KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\"' | sudo tee /etc/udev/rules.d/99-uinput.rules\n  sudo udevadm control --reload-rules && sudo udevadm trigger\nThen logout and login again."
            );
        }
    }

    // macOS: Use listen (doesn't consume events, different API)
    #[cfg(target_os = "macos")]
    {
        let pressed_keys: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
        let pressed_keys_clone = pressed_keys.clone();
        let hotkey_triggered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let hotkey_triggered_clone = hotkey_triggered.clone();
        let main_key = hotkey.key;

        let callback = move |event: Event| match event.event_type {
            EventType::KeyPress(key) => {
                let mut keys = lock_or_recover(&pressed_keys_clone);
                keys.insert(key);

                let mut triggered = lock_or_recover(&hotkey_triggered_clone);
                if *triggered {
                    return;
                }

                if hotkey.is_pressed(&keys) {
                    *triggered = true;
                    on_press();
                }
            }
            EventType::KeyRelease(key) => {
                let mut keys = lock_or_recover(&pressed_keys_clone);
                keys.remove(&key);

                if key == main_key {
                    let mut triggered = lock_or_recover(&hotkey_triggered_clone);
                    if *triggered {
                        *triggered = false;
                        on_release();
                    }
                }
            }
            _ => {}
        };

        if let Err(e) = listen(callback) {
            anyhow::bail!(
                "Failed to listen for keyboard events: {e:?}\n\nmacOS setup required:\n  1. Open System Settings → Privacy & Security → Accessibility\n  2. Add your terminal app (e.g., Terminal.app, iTerm2, WezTerm)\n  3. Enable the checkbox next to it\n  4. Restart your terminal app completely (Cmd+Q, then reopen)\n  5. Run 'whis listen' again"
            );
        }
    }

    Ok(())
}
