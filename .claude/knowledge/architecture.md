# OpenHush Architecture

## High-Level Data Flow

```
┌──────────┐    ┌─────────────┐    ┌──────────┐    ┌─────────────┐    ┌────────┐
│  Hotkey  │───▶│  Audio      │───▶│  Queue   │───▶│  GPU Pool   │───▶│ Output │
│  Handler │    │  Capture    │    │ (seq ID) │    │  Whisper    │    │ Paste  │
└──────────┘    └─────────────┘    └──────────┘    └─────────────┘    └────────┘
     │                                   │                │                │
     │                                   │                │                │
     ▼                                   ▼                ▼                ▼
  rdev/evdev              Recording{seq_id, audio}    Per-GPU worker    Platform
                                                      threads           dispatch
```

## Module Responsibilities

| Module | Responsibility | Owns |
|--------|----------------|------|
| `main.rs` | CLI parsing, command dispatch | CLI args |
| `config.rs` | Load/save TOML config, XDG paths | `Config` struct |
| `daemon.rs` | Main loop, orchestration | Daemon lifecycle |
| `platform/mod.rs` | Platform traits | `HotkeyHandler`, `TextOutput`, `Notifier`, `AudioFeedback` |
| `platform/linux.rs` | Linux implementation | X11/Wayland/TTY dispatch |
| `input/hotkey.rs` | Global hotkey capture | Key state machine |
| `input/audio.rs` | Microphone capture | Audio buffer (16kHz mono) |
| `queue/recording.rs` | Recording queue | Sequence ID assignment |
| `queue/result.rs` | Result reordering | Reorder buffer |
| `gpu/pool.rs` | GPU enumeration, model loading | GPU handles, loaded models |
| `gpu/worker.rs` | Per-GPU transcription | Worker thread |
| `gpu/scheduler.rs` | Job assignment | Load balancing |
| `engine/whisper.rs` | Whisper API wrapper | Transcription call |
| `engine/vad.rs` | Voice activity detection | Silence trimming |
| `correction/ollama.rs` | LLM post-processing | HTTP client |
| `output/clipboard.rs` | Clipboard operations | Clipboard handle |
| `output/paste.rs` | Platform paste dispatch | Paste strategy |

## Key Data Structures

```rust
// Recording with sequence ID for ordered output
struct Recording {
    sequence_id: u64,
    audio_data: Vec<f32>,  // 16kHz mono PCM
    timestamp: Instant,
}

// Transcription result
struct TranscriptionResult {
    sequence_id: u64,
    text: String,
    confidence: f32,
}

// GPU worker handle
struct GpuWorker {
    device_id: u32,
    model: WhisperContext,
    busy: AtomicBool,
}
```

## Key Invariants

1. **Output order = Input order**: Sequence IDs ensure recordings paste in speech order
2. **No dropped recordings**: Queue never drops unless `max_pending` exceeded
3. **Graceful degradation**: GPU fail → retry other GPU → CPU fallback
4. **Model stays loaded**: Daemon keeps model in memory for low latency
5. **Platform isolation**: All platform-specific code behind traits

## Thread Model

```
Main Thread (Tokio)
├── Hotkey listener (async)
├── Audio capture (async, polls cpal)
├── Result collector (async, receives from workers)
└── Output handler (async, pastes in order)

GPU Worker Threads (std::thread)
├── GPU 0 worker (blocking whisper calls)
├── GPU 1 worker
└── GPU N worker

Channels:
├── recording_tx/rx: Main → Workers (recordings to transcribe)
└── result_tx/rx: Workers → Main (transcription results)
```

## Error Handling Strategy

- Use `thiserror` for typed errors per module
- Errors bubble up with context via `anyhow` at boundaries
- User-facing errors get error codes (see error-codes.md)
- Log errors with `tracing` at appropriate levels

## Configuration Hierarchy

```
Defaults (compiled in)
    ↓ overridden by
Config file (~/.config/openhush/config.toml)
    ↓ overridden by
Environment variables (OPENHUSH_*)
    ↓ overridden by
CLI arguments
```
