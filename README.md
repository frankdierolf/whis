# Whispo

Voice-to-text for terminal users. Record your voice, get instant transcription to clipboard.

Built for Linux terminal workflows with Claude Code, Cursor, Gemini CLI, and other AI coding tools. Think whisperflow.ai, but minimal and CLI-native.

## Demo

![Whispo Demo](demo.gif)

## Why Whispo?

I wanted whisperflow.ai for the terminal—accurate voice input for AI coding tools. But it's $12/month with no Linux version. Local models required downloads, resources, and constant setup. I got annoyed. I wanted simplicity and the Unix philosophy.

Whispo costs based on usage—maximum €3/month for typical use. Zero setup, zero configuration.

**For AI coding workflows.** Built for composing prompts to Claude Code, Cursor, and terminal AI tools. Not for coding by voice or general dictation.

**Radically minimal.** One command. No config files, no background daemons, no hotkeys, no models. Run it, speak, press Enter, paste.

**Terminal-native.** Cloud API accuracy without local complexity. Built for developers who live in Linux terminals.

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

## Requirements

- Rust (latest stable)
- OpenAI API key ([get one here](https://platform.openai.com/api-keys))
- FFmpeg (for audio compression)
- Linux with working microphone
- ALSA or PulseAudio

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

## Inspiration

Inspired by [whisp](https://github.com/yummyweb/whisp) - a desktop voice input tool with system tray integration.

## License

MIT
