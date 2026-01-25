<div align="center">
<img src="https://raw.githubusercontent.com/frankdierolf/whis/main/crates/whis-desktop/icons/128x128.png" alt="whis" width="80" height="80" />

<h3>whis</h3>
<p>
  Your voice, piped to clipboard.
  <br />
  <a href="https://whis.ink">Website</a>
  ·
  <a href="https://github.com/frankdierolf/whis/tree/main/crates/whis-desktop">Desktop</a>
  ·
  <a href="https://github.com/frankdierolf/whis/tree/main/crates/whis-mobile">Mobile</a>
  ·
  <a href="https://github.com/frankdierolf/whis/releases">Releases</a>
</p>
</div>

## Introduction

The terminal-native voice-to-text tool. Record, transcribe, paste — all from your shell. Supports hotkey mode, presets, and pipes nicely with AI assistants.

## Quick Start

```bash
cargo install whis-cli
whis setup         # Interactive wizard
whis
```

## Usage

```bash
# Record once
whis                           # Press Enter to stop — text copied!

# Background service (hotkey mode)
whis start                     # Start service (ctrl+alt+w toggles recording)
whis stop                      # Stop background service
whis status                    # Check if running

# Transcribe from file
whis -f recording.wav          # Transcribe a WAV file

# Output options
whis --print                   # Print to stdout instead of clipboard
whis start --autotype          # Type into active window (hotkey mode)
whis -d 10                     # Record for 10 seconds (non-interactive)
whis -v                        # Verbose output

# Presets
whis --as email                # Use preset (auto-enables post-processing)
whis preset                    # List all
whis preset new                # Print template for new preset
whis preset edit xyz           # Edit preset in $EDITOR

# Post-process with LLM (presets define the transformation)
whis --post-process

# Configuration
whis config                    # Show current settings
whis config provider openai    # Set provider
whis config language en        # Set language hint
whis model                     # List available models
```

## Environment Variables

API keys can be set via environment variables instead of `whis setup`:

```bash
OPENAI_API_KEY=sk-...
MISTRAL_API_KEY=...
GROQ_API_KEY=gsk_...
DEEPGRAM_API_KEY=...
ELEVENLABS_API_KEY=...
OLLAMA_URL=http://localhost:11434   # Default
OLLAMA_MODEL=qwen2.5:1.5b           # Default post-processing model
```

## Requirements

- API key from [OpenAI](https://platform.openai.com/api-keys), Mistral, Groq, Deepgram, or ElevenLabs — or use local Whisper/Parakeet (no API key needed)
- Linux (X11/Wayland), macOS, or Windows

**For hotkey mode** (Linux):
```bash
# Option 1: Compositor keybinding (no permissions needed)
# GNOME: Settings > Keyboard > Custom Shortcuts → whis toggle
# Sway:  bindsym Ctrl+Alt+w exec whis toggle

# Option 2: Direct capture
sudo usermod -aG input $USER
# Logout and login, then: whis start
```

## Prefer a GUI?

See [whis-desktop](https://github.com/frankdierolf/whis/tree/main/crates/whis-desktop) — same functionality, with system tray.

## License

MIT
