# OpenHush

> Open-source voice-to-text that acts as a seamless whisper keyboard.

Press a hotkey, speak, release — your words appear where your cursor is. Powered by local AI, no cloud required.

## Features

- **Push-to-talk** — Hold key to record, release to transcribe
- **Toggle mode** — Press to start, press again to stop
- **Local AI** — All processing on your machine, no internet needed
- **GPU accelerated** — Fast transcription with CUDA (works on CPU too)
- **Auto-paste** — Text appears where your cursor is
- **Translation** — Speak any language, get English text
- **LLM correction** — Optional grammar fix via Ollama
- **Audio preprocessing** — Normalization, compression, limiting
- **Streaming** — Real-time transcription for long recordings
- **System tray** — Background daemon with tray icon
- **Preferences GUI** — Easy configuration (Linux)
- **Wayland native** — Full KDE/GNOME/Sway support
- **Terminal mode** — Works in TTY without X server
- **Crash recovery** — Detailed diagnostics when things go wrong
- **Open source** — MIT licensed, no telemetry

### Planned

- **Wake word** — "Hey OpenHush" hands-free activation
- **Continuous dictation** — Voice activity detection (Silero-VAD)
- **Noise reduction** — RNNoise background noise removal
- **Custom vocabulary** — Domain-specific terms and names
- **Filler word removal** — Remove "um", "uh", "like" via LLM
- **Text snippets** — Expand shortcuts (e.g., "sig" → full signature)
- **Post-transcription actions** — Run shell commands or API calls
- **System audio capture** — Transcribe meetings with speaker labels
- **App-aware context** — Different settings per application
- **macOS support** — Native macOS integration
- **Windows support** — Native Windows integration
- **Flatpak / AUR** — Easy installation packages
- **Dark mode** — Theme support for preferences GUI
- **Onboarding wizard** — First-run setup guide
- **Plugin system** — Community extensions

## Quick Start

```bash
# Install
cargo install openhush

# Download model (first time)
openhush model download small

# Start daemon
openhush start

# Default hotkey: Right Ctrl (hold to record)
```

## CLI Commands

```bash
openhush start              # Start daemon
openhush start --foreground # Run in foreground
openhush stop               # Stop daemon
openhush status             # Check if running
openhush preferences        # Open settings GUI (Linux)

openhush config --show      # View config
openhush config --hotkey F12
openhush config --model large-v3
openhush config --language de
openhush config --translate true
openhush config --llm ollama:llama3.2:3b

openhush model list         # Show available models
```

## Requirements

- **GPU (recommended)**: NVIDIA with CUDA
- **CPU**: Works without GPU (slower)
- **RAM**: 4-8GB depending on model

### Models

| Model | Size | Speed | Accuracy |
|-------|------|-------|----------|
| tiny | 75MB | Fastest | Basic |
| base | 142MB | Fast | Good |
| small | 466MB | Balanced | Better |
| medium | 1.5GB | Slower | High |
| large-v3 | 3GB | Slowest | Best |

## Configuration

Config file: `~/.config/openhush/config.toml`

```toml
[hotkey]
key = "ControlRight"
mode = "push_to_talk"  # or "toggle"

[transcription]
model = "large-v3"     # tiny, base, small, medium, large-v3
language = "auto"      # or "en", "de", etc.
device = "cuda"        # or "cpu"
translate = false      # true = translate to English

[output]
clipboard = true
paste = true

[correction]
enabled = false
ollama_model = "llama3.2:3b"

[feedback]
audio = true           # Beep sounds
visual = true          # Desktop notifications

[audio]
preprocessing = false  # Enable audio processing
```

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux X11 | Full | Recommended |
| Linux Wayland | Full | KDE/GNOME/Sway |
| Linux TTY | Full | Terminal mode |
| macOS | WIP | Coming soon |
| Windows | WIP | Coming soon |

## Building from Source

```bash
git clone https://github.com/claymore666/openhush.git
cd openhush
cargo build --release
cargo install --path .
```

### Linux Dependencies

```bash
# Debian/Ubuntu
sudo apt install libasound2-dev libgtk-3-dev libxdo-dev

# Fedora
sudo dnf install alsa-lib-devel gtk3-devel libxdo-devel

# Arch
sudo pacman -S alsa-lib gtk3 xdotool
```

## Troubleshooting

```bash
openhush status          # Check if running
openhush stop            # Stop existing daemon
openhush start -f -v     # Foreground with verbose logging
```

Logs: `~/.local/share/openhush/openhush.log`
Crash reports: `~/.local/share/openhush/crash.log`

## License

MIT License — see [LICENSE](LICENSE)
