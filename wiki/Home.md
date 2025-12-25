# OpenHush Wiki

> Open-source voice-to-text that acts as a seamless whisper keyboard.

**Current Version:** v0.5.0
**License:** MIT
**Status:** Active Development

---

## Quick Navigation

| Documentation | Description |
|--------------|-------------|
| [Architecture](Architecture) | System overview, data flow, tech stack |
| [Components](Components) | Deep dive into each module |
| [Product Vision](Product-Vision) | Evolution roadmap and future direction |
| [Market Analysis](Market-Analysis) | Target audiences, peer groups, competitive positioning |
| [Plugin System](Plugin-System) | Future plugin architecture design |
| [User Guide](User-Guide) | Installation, configuration, CLI reference |

---

## What is OpenHush?

OpenHush is a **privacy-first voice-to-text tool** that runs entirely on your local machine. No cloud, no subscriptions, no telemetry. Just fast, accurate transcription using OpenAI's Whisper model.

### Key Features

- **Push-to-Talk & Toggle Modes** - Hold a hotkey or tap to toggle recording
- **Auto-Paste** - Text appears at your cursor instantly
- **GPU Acceleration** - CUDA, AMD ROCm, Apple Metal, Vulkan
- **Cross-Platform** - Linux (X11/Wayland/TTY), macOS, Windows
- **File Transcription** - Ingest WAV/MP3 recordings
- **LLM Correction** - Optional grammar/filler cleanup via Ollama
- **Voice Activity Detection** - Continuous dictation mode

### Design Principles

1. **Privacy-First** - All processing happens locally
2. **Zero Configuration** - Works out of the box with sensible defaults
3. **Performance** - Native Rust, GPU-accelerated, async architecture
4. **Extensible** - Plugin system (coming soon) for community extensions

---

## Resources

- [README](../README.md) - Quick start guide
- [Changelog](../CHANGELOG.md) - Version history
- [Requirements](../REQUIREMENTS.md) - Functional specifications
- [Competitors](../COMPETITORS.md) - Competitive analysis
- [GitHub Repository](https://github.com/claymore666/openhush)

---

## Getting Started

```bash
# Start the daemon
openhush start

# Check status
openhush status

# Configure hotkey
openhush config hotkey ControlRight

# Transcribe a file
openhush transcribe recording.wav
```

For detailed instructions, see the [User Guide](User-Guide).
