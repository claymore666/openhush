# OpenHush

> Open-source voice-to-text that acts as a seamless whisper keyboard.

Press a hotkey, speak, release — your words appear where your cursor is. Powered by local AI, no cloud required.

## Features

- **Push-to-talk** — Hold key to record, release to transcribe
- **Local processing** — All AI runs on your machine (GPU or CPU)
- **Auto-paste** — Text appears where your cursor is
- **LLM correction** — Optional grammar/punctuation fix via Ollama
- **Cross-platform** — Linux, macOS, Windows
- **Wayland native** — Full KDE Plasma / GNOME support
- **Terminal mode** — Works in TTY without X server
- **Open source** — MIT licensed, no telemetry, no cloud

## Quick Start

```bash
# Install
cargo install openhush

# Download model (first time)
openhush model download large-v3

# Start daemon
openhush start

# Default hotkey: Right Ctrl (hold to record)
```

## Requirements

- **GPU (recommended)**: NVIDIA with CUDA drivers
- **CPU**: Works without GPU (slower)
- **RAM**: 4-8GB depending on model
- **Disk**: 75MB - 3GB for model files

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

[output]
clipboard = true
paste = true

[correction]
enabled = false
ollama_model = "llama3.2:3b"
```

## Platforms

| Platform | Status | Paste Method |
|----------|--------|--------------|
| Linux X11 | ✅ | xdotool/enigo |
| Linux Wayland | ✅ | wtype |
| Linux TTY | ✅ | evdev |
| macOS | 🚧 | CGEvent |
| Windows | 🚧 | SendInput |

## Building from Source

```bash
# Clone
git clone https://github.com/claymore666/openhush.git
cd openhush

# Build (release)
cargo build --release

# With CUDA support
cargo build --release --features cuda

# Install
cargo install --path .
```

## License

MIT License — see [LICENSE](LICENSE)

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting PRs.
