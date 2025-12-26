# Component Deep Dive

This document provides detailed documentation for each major component in OpenHush.

---

## Table of Contents

1. [Daemon](#daemon)
2. [Audio Input](#audio-input)
3. [Whisper Engine](#whisper-engine)
4. [Transcription Queue](#transcription-queue)
5. [Output Handlers](#output-handlers)
6. [VAD System](#vad-system)
7. [LLM Correction](#llm-correction)
8. [Vocabulary System](#vocabulary-system)
9. [Platform Layer](#platform-layer)
10. [GUI/Preferences](#guipreferences)
11. [D-Bus Interface](#d-bus-interface)

---

## Daemon

**Location:** `src/daemon.rs`

The daemon is the heart of OpenHush, orchestrating all other components.

### Responsibilities

- Initialize and coordinate all subsystems
- Handle hotkey events (start/stop recording)
- Manage recording sessions
- Route audio to transcription queue
- Deliver results to output handlers

### Key Functions

| Function | Purpose |
|----------|---------|
| `run()` | Main entry point, starts event loop |
| `run_loop()` | Core event loop processing |
| `start_recording()` | Begin audio capture session |
| `stop_recording()` | End session, queue for transcription |

### State Management

```rust
enum DaemonState {
    Idle,
    Recording { session_id: u64, start_time: Instant },
    Processing { pending_count: usize },
}
```

---

## Audio Input

**Location:** `src/input/`

### AudioRecorder (`audio.rs`)

Captures audio from the system microphone using CPAL.

**Key Features:**
- 16kHz mono capture (Whisper requirement)
- 32-bit float samples [-1.0, 1.0]
- Automatic device detection with fallback
- Hardware disconnect handling

```rust
pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    ring_buffer: Arc<AudioRingBuffer>,
    device: cpal::Device,
}
```

### AudioRingBuffer

Continuous audio capture without startup delay.

**Configuration:**
- Default: 30 seconds of audio
- Sample rate: 16kHz
- Memory: ~1.9MB for 30 seconds

### HotkeyListener (`hotkey.rs`)

Global hotkey detection using `rdev`.

**Supported Modes:**
- `push_to_talk` - Hold to record, release to transcribe
- `toggle` - Tap to start, tap again to stop

**Supported Keys:**
- Control keys: `ControlRight`, `ControlLeft`, `AltRight`, `AltLeft`
- Function keys: `F1`-`F12`
- Special keys: `Space`, `Escape`, etc.

---

## Whisper Engine

**Location:** `src/engine/whisper.rs`

Rust bindings to whisper.cpp for speech recognition.

### Available Models

| Model | Size | Speed | Quality | Use Case |
|-------|------|-------|---------|----------|
| tiny | 75MB | Instant | Basic | Quick notes |
| base | 142MB | Very fast | Good | Everyday use |
| small | 466MB | Fast | Better | General dictation |
| medium | 1.5GB | Moderate | High | Professional use |
| large-v3 | 3GB | Slower | Best | Maximum accuracy |

### Model Management

Models are stored in `~/.local/share/openhush/models/`.

```bash
# Download a model
openhush model download small

# List installed models
openhush model list

# Remove a model
openhush model remove tiny
```

### GPU Acceleration

| Backend | Platform | Requirements |
|---------|----------|--------------|
| CUDA | Linux, Windows | NVIDIA GPU, CUDA Toolkit 11.x+ |
| HIP/ROCm | Linux | AMD GPU, ROCm 5.x+ |
| Metal | macOS | Apple Silicon (M1/M2/M3) |
| Vulkan | All | Vulkan SDK |

---

## Transcription Queue

**Location:** `src/queue/`

Async queue for non-blocking transcription.

### Architecture

```
                    ┌─────────────┐
Audio Chunks ──────▶│    Queue    │──────▶ Worker 1 ──▶ Results
                    │  (channel)  │──────▶ Worker 2 ──▶ Results
                    └─────────────┘──────▶ Worker N ──▶ Results
```

### Configuration

```toml
[queue]
max_pending = 0           # 0 = unlimited
separator = " "           # Text between chunks
backpressure = "drop"     # or "wait"
streaming = true          # Output immediately
```

### TranscriptionTracker

Manages ordering and deduplication of results.

```rust
pub struct TranscriptionTracker {
    pending: HashMap<u64, PendingResult>,
    completed: VecDeque<CompletedResult>,
    next_output_id: u64,
}
```

---

## Output Handlers

**Location:** `src/output/`

### Clipboard (`clipboard.rs`)

Cross-platform clipboard using `arboard`.

```rust
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}
```

### Paste (`paste.rs`)

Platform-specific text input simulation.

| Platform | Method |
|----------|--------|
| X11 | xdotool type |
| Wayland | wtype |
| macOS | Native keyboard events |
| Windows | SendInput API |

---

## VAD System

**Location:** `src/vad/`

Voice Activity Detection using Silero VAD.

### How It Works

1. Audio is processed in 512-sample chunks (32ms)
2. Each chunk gets a speech probability [0.0, 1.0]
3. Speech segments are detected when probability exceeds threshold
4. Segments are padded and merged based on configuration

### Configuration

```toml
[vad]
enabled = false
threshold = 0.5           # Speech probability threshold
min_silence_ms = 700      # Silence to end speech
min_speech_ms = 250       # Minimum speech duration
speech_pad_ms = 30        # Padding around speech
```

### Use Cases

- **Continuous Dictation** - Automatically detect speech without hotkey
- **Meeting Recording** - Segment long recordings by speaker pauses

---

## LLM Correction

**Location:** `src/correction/mod.rs`

Optional post-processing via Ollama.

### Features

- Grammar correction
- Filler word removal (um, uh, like, you know)
- Punctuation improvement

### Configuration

```toml
[correction]
enabled = false
ollama_url = "http://localhost:11434"
ollama_model = "llama3.2:3b"
timeout_secs = 30
```

### Filler Removal Modes

| Mode | Behavior |
|------|----------|
| `disabled` | No filler removal |
| `aggressive` | Remove all detected fillers |
| `conservative` | Remove only obvious fillers |

---

## Vocabulary System

**Location:** `src/vocabulary/mod.rs`

Domain-specific term replacement.

### Vocabulary File Format

```toml
# ~/.config/openhush/vocabulary.toml

[replacements]
enabled = true
case_sensitive = false
"gonna" = "going to"
"wanna" = "want to"

[medical]
enabled = true
case_sensitive = true
"bp" = "blood pressure"
"rx" = "prescription"

[acronyms]
enabled = true
case_sensitive = true
"AI" = "artificial intelligence"
```

### Features

- Multiple vocabulary sections
- Case-sensitive or case-insensitive matching
- Hot-reload support (configurable interval)

---

## Platform Layer

**Location:** `src/platform/`

Abstraction for platform-specific functionality.

### Display Server Detection

```rust
pub enum DisplayServer {
    X11,
    Wayland,
    Windows,
    MacOS,
    TTY,  // Headless/terminal
}
```

Detection uses environment variables:
- `WAYLAND_DISPLAY` → Wayland
- `DISPLAY` → X11
- `TERM` (without DISPLAY) → TTY

### Platform Traits

| Trait | Purpose |
|-------|---------|
| `HotkeyHandler` | Platform-specific hotkey detection |
| `TextOutput` | Clipboard and paste operations |
| `Notifier` | Desktop notifications |
| `AudioFeedback` | Audio beep sounds |
| `SystemTray` | Tray icon and menu |

---

## GUI/Preferences

**Location:** `src/gui/mod.rs`

Preferences GUI using egui framework.

### Features

- Point-and-click configuration
- Theme auto-detection (dark/light)
- Real-time preview

### Availability

| Platform | Status |
|----------|--------|
| Linux (X11) | Full support |
| Linux (Wayland) | Full support |
| Linux (TTY) | Not available |
| macOS | Full support |
| Windows | Full support |

---

## IPC (Inter-Process Communication)

**Location:** `src/ipc/`

Cross-platform daemon control.

### Platform Implementations

| Platform | Method | Location |
|----------|--------|----------|
| Linux | D-Bus | `src/dbus/service.rs` |
| macOS | Unix sockets | `src/ipc/unix_socket.rs` |
| Windows | Named pipes | `src/ipc/named_pipe.rs` |

### Commands

| Command | Description |
|---------|-------------|
| `status` | Query daemon status |
| `stop` | Stop the daemon |

### Usage

```bash
# Check if daemon is running
openhush status

# Stop the daemon
openhush stop
```

---

## Service Management

**Location:** `src/service/`

Cross-platform autostart management.

### Platform Implementations

| Platform | Method | Location |
|----------|--------|----------|
| Linux | systemd user service | `src/service/linux.rs` |
| macOS | LaunchAgent | `src/service/macos.rs` |
| Windows | Registry Run key | `src/service/windows.rs` |

### Service Files

- **Linux:** `~/.config/systemd/user/openhush.service`
- **macOS:** `~/Library/LaunchAgents/org.openhush.daemon.plist`
- **Windows:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

### CLI Usage

```bash
# Install autostart
openhush service install

# Remove autostart
openhush service uninstall

# Check status
openhush service status
```

---

## D-Bus Interface

**Location:** `src/dbus/service.rs`

Linux-only daemon control via D-Bus.

### Bus Information

- **Bus Name:** `org.openhush.Daemon1`
- **Object Path:** `/org/openhush/Daemon1`

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `StartRecording` | `() → ()` | Begin audio capture |
| `StopRecording` | `() → ()` | End audio capture |
| `ToggleRecording` | `() → ()` | Toggle recording state |
| `GetStatus` | `() → s` | Query daemon status |
| `GetQueueDepth` | `() → u` | Pending transcriptions |
| `GetVersion` | `() → s` | Daemon version |

### Signals

| Signal | Description |
|--------|-------------|
| `IsRecordingChanged` | Emitted when recording starts/stops |

### CLI Usage

```bash
# Start recording
openhush recording start

# Stop recording
openhush recording stop

# Check status
openhush recording status
```

---

## See Also

- [Architecture](Architecture) - System overview
- [User Guide](User-Guide) - Configuration reference
