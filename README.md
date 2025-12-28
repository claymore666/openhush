# OpenHush

> Local voice-to-text that just works—with the power to do more when you're ready.

Press a hotkey, speak, release — your words appear where your cursor is.

**100% local. Your voice never leaves your device.**

## Privacy First

OpenHush processes everything on your machine:

- **No cloud** — Transcription runs locally using Whisper AI
- **No account** — No sign-up, no login, no tracking
- **No network** — Works fully offline after initial model download
- **No data collection** — We can't see your voice data because it never leaves your computer
- **Secure secrets** — API keys stored in your system's keyring, never in plain text

Your voice is yours. Period.

## Quick Start

```bash
# Install and run the wizard
openhush wizard
```

The wizard will:
1. Download the right model for your hardware
2. Configure your hotkey
3. Test your microphone
4. Start the daemon

**That's it.** Hold Right Ctrl and speak.

Or if you prefer manual setup:
```bash
openhush model download small   # Download a model
openhush start                  # Start the daemon
```

## Capture Anything

OpenHush supports multiple audio sources:

| Input | Description |
|-------|-------------|
| **Microphone** | Any USB, built-in, or Bluetooth mic |
| **System Audio** | Transcribe meetings, calls, videos, podcasts playing on your machine |
| **Multi-Channel** | Select specific channels from pro audio interfaces |
| **Wake Word** | "Hey Computer" for hands-free activation—no hotkey needed |

Mix and match. Capture your mic AND system audio together for meeting transcription with your own comments.

## Output Anywhere

Route your transcriptions wherever you need them:

| Destination | What It Does |
|-------------|--------------|
| **Cursor** | Paste directly into any application |
| **Clipboard** | Copy for manual paste |
| **File** | Save transcripts to disk (text, JSON, SRT) |
| **Translation** | Speak in any language, get English text |
| **LLM Correction** | Grammar, punctuation, filler word removal via Ollama or OpenAI |
| **Meeting Summaries** | AI-generated summaries of long recordings |
| **Custom Hooks** | Trigger any script—send to Notion, commit to git, post to Slack |
| **REST API** | Integrate with external tools, Stream Deck, Home Assistant |

### Output Pipeline Example

```
Voice → Noise Reduction → Whisper → LLM Cleanup → Save to File + Paste at Cursor
```

You control every step.

## Features

### Easy Mode (Works Out of the Box)
- **Push-to-talk** — Hold hotkey, speak, release
- **Toggle mode** — Press once to start, again to stop
- **Auto-paste** — Text appears at your cursor automatically
- **System tray** — Runs quietly in the background
- **Preferences GUI** — Point-and-click settings

### Power Mode (When You Need More)
- **Continuous dictation** — Voice Activity Detection for natural pausing
- **Streaming transcription** — See words appear as you speak
- **Noise reduction** — RNNoise AI filters background noise in real-time
- **Filler word removal** — Strips "um", "uh", "like" automatically
- **Custom vocabulary** — Boost recognition for names, jargon, technical terms
- **Text snippets** — Expand abbreviations (e.g., "sig" → your email signature)
- **Post-transcription hooks** — Run shell commands after each transcription
- **REST API + Swagger UI** — Full remote control and automation
- **D-Bus integration** — Desktop integration on Linux

### Quality Presets
| Preset | Model | Best For |
|--------|-------|----------|
| **Instant** | small | Quick notes, chat |
| **Balanced** | medium | Everyday dictation |
| **Quality** | large-v3 | Important documents |

## Installation

### Quick Install (Recommended)

```bash
# Arch Linux (AUR)
yay -S openhush

# Flatpak (coming soon)
flatpak install flathub org.openhush.OpenHush

# Homebrew (macOS)
brew install openhush
```

### From Source

```bash
git clone https://github.com/claymore666/openhush.git
cd openhush
cargo build --release --features cuda  # or: hipblas, metal, vulkan
sudo cp target/release/openhush /usr/local/bin/
```

See [Building from Source](#building-from-source) for GPU-specific instructions.

## Usage

### Everyday Commands

```bash
openhush start              # Start in background
openhush stop               # Stop the daemon
openhush status             # Check if running
openhush preferences        # Open settings GUI
```

### Model Management

```bash
openhush model list                 # See available models
openhush model download medium      # Download a model
openhush model remove tiny          # Delete a model
```

### File Transcription

```bash
openhush transcribe meeting.wav                    # Transcribe a file
openhush transcribe recording.mp3 --output json    # Output as JSON
openhush transcribe --summarize interview.wav      # Generate AI summary
```

### Configuration

```bash
openhush config --show              # View current settings
openhush config --hotkey F12        # Change hotkey
openhush config --model large-v3    # Use most accurate model
openhush config --language de       # Set language (or "auto")
```

## System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| GPU | None (CPU works) | NVIDIA, AMD, or Apple Silicon |
| RAM | 4 GB | 8 GB |
| Storage | 500 MB | 4 GB (for large models) |

### GPU Acceleration

| GPU | Feature | Platform |
|-----|---------|----------|
| NVIDIA | `cuda` | Linux, Windows |
| AMD | `hipblas` | Linux |
| Apple Silicon | `metal` | macOS |
| Any | `vulkan` | All platforms |

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux (X11) | Full | Primary development platform |
| Linux (Wayland) | Full | KDE Plasma, GNOME, Sway |
| Linux (TTY) | Full | Terminal-only mode |
| macOS | Full | Intel & Apple Silicon |
| Windows | Full | Windows 10/11 |

## Configuration

Settings live in `~/.config/openhush/config.toml`:

```toml
[hotkey]
key = "ControlRight"
mode = "push_to_talk"       # or "toggle", "continuous"

[transcription]
preset = "balanced"         # instant, balanced, quality
language = "auto"           # or "en", "de", "es", etc.
translate = false           # true = always output English

[audio]
channels = "all"            # or [0, 1] for specific channels

[output]
clipboard = true
paste = true

[correction]
enabled = true              # LLM post-processing
remove_fillers = true       # Remove um, uh, like
ollama_url = "http://localhost:11434"
ollama_model = "llama3.2"

[wake_word]
enabled = false
phrase = "hey computer"

[appearance]
theme = "auto"              # auto, light, dark
```

## Building from Source

```bash
git clone https://github.com/claymore666/openhush.git
cd openhush

# CPU only
cargo build --release

# With GPU acceleration
cargo build --release --features cuda      # NVIDIA
cargo build --release --features hipblas   # AMD
cargo build --release --features metal     # Apple Silicon
cargo build --release --features vulkan    # Cross-platform
```

### Linux Dependencies

```bash
# Debian/Ubuntu
sudo apt install libasound2-dev libdbus-1-dev libpulse-dev pkg-config

# Fedora
sudo dnf install alsa-lib-devel dbus-devel pulseaudio-libs-devel

# Arch
sudo pacman -S alsa-lib dbus libpulse
```

## Troubleshooting

### Nothing happens when I press the hotkey
```bash
openhush status           # Is it running?
openhush start -f -v      # Run in foreground with verbose logs
```

### Transcription is slow
- Try a smaller model: `openhush config --model small`
- Verify GPU is working: `nvidia-smi` or check logs

### Where are the logs?
- Main log: `~/.local/share/openhush/openhush.log`
- Crash reports: `~/.local/share/openhush/crash.log`

## Documentation

Visit the **[OpenHush Wiki](https://github.com/claymore666/openhush/wiki)** for:

- [User Guide](https://github.com/claymore666/openhush/wiki/User-Guide) — Detailed usage
- [Architecture](https://github.com/claymore666/openhush/wiki/Architecture) — System design
- [REST API](https://github.com/claymore666/openhush/wiki/REST-API) — API reference
- [Hooks & Automation](https://github.com/claymore666/openhush/wiki/Hooks) — Scripting guide

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License — see [LICENSE](LICENSE) for details.

---

**OpenHush** is not affiliated with OpenAI. Whisper is OpenAI's open-source speech recognition model.
