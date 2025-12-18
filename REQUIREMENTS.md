# OpenHush - Requirements Specification

## Project Vision

OpenHush is an open-source voice-to-text tool that acts as a **seamless whisper keyboard**. Press a hotkey, speak, release, and your words appear where your cursor is - powered by local AI, no cloud required.

## Functional Requirements

### Core Features

| ID | Requirement | Priority | Description |
|----|-------------|----------|-------------|
| F01 | Background daemon | Must | Tool loads and stays resident in memory |
| F02 | Hotkey trigger | Must | Global hotkey to start/stop recording |
| F03 | Push-to-talk | Must | Record while key held, stop on release |
| F04 | Local transcription | Must | Transcribe using local Whisper model |
| F05 | GPU acceleration | Must | CUDA support for NVIDIA GPUs |
| F06 | CPU fallback | Must | Works without GPU (slower) |
| F07 | Clipboard output | Must | Copy transcribed text to clipboard |
| F08 | Auto-paste | Must | Type text where cursor is located |
| F09 | LLM correction | Should | Optional post-processing via local LLM |
| F10 | Model selection | Should | Choose Whisper model size (tiny→large) |
| F11 | Language selection | Could | Specify transcription language |
| F12 | Custom vocabulary | Could | Add domain-specific terms |
| F13 | Queued dictation | Must | Re-pressing hotkey queues new recording; transcriptions paste in order |
| F14 | Multi-GPU | Should | Distribute transcriptions across multiple GPUs on single machine |
| F15 | Platform abstraction | Must | XDG-compliant paths, platform-specific modules for paste/hotkey |

### Platform Support

| ID | Requirement | Priority | Description |
|----|-------------|----------|-------------|
| P01 | Linux X11 | Must | Full support including paste |
| P02 | Linux Wayland | Must | Full support via wtype |
| P03 | Linux TTY | Should | Headless/terminal mode via evdev |
| P04 | macOS | Should | Native support |
| P05 | Windows | Should | Native support |
| P06 | KDE Plasma | Must | Native Wayland integration |
| P07 | GNOME | Should | Wayland support |
| P08 | Android | Could | Play Store app (post-v1.0, floating button trigger) |

### Configuration

| ID | Requirement | Priority | Description |
|----|-------------|----------|-------------|
| C01 | Config file | Must | TOML configuration file |
| C02 | Hotkey config | Must | Configurable trigger key |
| C03 | Model config | Must | Select Whisper model |
| C04 | LLM config | Should | Configure Ollama endpoint/model |
| C05 | Output mode | Should | Clipboard only, paste only, or both |
| C06 | Audio device | Could | Select input microphone |
| C07 | Feedback config | Should | Configurable audio beep and/or visual notification |
| C08 | Queue limit | Should | Configurable max pending recordings (0 = unlimited) |
| C09 | Queue separator | Should | Configurable separator between transcriptions |
| C10 | GPU selection | Should | Auto-detect or specify GPU devices |

### Distribution

| ID | Requirement | Priority | Description |
|----|-------------|----------|-------------|
| D01 | apt package | Should | .deb package with repository |
| D02 | rpm package | Should | .rpm package with repository |
| D03 | AUR package | Should | Arch Linux PKGBUILD |
| D04 | Homebrew | Should | macOS Homebrew formula |

## Non-Functional Requirements

### Performance

| ID | Requirement | Target |
|----|-------------|--------|
| NF01 | Transcription latency | < 3s for 10s audio (GPU) |
| NF02 | Transcription latency | < 20s for 10s audio (CPU) |
| NF03 | Startup time | < 500ms (model preloaded) |
| NF04 | Memory usage | < 8GB VRAM (large-v3) |
| NF05 | Idle CPU | < 1% when waiting |

### Usability

| ID | Requirement | Description |
|----|-------------|-------------|
| NF06 | Single binary | One executable per platform |
| NF07 | Minimal dependencies | No runtime dependencies (static link where possible) |
| NF08 | Simple setup | Works out of the box with defaults |
| NF09 | Clear feedback | Visual/audio indication of recording state |

### Security & Privacy

| ID | Requirement | Description |
|----|-------------|-------------|
| NF10 | Local processing | All transcription happens locally |
| NF11 | No telemetry | No data sent to external servers |
| NF12 | No audio storage | Audio deleted after transcription |

## Technical Specifications

### Transcription Backend

| Option | Library | Speed | Accuracy | Notes |
|--------|---------|-------|----------|-------|
| Primary | whisper-rs | 4x | High | Rust bindings to whisper.cpp |
| Alternative | faster-whisper | 4-8x | High | Python, CTranslate2 |

### Whisper Models

| Model | Size | VRAM | Speed (10s audio) | Accuracy |
|-------|------|------|-------------------|----------|
| tiny | 75MB | ~1GB | ~0.3s | Low |
| base | 142MB | ~1GB | ~0.5s | Medium |
| small | 466MB | ~2GB | ~0.8s | Good |
| medium | 1.5GB | ~4GB | ~1.5s | Very Good |
| large-v3 | 3GB | ~6GB | ~3s | Best |

### LLM Correction (Optional)

| Model | Provider | Purpose |
|-------|----------|---------|
| llama3.2:3b | Ollama | Fast grammar/punctuation fix |
| llama3.2:7b | Ollama | Better correction quality |
| Custom | Ollama | User-specified model |

### Audio Specifications

| Parameter | Value |
|-----------|-------|
| Sample rate | 16000 Hz |
| Channels | Mono |
| Format | 16-bit PCM |
| Min duration | 0.5s |
| Max duration | 30s (configurable) |

## User Stories

### US01: Basic Transcription
> As a user, I want to hold a hotkey, speak, and have my words typed where my cursor is, so I can write faster than typing.

### US02: Quick Notes
> As a user, I want to quickly capture thoughts without switching applications, so I can stay focused on my work.

### US03: Code Comments
> As a developer, I want to dictate code comments and documentation, so I can document code hands-free.

### US04: Terminal Usage
> As a sysadmin, I want to use voice input in a TTY without X server, so I can work on headless servers.

### US05: Grammar Correction
> As a user, I want optional LLM post-processing to fix grammar and punctuation, so my text is polished.

### US06: Cross-Platform
> As a user who works on multiple operating systems, I want the same tool to work everywhere, so I have a consistent experience.

### US07: Continuous Dictation
> As a user, I want to keep speaking in bursts without waiting for each transcription to finish, so I can dictate naturally and have all text appear in order.

## Acceptance Criteria

### MVP (Minimum Viable Product)

- [ ] Daemon starts and stays running
- [ ] Configurable hotkey triggers recording
- [ ] Push-to-talk: hold to record, release to transcribe
- [ ] Whisper transcription with GPU (CUDA)
- [ ] CPU fallback when no GPU
- [ ] Text copied to clipboard
- [ ] Text pasted at cursor (Linux X11/Wayland)
- [ ] Queued dictation (re-press hotkey to queue, paste in order)
- [ ] TOML config file
- [ ] CLI for configuration

### Version 1.0

- [ ] All MVP features
- [ ] macOS support
- [ ] Windows support
- [ ] LLM correction via Ollama
- [ ] Model download helper
- [ ] Systemd service file
- [ ] Desktop notifications

### Future Considerations

- [ ] Android app (Play Store)
- [ ] GUI settings panel (egui or Qt)
- [ ] System tray icon
- [ ] Multiple language support
- [ ] Custom wake word
- [ ] Streaming transcription
- [ ] Plugin system

## Competitor Analysis

| Feature | OpenHush | HyperWhisper | SuperWhisper | Whisper Keys |
|---------|:--------:|:------------:|:------------:|:------------:|
| Open source | ✅ | ❌ | ❌ | ❌ |
| Price | Free | $39 | $? | $59.99/yr |
| Linux | ✅ | ❌ | ❌ | ❌ |
| macOS | ✅ | ✅ | ✅ | ✅ |
| Windows | ✅ | ❌ | ❌ | ❌ |
| Local GPU | ✅ | ✅ | ✅ | ✅ |
| LLM correction | ✅ | ❌ | ❌ | ✅ |
| Terminal mode | ✅ | ❌ | ❌ | ❌ |
| Wayland | ✅ | ❌ | ❌ | ❌ |

## References

- [whisper.cpp](https://github.com/ggerganov/whisper.cpp)
- [whisper-rs](https://github.com/tazz4843/whisper-rs)
- [faster-whisper](https://github.com/SYSTRAN/faster-whisper)
- [Ollama](https://ollama.com/)
