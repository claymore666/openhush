# User Guide

This guide covers installation, configuration, and daily usage of OpenHush.

---

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Configuration](#configuration)
4. [CLI Commands](#cli-commands)
5. [Recording Modes](#recording-modes)
6. [Wake Word Detection](#wake-word-detection)
7. [System Audio Capture](#system-audio-capture)
8. [Post-Transcription Actions](#post-transcription-actions)
9. [App-Aware Profiles](#app-aware-profiles)
10. [Secret Management](#secret-management)
11. [File Transcription](#file-transcription)
12. [GPU Acceleration](#gpu-acceleration)
13. [Troubleshooting](#troubleshooting)

---

## Installation

### From Releases (Recommended)

Download the latest release for your platform from the [GitHub Releases](https://github.com/claymore666/openhush/releases) page.

```bash
# Linux (x86_64)
tar -xzf openhush-linux-x86_64.tar.gz
sudo mv openhush /usr/local/bin/

# Verify installation
openhush --version
```

### Building from Source

**Prerequisites:**
- Rust 1.75+
- ALSA development libraries (Linux)
- CUDA Toolkit 11.x+ (for NVIDIA GPU support)

```bash
# Clone the repository
git clone https://github.com/claymore666/openhush.git
cd openhush

# Build without GPU acceleration
cargo build --release

# Build with NVIDIA CUDA support
cargo build --release --features cuda

# Build with AMD ROCm support
cargo build --release --features hipblas

# Build with Apple Metal support (macOS)
cargo build --release --features metal

# Install to PATH
cargo install --path .
```

### Dependencies (Linux)

```bash
# Debian/Ubuntu
sudo apt install libasound2-dev libdbus-1-dev libgtk-3-dev

# Fedora
sudo dnf install alsa-lib-devel dbus-devel gtk3-devel

# Arch Linux
sudo pacman -S alsa-lib dbus gtk3
```

---

## Quick Start

Get transcribing in 2 minutes:

```bash
# 1. Download a Whisper model
openhush model download small

# 2. Start the daemon
openhush start

# 3. Check status
openhush status

# You should see: "Daemon is running (recording: no)"

# 4. Hold the right Control key and speak
# 5. Release the key - text appears at your cursor!
```

### First Transcription

1. Open any text editor or input field
2. Hold the **Right Control** key (default hotkey)
3. Speak clearly into your microphone
4. Release the key
5. Wait for the transcription to appear

---

## Configuration

Configuration is stored in `~/.config/openhush/config.toml`.

### Editing Configuration

```bash
# Using the CLI
openhush config hotkey ControlLeft
openhush config model medium
openhush config language de

# Or edit the file directly
nano ~/.config/openhush/config.toml
```

### Configuration Reference

```toml
# ~/.config/openhush/config.toml

[hotkey]
key = "ControlRight"      # Hotkey to trigger recording
mode = "push_to_talk"     # "push_to_talk" or "toggle"

[transcription]
model = "small"           # tiny, base, small, medium, large-v3
device = "cuda"           # "cuda", "cpu", or specific device
language = "auto"         # "auto" or ISO code ("en", "de", "fr", etc.)
translate = false         # true = always output English

[audio]
resampling_quality = "high"  # "low", "medium", "high"

[output]
clipboard = true          # Copy to clipboard
paste = true              # Auto-paste at cursor

[feedback]
beep_on_start = true      # Audio beep when recording starts
beep_on_stop = true       # Audio beep when recording stops
notifications = true      # Desktop notifications

[correction]
enabled = false           # Enable LLM post-processing
ollama_url = "http://localhost:11434"
ollama_model = "llama3.2:3b"
timeout_secs = 30

[vad]
enabled = false           # Voice Activity Detection
threshold = 0.5           # Speech probability threshold
min_silence_ms = 700      # Silence to end speech
min_speech_ms = 250       # Minimum speech duration

[vocabulary]
enabled = false
path = "~/.config/openhush/vocabulary.toml"
reload_interval_secs = 5

[queue]
max_pending = 0           # 0 = unlimited
separator = " "           # Text between chunks
backpressure = "drop"     # "drop" or "wait"
streaming = true          # Output chunks immediately

[logging]
level = "info"            # "trace", "debug", "info", "warn", "error"

[appearance]
theme = "auto"            # "light", "dark", "auto"
```

---

## CLI Commands

### Daemon Control

```bash
# Start the daemon (background)
openhush start

# Start in foreground (for debugging)
openhush start -f

# Stop the daemon
openhush stop

# Check status
openhush status
```

### Model Management

```bash
# Download a model
openhush model download small
openhush model download large-v3

# List installed models
openhush model list

# Remove a model
openhush model remove tiny
```

### Configuration

```bash
# View current config
openhush config

# Set specific options
openhush config hotkey ControlLeft
openhush config model medium
openhush config language de
openhush config translate on
openhush config llm --model llama3.2:3b --url http://localhost:11434
```

### File Transcription

```bash
# Transcribe a WAV file
openhush transcribe recording.wav

# Output as JSON
openhush transcribe recording.wav --output json

# Specify model
openhush transcribe recording.wav --model large-v3
```

### Recording Control (D-Bus, Linux only)

```bash
# Start recording
openhush recording start

# Stop recording
openhush recording stop

# Toggle recording
openhush recording toggle

# Check recording status
openhush recording status
```

### Status Bar Integration (Waybar/Polybar)

OpenHush provides scripts for status bar integration on Linux:

**Waybar** (`~/.config/waybar/config`):
```json
"custom/openhush": {
    "exec": "/path/to/openhush/contrib/status-bar/waybar-openhush.sh",
    "return-type": "json",
    "interval": 1,
    "on-click": "openhush recording toggle"
}
```

**Waybar styling** (`~/.config/waybar/style.css`):
```css
#custom-openhush.recording {
    color: #f38ba8;
    animation: pulse 1s ease-in-out infinite;
}
#custom-openhush.listening {
    color: #a6e3a1;
}
@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
}
```

**Polybar** (`~/.config/polybar/config.ini`):
```ini
[module/openhush]
type = custom/script
exec = /path/to/openhush/contrib/status-bar/polybar-openhush.sh --polybar-colors
interval = 1
click-left = openhush recording toggle
```

**Icons shown:**
- 󰍬 Idle (ready)
- 󰍮 Listening for wake word
- 󰑊 Recording
- 󰔟 Processing transcription
- 󰍭 Daemon not running

### Preferences GUI

```bash
openhush preferences
```

Opens a graphical settings window. Available on Linux, macOS, and Windows.

### Autostart Service

```bash
# Enable autostart on login
openhush service install

# Disable autostart
openhush service uninstall

# Check service status
openhush service status
```

Works on all platforms:
- **Linux:** Creates systemd user service
- **macOS:** Creates LaunchAgent
- **Windows:** Adds to Registry Run key

---

## Recording Modes

### Push-to-Talk (Default)

- **Hold** the hotkey to record
- **Release** to stop and transcribe

Best for: Short dictations, quick notes, corrections

### Toggle Mode

- **Tap** the hotkey to start recording
- **Tap again** to stop and transcribe

Best for: Longer dictations, hands-free operation

```bash
# Enable toggle mode
openhush config mode toggle

# Return to push-to-talk
openhush config mode push_to_talk
```

---

## Wake Word Detection

Enable hands-free activation with wake word detection. Say "Hey OpenHush" and start speaking — no hotkey needed.

### Configuration

```toml
# ~/.config/openhush/config.toml

[wake_word]
enabled = true              # Enable wake word detection
sensitivity = 0.5           # Detection sensitivity (0.0 = strict, 1.0 = loose)
threshold = 0.5             # Minimum confidence for detection (0.0 - 1.0)
timeout_secs = 10.0         # Max recording time after wake word (0 = until silence)
beep_on_detect = true       # Audio beep when wake word detected
notify_on_detect = true     # Desktop notification on detection
```

### Usage

1. Enable wake word detection in config or via CLI
2. OpenHush continuously listens in the background (low CPU usage)
3. Say "Hey OpenHush" clearly
4. You'll hear a beep and see a notification
5. Start speaking your dictation
6. Recording stops automatically after silence or timeout

### Privacy Note

Wake word detection keeps the microphone listening continuously. Audio is processed locally and never sent anywhere. If privacy is a concern, use the hotkey mode instead.

---

## System Audio Capture

Transcribe meetings, calls, podcasts, or any desktop audio. Works on Linux with PulseAudio or PipeWire.

### Audio Sources

| Source | Description |
|--------|-------------|
| `mic` | Default microphone input (default) |
| `monitor` | System/desktop audio only |
| `both` | Mix of microphone and system audio |

### Configuration

```toml
# ~/.config/openhush/config.toml

[audio]
source = "monitor"      # mic, monitor, or both
```

### CLI Usage

```bash
# Transcribe system audio
openhush start --source monitor

# Transcribe file with system audio capture
openhush transcribe --source monitor

# Mix microphone and system audio
openhush start --source both
```

### Use Cases

- **Meeting transcription** — Capture Zoom, Teams, or Google Meet audio
- **Podcast notes** — Transcribe while listening
- **Video captions** — Generate subtitles from video playback
- **Language learning** — Transcribe foreign language content

### Requirements

- Linux with PulseAudio or PipeWire
- PipeWire users: PulseAudio compatibility layer (usually installed by default)

### Troubleshooting

```bash
# List available monitor sources
pactl list sources short | grep monitor

# Test recording from monitor
parecord -d alsa_output.pci-0000_00_1f.3.analog-stereo.monitor test.wav
```

---

## Post-Transcription Actions

Run custom actions after each transcription: shell commands, HTTP requests, or file logging.

### Configuration

```toml
# ~/.config/openhush/config.toml

[output]
clipboard = true
paste = true

# Notify on transcription
[[output.actions]]
type = "shell"
command = "notify-send 'OpenHush' '{text}'"
enabled = true

# Log all transcriptions to file
[[output.actions]]
type = "file"
path = "~/Documents/dictation/{date}.md"
format = "- [{time}] {text}\n"
append = true
enabled = true

# Send to webhook
[[output.actions]]
type = "http"
url = "http://localhost:8080/api/notes"
method = "POST"
body = '{"content": "{text_escaped}", "source": "voice"}'
headers = { "Content-Type" = "application/json" }
enabled = false
```

### Action Types

| Type | Description |
|------|-------------|
| `shell` | Run a shell command |
| `file` | Append or write to a file |
| `http` | Send an HTTP request |

### Variables

Use these placeholders in commands, URLs, and templates:

| Variable | Description |
|----------|-------------|
| `{text}` | Raw transcribed text |
| `{text_escaped}` | JSON-escaped text (for API payloads) |
| `{text_base64}` | Base64-encoded text |
| `{date}` | Current date (YYYY-MM-DD) |
| `{time}` | Current time (HH:MM:SS) |
| `{duration}` | Recording duration in seconds |
| `{model}` | Whisper model used |
| `{seq_id}` | Transcription sequence ID |

### Examples

**Voice notes to Obsidian:**
```toml
[[output.actions]]
type = "file"
path = "~/Obsidian/Voice Notes/{date}.md"
format = "## {time}\n{text}\n\n"
append = true
```

**Slack notification:**
```toml
[[output.actions]]
type = "http"
url = "https://hooks.slack.com/services/XXX/YYY/ZZZ"
method = "POST"
body = '{"text": "{text_escaped}"}'
```

---

## App-Aware Profiles

Configure different settings per application. For example, use aggressive filler word removal in email clients but conservative mode in code editors.

### Configuration

```toml
# ~/.config/openhush/config.toml

# Default settings apply when no profile matches
[correction]
filler_removal = "moderate"

# Profile for email apps
[[profiles]]
name = "email"
apps = ["Thunderbird", "Evolution", "Geary", "Gmail"]
filler_removal = "aggressive"
vocabulary_file = "~/.config/openhush/vocab-email.toml"

# Profile for code editors
[[profiles]]
name = "code"
apps = ["Code", "code-oss", "vim", "nvim", "Sublime"]
filler_removal = "conservative"
vocabulary_file = "~/.config/openhush/vocab-code.toml"

# Disable in browsers
[[profiles]]
name = "disabled"
apps = ["Firefox", "Chromium", "Chrome"]
enabled = false
```

### How It Works

1. OpenHush detects the currently focused application
2. Matches against profile `apps` list (case-insensitive, partial match)
3. Applies profile overrides (vocabulary, filler removal, etc.)
4. Falls back to default settings if no profile matches

### Platform Support

| Platform | Detection Method |
|----------|------------------|
| Linux X11 | `xdotool getactivewindow` |
| Linux Wayland | Hyprland/Sway IPC |
| macOS | `NSWorkspace.frontmostApplication` |
| Windows | `GetForegroundWindow()` |

---

## Secret Management

Store API keys securely in your platform's keyring instead of plaintext config files.

### CLI Commands

```bash
# Store a secret
openhush secret set ollama-api
Enter secret: ********
Secret 'ollama-api' stored securely.

# List stored secrets (names only)
openhush secret list
Stored secrets:
  - ollama-api
  - slack-webhook

# Delete a secret
openhush secret delete ollama-api
Secret 'ollama-api' deleted.

# Show a secret (use with caution)
openhush secret show ollama-api
```

### Using Secrets in Config

Reference secrets with the `keyring:` prefix:

```toml
# ~/.config/openhush/config.toml

[correction]
# Reference secret from keyring (recommended)
ollama_api_key = "keyring:ollama-api"

# Inline secret (NOT recommended, will show warning)
# ollama_api_key = "sk-plaintext-key-here"
```

### Platform Keyrings

| Platform | Backend |
|----------|---------|
| Linux | Secret Service (GNOME Keyring, KWallet) |
| macOS | Keychain |
| Windows | Credential Manager |

---

## File Transcription

Transcribe existing audio files (WAV, MP3).

### Basic Usage

```bash
# Transcribe a file
openhush transcribe meeting.wav

# Output:
# Transcription:
# Welcome to the standup meeting. Let's go around the room...
#
# Audio duration: 1847.0 seconds
# Transcription time: 72.36 seconds
# Real-time factor: 0.039x
```

### JSON Output

```bash
openhush transcribe meeting.wav --output json

# Output:
{
  "text": "Welcome to the standup meeting...",
  "language": "en",
  "duration_ms": 1847000,
  "audio_duration_secs": 1847.0,
  "transcription_time_ms": 72360,
  "real_time_factor": 0.039,
  "model": "medium"
}
```

### Performance

| Model | RTX 3090 | CPU (Ryzen 9) |
|-------|----------|---------------|
| tiny | 0.01x RTF | 0.1x RTF |
| small | 0.02x RTF | 0.3x RTF |
| medium | 0.04x RTF | 0.6x RTF |
| large-v3 | 0.08x RTF | 1.2x RTF |

RTF = Real-Time Factor (lower is faster, 0.04x means 25x real-time speed)

---

## GPU Acceleration

### NVIDIA CUDA

**Requirements:** CUDA Toolkit 11.x+

```bash
# Check CUDA version
nvidia-smi

# Build with CUDA support
cargo build --release --features cuda

# Set device (if multiple GPUs)
openhush config device cuda:0
```

### AMD ROCm

**Requirements:** ROCm 5.x+

```bash
# Check ROCm version
rocminfo

# Build with ROCm support
cargo build --release --features hipblas

# Set device
openhush config device hip:0
```

### Apple Metal

**Requirements:** macOS with Apple Silicon (M1/M2/M3)

```bash
# Build with Metal support
cargo build --release --features metal

# Device is auto-detected
```

### Vulkan (Cross-platform)

**Requirements:** Vulkan SDK

```bash
# Build with Vulkan support
cargo build --release --features vulkan
```

---

## Troubleshooting

### Daemon Won't Start

```bash
# Check if already running
openhush status

# Check for errors
openhush start -f  # Run in foreground

# Check logs
cat ~/.local/share/openhush/openhush.log
```

### No Audio Captured

```bash
# Check microphone permissions
pactl list sources short

# Test microphone
arecord -d 5 test.wav && aplay test.wav

# Check audio device in config
openhush config device
```

### Transcription Errors

```bash
# Check model is installed
openhush model list

# Try a smaller model
openhush config model small

# Check GPU memory
nvidia-smi
```

### Hotkey Not Working

```bash
# Wayland users: Check for evdev permissions
sudo usermod -a -G input $USER
# Log out and back in

# X11 users: Check for conflicts
xev  # Press your hotkey, check if it's captured

# Try a different hotkey
openhush config hotkey F12
```

### Paste Not Working

```bash
# Wayland users: Install wtype
sudo apt install wtype

# X11 users: Install xdotool
sudo apt install xdotool

# Check output mode
openhush config paste on
openhush config clipboard on
```

### High Memory Usage

```bash
# Use a smaller model
openhush config model small  # 466MB vs 1.5GB for medium

# Check for memory leaks
watch -n 1 'ps aux | grep openhush'
```

### Slow Transcription

```bash
# Enable GPU acceleration (rebuild required)
cargo build --release --features cuda

# Use a smaller model
openhush config model small

# Check GPU utilization
nvidia-smi -l 1
```

---

## Getting Help

- **Documentation:** This wiki
- **Issues:** [GitHub Issues](https://github.com/claymore666/openhush/issues)
- **Logs:** `~/.local/share/openhush/openhush.log`

---

## See Also

- [Architecture](Architecture) - How OpenHush works
- [Components](Components) - Module documentation
- [Product Vision](Product-Vision) - Future plans
