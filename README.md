# OpenHush

> Open-source voice-to-text that acts as a seamless whisper keyboard.

Press a hotkey, speak, release — your words appear where your cursor is. Powered by local AI, no cloud required.

## Why OpenHush?

- **Privacy first** — Your voice never leaves your computer. No cloud, no subscriptions, no data collection.
- **Works everywhere** — Type into any application: emails, documents, chat, code editors, terminals.
- **Fast** — GPU acceleration gives you results in under a second for most dictation.
- **Accurate** — Uses OpenAI's Whisper models, the same tech behind ChatGPT's voice features.
- **Free forever** — Open source, MIT licensed. No trials, no premium tiers.

## Features

### Core Dictation
- **Push-to-talk** — Hold your hotkey, speak, release. Text appears at your cursor.
- **Toggle mode** — Press once to start recording, press again to stop.
- **Auto-paste** — Transcribed text is typed automatically, or copied to clipboard.
- **Translation** — Speak in any language, get English text (great for multilingual users).

### Smart Processing
- **Continuous dictation** — Keep talking naturally; OpenHush detects pauses and transcribes automatically.
- **Noise reduction** — AI-powered background noise removal (keyboard, fans, traffic).
- **Filler word cleanup** — Removes "um", "uh", "like", "you know" from your speech.
- **Custom vocabulary** — Add names, jargon, or terms that Whisper gets wrong.
- **Text snippets** — Expand abbreviations (e.g., "sig" → your email signature).

### Quality Options
- **Instant mode** — Fastest response, good for quick notes and chat.
- **Balanced mode** — Best of both worlds (default).
- **Quality mode** — Most accurate, for important documents.

### User Experience
- **System tray** — Runs quietly in the background with status indicator.
- **Preferences GUI** — Point-and-click settings, no config files needed.
- **Dark mode** — Follows your system theme (or choose manually).
- **Crash recovery** — If something goes wrong, diagnostic reports help fix it.

## Quick Start

```bash
# Install (requires Rust)
cargo install openhush

# Download a model (first time only)
openhush model download small

# Start the daemon
openhush start

# That's it! Hold Right Ctrl and speak.
```

## Usage

### Basic Commands

```bash
openhush start              # Start in background
openhush stop               # Stop the daemon
openhush status             # Check if running
openhush preferences        # Open settings window
```

### Model Management

```bash
openhush model list                 # See available models
openhush model download medium      # Download a model
openhush model remove tiny          # Delete a model
```

### Configuration

```bash
openhush config --show              # View current settings
openhush config --hotkey F12        # Change hotkey
openhush config --model large-v3    # Use most accurate model
openhush config --language de       # Set language (or "auto")
```

## Choosing a Model

| Model | Download | Speed | Best For |
|-------|----------|-------|----------|
| tiny | 75 MB | Instant | Quick notes, chat |
| base | 142 MB | Very fast | Everyday use |
| small | 466 MB | Fast | General dictation |
| medium | 1.5 GB | Moderate | Professional use |
| large-v3 | 3 GB | Slower | Maximum accuracy |

**Recommendation:** Start with `small`. Upgrade to `medium` or `large-v3` if accuracy matters more than speed.

## System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| GPU | None (CPU works) | NVIDIA, AMD, or Apple Silicon |
| RAM | 4 GB | 8 GB |
| Storage | 500 MB | 4 GB (for large models) |

### GPU Acceleration

GPU acceleration significantly speeds up transcription. OpenHush supports multiple GPU backends:

| GPU | Feature | Platform | Requirements |
|-----|---------|----------|--------------|
| NVIDIA | `cuda` | Linux, Windows | CUDA Toolkit 11.x+ |
| AMD | `hipblas` | Linux | ROCm 5.x+ |
| Apple Silicon | `metal` | macOS | Built-in (M1/M2/M3) |
| Any | `vulkan` | All | Vulkan SDK |

Build with your GPU's feature flag (see [Building from Source](#building-from-source)).

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux (X11) | ✅ Full | Ubuntu, Fedora, Debian, etc. |
| Linux (Wayland) | ✅ Full | KDE Plasma, GNOME, Sway |
| Linux (TTY) | ✅ Full | Terminal-only mode |
| macOS | ✅ Basic | Intel & Apple Silicon |
| Windows | ✅ Basic | Windows 10/11 |

## Configuration File

Settings are stored in `~/.config/openhush/config.toml`:

```toml
[hotkey]
key = "ControlRight"      # Try: F12, AltRight, etc.
mode = "push_to_talk"     # or "toggle"

[transcription]
preset = "balanced"       # instant, balanced, quality
language = "auto"         # or "en", "de", "es", etc.
translate = false         # true = always output English

[output]
clipboard = true          # Copy to clipboard
paste = true              # Auto-type at cursor

[feedback]
audio = true              # Beep when recording starts/stops
visual = true             # Desktop notifications

[appearance]
theme = "auto"            # auto, light, dark

[correction]
enabled = false           # Enable LLM post-processing
remove_fillers = false    # Remove um, uh, like
```

## Troubleshooting

### Nothing happens when I press the hotkey
```bash
openhush status           # Is it running?
openhush start -f -v      # Run in foreground with verbose logs
```

### Transcription is slow
- Try a smaller model: `openhush config --model small`
- Make sure CUDA is working: check `nvidia-smi`

### Text appears in wrong place
- Some Wayland apps need `wtype` installed
- Some X11 apps need `xdotool` installed

### Where are the logs?
- Main log: `~/.local/share/openhush/openhush.log`
- Crash reports: `~/.local/share/openhush/crash.log`

## Building from Source

```bash
git clone https://github.com/claymore666/openhush.git
cd openhush

# CPU only (no GPU acceleration)
cargo build --release

# NVIDIA GPU (CUDA)
cargo build --release --features cuda

# AMD GPU (ROCm/HIP)
cargo build --release --features hipblas

# Apple Silicon (Metal)
cargo build --release --features metal

# Cross-platform (Vulkan)
cargo build --release --features vulkan
```

**Note:** Only one GPU feature can be enabled at compile time. Choose the one matching your hardware.

### Linux Dependencies

```bash
# Debian/Ubuntu
sudo apt install libasound2-dev libdbus-1-dev pkg-config

# Fedora
sudo dnf install alsa-lib-devel dbus-devel

# Arch
sudo pacman -S alsa-lib dbus
```

## Roadmap

### Coming Soon
- Wake word activation ("Hey OpenHush")
- System audio capture (transcribe meetings)
- App-specific settings (different config per application)
- Plugin system for community extensions

### Packaging
- Flatpak
- AUR (Arch User Repository)
- Homebrew (macOS)
- Chocolatey/winget (Windows)

See the [GitHub milestones](https://github.com/claymore666/openhush/milestones) for detailed plans.

## Contributing

Contributions welcome! Please read our contributing guidelines before submitting PRs.

## License

MIT License — see [LICENSE](LICENSE) for details.

---

**OpenHush** is not affiliated with OpenAI. Whisper is OpenAI's open-source speech recognition model.
