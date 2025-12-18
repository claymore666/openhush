# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OpenHush is an open-source, cross-platform voice-to-text tool that acts as a seamless whisper keyboard. Press a hotkey, speak, release — your words appear where your cursor is. Runs as a background daemon with local GPU transcription (CUDA) and optional LLM correction via Ollama.

## Build Commands

```bash
# Development
cargo build

# Release
cargo build --release

# With CUDA support
cargo build --release --features cuda

# Run tests, linting, formatting
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

## Architecture

```
src/
├── main.rs              # Entry point, CLI (clap)
├── config.rs            # TOML config handling + XDG paths
├── daemon.rs            # Background service, recording queue, result ordering
├── platform/
│   ├── mod.rs           # Traits: HotkeyHandler, TextOutput, Notifier, AudioFeedback
│   ├── linux.rs         # X11, Wayland (wtype), TTY (evdev)
│   ├── macos.rs         # CGEvent paste (stub)
│   └── windows.rs       # SendInput paste (stub)
├── input/               # (TODO)
│   ├── hotkey.rs        # Global hotkey via rdev
│   ├── audio.rs         # Microphone capture via cpal (16kHz mono)
│   └── evdev.rs         # Linux TTY hotkey fallback
├── queue/               # (TODO)
│   ├── recording.rs     # Recording queue with sequence IDs
│   └── result.rs        # Ordered result aggregation
├── gpu/                 # (TODO)
│   ├── pool.rs          # Multi-GPU enumeration and model preloading
│   ├── worker.rs        # Per-GPU transcription worker
│   └── scheduler.rs     # Job assignment
├── engine/              # (TODO)
│   ├── whisper.rs       # whisper-rs wrapper
│   └── vad.rs           # Voice activity detection
├── correction/          # (TODO)
│   └── ollama.rs        # Optional LLM correction via Ollama API
└── output/              # (TODO)
    ├── clipboard.rs     # Cross-platform clipboard (arboard)
    └── paste.rs         # Platform dispatcher
```

## Key Design Patterns

- **Queued dictation**: Recordings get sequence IDs, results reordered before output
- **Platform abstraction**: Traits in `platform/mod.rs`, implementations per OS
- **Multi-GPU**: GPU pool preloads models, scheduler assigns jobs round-robin
- **Async + threads**: Tokio for I/O, dedicated threads for GPU compute

## Configuration

Config file: `~/.config/openhush/config.toml` (Linux), platform-appropriate elsewhere.

Key sections: `[hotkey]`, `[transcription]`, `[output]`, `[correction]`, `[feedback]`, `[queue]`, `[gpu]`

## CLI Commands

```bash
openhush start [-f]           # Start daemon (foreground with -f)
openhush stop                 # Stop daemon
openhush status               # Check if running
openhush config --show        # Show current config
openhush config --hotkey X    # Set hotkey
openhush model download NAME  # Download Whisper model
openhush transcribe FILE      # One-shot transcription
```

## Development Process

- **Milestones**: GitHub Milestones for feature groups
- **Issues**: Use requirement.yml template for new features
- **CI**: GitHub Actions runs on all PRs (build, test, clippy, fmt)
- **Agents**: Claude Code commands in `.claude/commands/` for PR review

## Platform-Specific Notes

| Platform | Paste | Notes |
|----------|-------|-------|
| Linux X11 | xdotool | Requires `xdotool` installed |
| Linux Wayland | wtype | Requires `wtype` installed |
| Linux TTY | stdout | No paste, prints to terminal |
| macOS | enigo | CGEvent-based |
| Windows | enigo | SendInput-based |

## Claude Code Notes

- **No sudo access**: Claude Code cannot run `sudo` commands. When system packages are needed (e.g., `apt install`, `dnf install`), tell the user which commands to run.
- **Rust not installed**: If `cargo` is not found, tell the user to install Rust via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
