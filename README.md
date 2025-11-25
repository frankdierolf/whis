# Whispo

Minimal voice-to-text CLI for terminal users. Record your voice, get instant transcription to clipboard.

## Demo

![Whispo Demo](demo.gif)

## Quick Start

```bash
# Install
cargo install whispo

# Set API key (add to ~/.bashrc or ~/.zshrc)
export OPENAI_API_KEY=sk-your-key-here

# Run
whispo
```

## Usage

```bash
whispo
```

1. Recording starts automatically
2. Press Enter to stop
3. Transcription copies to clipboard

That's it. Paste into your AI coding tool.

## Hotkey Mode

For hands-free operation with a global hotkey:

```bash
# One-time setup (run these once, then logout/login)
sudo usermod -aG input $USER
echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' | sudo tee /etc/udev/rules.d/99-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger

# Start the service with built-in hotkey
whispo listen                        # Default: Ctrl+Shift+R
whispo listen --hotkey "ctrl+alt+r"  # Custom hotkey
whispo listen -k "super+r"           # Short form
```

Press your hotkey anywhere to toggle recording. Works on all Linux distros (X11 and Wayland).

Other commands:
```bash
whispo status          # Check service status
whispo stop            # Stop background service
```

## Requirements

- cargo (Rust package manager)
- OpenAI API key ([get one here](https://platform.openai.com/api-keys))
- FFmpeg (for audio compression)
- Linux with working microphone
- ALSA or PulseAudio
- `input` group + uinput access (for hotkey mode, see setup above)

### Installing FFmpeg

```bash
# Ubuntu/Debian
sudo apt install ffmpeg

# macOS
brew install ffmpeg
```

## Building from Source

```bash
cargo build --release
```

Binary will be at `./target/release/whispo`

## FAQ

**How does hotkey mode work?**

A lightweight background service listens for your hotkey via evdev (works on both X11 and Wayland). The `input` group and uinput access allow reading and re-emitting keyboard events without root.

**What hotkeys can I use?**

Combinations of modifiers (`ctrl`, `shift`, `alt`, `super`) and keys (`a-z`, `0-9`, `f1-f12`, `space`, `enter`, etc.). Examples: `ctrl+shift+r`, `super+space`, `alt+1`.

**Does the simple mode still work?**

Yes! Running `whispo` without arguments works exactly as before. Hotkey mode is completely optional.

## Inspiration

Inspired by [whisp](https://github.com/yummyweb/whisp) - a desktop voice input tool with system tray integration.

## License

MIT
