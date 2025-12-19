# OpenHush

> Open-source voice-to-text that acts as a seamless whisper keyboard.

Press a hotkey, speak, release — your words appear where your cursor is. Powered by local AI, no cloud required.

## Features

- **Push-to-talk** — Hold key to record, release to transcribe
- **Local processing** — All AI runs on your machine (GPU or CPU)
- **Auto-paste** — Text appears where your cursor is
- **Translation mode** — Speak in any language, get English text
- **Audio preprocessing** — RMS normalization, compression, and limiting for cleaner audio
- **System tray** — Background daemon with tray icon and preferences GUI
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
openhush model download small

# Start daemon
openhush start

# Default hotkey: Right Ctrl (hold to record)
# Open preferences GUI
openhush preferences
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
model = "small"        # tiny, base, small, medium, large-v3
language = "auto"      # or "en", "de", etc.
device = "cuda"        # or "cpu"
translate = false      # true = translate to English

[audio]
preprocessing = false  # enable audio preprocessing
[audio.normalization]
enabled = true
target_db = -18.0
[audio.compression]
enabled = true
threshold_db = -20.0
ratio = 4.0
[audio.limiter]
enabled = true
threshold_db = -3.0

[output]
clipboard = true
paste = true

[correction]
enabled = false
ollama_model = "llama3.2:3b"

[queue]
chunk_interval_secs = 0     # 0 = auto-tune based on GPU benchmark
chunk_safety_margin = 0.2   # 20% safety margin for auto-tuned interval
```

### Auto-tuned Streaming

OpenHush automatically benchmarks your GPU at startup to determine the optimal chunk interval for streaming transcription. This ensures:
- Fast feedback without chunks queuing up
- Optimal performance across different GPUs (RTX 3090 → GTX 1060)
- No manual tuning required

Set `chunk_interval_secs = 5.0` (or another value) to override auto-tuning.

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
