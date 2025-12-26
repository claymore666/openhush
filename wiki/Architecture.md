# Architecture Overview

This document describes the technical architecture of OpenHush, including data flow, component interactions, and key design decisions.

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              OpenHush Daemon                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────┐   │
│  │   Hotkey     │───▶│    Audio     │───▶│   Transcription Queue    │   │
│  │   Listener   │    │   Recorder   │    │   (async workers)        │   │
│  └──────────────┘    └──────────────┘    └──────────────────────────┘   │
│         │                   │                        │                   │
│         │            ┌──────▼──────┐          ┌──────▼──────┐           │
│         │            │ Ring Buffer │          │   Whisper   │           │
│         │            │  (16kHz)    │          │   Engine    │           │
│         │            └─────────────┘          └─────────────┘           │
│         │                                            │                   │
│         │                                     ┌──────▼──────┐           │
│         │                                     │     VAD     │           │
│         │                                     │  (Silero)   │           │
│         │                                     └─────────────┘           │
│         │                                            │                   │
│  ┌──────▼──────────────────────────────────────────▼─────────────────┐  │
│  │                        Output Pipeline                             │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐   │  │
│  │  │ Correction │─▶│ Vocabulary │─▶│ Clipboard  │─▶│   Paste    │   │  │
│  │  │  (Ollama)  │  │ Replacement│  │            │  │  (enigo)   │   │  │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘   │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                   │
│  │   D-Bus      │  │   System     │  │    Config    │                   │
│  │   Service    │  │   Tray       │  │    Watcher   │                   │
│  └──────────────┘  └──────────────┘  └──────────────┘                   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow

### 1. Audio Pipeline

```
Microphone ──▶ CPAL Capture ──▶ Ring Buffer ──▶ Extract Chunk ──▶ RNNoise ──▶ Resample
                  │                  │               │              │            │
                16kHz              Always         On hotkey      Denoise     To Whisper
                Mono              Running         Release        Optional      16kHz
```

**Key Points:**
- Audio is captured continuously into a ring buffer (no startup delay)
- When the hotkey is released, the relevant audio chunk is extracted
- Optional noise reduction via RNNoise AI
- High-quality resampling via rubato (Sinc interpolation)

### 2. Transcription Pipeline

```
Audio Chunk ──▶ Queue ──▶ Worker Pool ──▶ Whisper ──▶ Text Result
                  │           │              │            │
              Async       Parallel        GPU/CPU     Streaming
             Channel      Workers       Inference      Output
```

**Key Points:**
- Non-blocking architecture with async workers
- Multiple workers for parallel processing
- GPU acceleration when available (CUDA, ROCm, Metal, Vulkan)
- Streaming output mode for immediate feedback

### 3. Output Pipeline

```
Raw Text ──▶ LLM Correction ──▶ Vocabulary ──▶ Clipboard ──▶ Paste
                 │                  │             │            │
            Grammar/Filler    Term Replace    arboard       enigo
              (Ollama)         (TOML)                    (xdotool/wtype)
```

**Key Points:**
- Optional LLM correction via Ollama for grammar and filler removal
- Custom vocabulary replacement for domain-specific terms
- Text copied to clipboard and auto-pasted at cursor

---

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Runtime** | Tokio | Async runtime for non-blocking I/O |
| **Audio Capture** | CPAL | Cross-platform audio library |
| **Speech Recognition** | whisper-rs | Rust bindings for whisper.cpp |
| **Voice Activity** | silero-vad-rust | ONNX-based VAD |
| **Noise Reduction** | nnnoiseless | RNNoise AI denoiser |
| **Resampling** | rubato | High-quality Sinc resampler |
| **Hotkeys** | rdev | Global hotkey detection |
| **Clipboard** | arboard | Cross-platform clipboard |
| **Text Input** | enigo | Keyboard simulation |
| **GUI** | egui/eframe | Immediate mode GUI (Linux) |
| **D-Bus** | zbus | Linux daemon control |
| **System Tray** | ksni | StatusNotifierItem protocol |
| **Config** | toml/serde | Configuration management |
| **Logging** | tracing | Structured logging |

---

## Key Architectural Decisions

### 1. Ring Buffer for Audio Capture

**Problem:** Users expect instant recording when pressing the hotkey.

**Solution:** Continuously capture audio into a ring buffer. When the user presses the hotkey, we already have the last N seconds of audio ready.

```rust
// Simplified ring buffer concept
struct AudioRingBuffer {
    buffer: Vec<f32>,
    write_pos: AtomicUsize,
    capacity: usize,
}
```

### 2. Async Transcription Queue

**Problem:** Transcription takes time, but the user might want to start another recording immediately.

**Solution:** Decouple recording from transcription using an async queue with worker threads.

```
Recording 1 ──▶ Queue ──▶ Worker 1 ──▶ Output
Recording 2 ──▶ Queue ──▶ Worker 2 ──▶ Output
Recording 3 ──▶ Queue ──▶ Worker 1 ──▶ Output (reused)
```

### 3. Platform Abstraction Layer

**Problem:** Different platforms have different APIs for hotkeys, clipboard, text input, etc.

**Solution:** Trait-based abstraction with platform-specific implementations.

```rust
trait TextOutput {
    fn copy_to_clipboard(&self, text: &str) -> Result<()>;
    fn paste(&self, text: &str) -> Result<()>;
}

// Implementations for X11, Wayland, macOS, Windows
```

### 4. GPU Backend Selection at Compile Time

**Problem:** Supporting multiple GPU vendors (NVIDIA, AMD, Apple) with different APIs.

**Solution:** Feature flags for compile-time GPU backend selection.

```bash
cargo build --release --features cuda     # NVIDIA
cargo build --release --features hipblas  # AMD
cargo build --release --features metal    # Apple
cargo build --release --features vulkan   # Cross-platform
```

---

## Configuration System

Configuration is managed via TOML files with hot-reload support.

**Location:** `~/.config/openhush/config.toml`

```toml
[hotkey]
key = "ControlRight"
mode = "push_to_talk"  # or "toggle"

[transcription]
model = "small"
device = "cuda"
language = "auto"
translate = false

[output]
clipboard = true
paste = true

[correction]
enabled = false
ollama_url = "http://localhost:11434"
ollama_model = "llama3.2:3b"

[vad]
enabled = false
threshold = 0.5
```

---

## D-Bus Service Interface (Linux)

The daemon exposes a D-Bus interface for programmatic control.

**Bus Name:** `org.openhush.Daemon1`
**Object Path:** `/org/openhush/Daemon1`

| Method | Description |
|--------|-------------|
| `StartRecording()` | Begin audio capture |
| `StopRecording()` | End audio capture |
| `ToggleRecording()` | Toggle recording state |
| `GetStatus()` | Query daemon status |
| `GetQueueDepth()` | Check pending transcriptions |
| `GetVersion()` | Get daemon version |

---

## Performance Characteristics

| Metric | Value | Notes |
|--------|-------|-------|
| **Audio Latency** | ~10ms | Ring buffer eliminates startup delay |
| **Transcription Speed** | ~0.04x RTF | RTX 3090 with medium model |
| **Memory Usage** | ~50-200MB | Depends on model size |
| **Model Load Time** | 150-2000ms | tiny (150ms) to large-v3 (2000ms) |
| **GPU Memory** | 1-6GB | Depends on model size |

---

## Error Handling Strategy

1. **Graceful Degradation** - Continue without optional features (e.g., no GUI on TTY)
2. **Comprehensive Logging** - All errors logged with context
3. **User Notifications** - Desktop notifications for critical errors
4. **Crash Recovery** - Panic handler saves diagnostic reports to disk

---

## See Also

- [Components](Components) - Detailed component documentation
- [User Guide](User-Guide) - Configuration reference
