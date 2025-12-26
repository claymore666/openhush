# Changelog

All notable changes to OpenHush are documented here.

## [Unreleased]

---

## [0.6.0] - 2025-12-26

### Cross-Platform Parity

This release completes full cross-platform support with system integration features.

**What's New:**

- **System Tray for Windows/macOS** — Native tray icons with status indicator and menu
- **Autostart Service** — `openhush service install` enables autostart on login
  - Linux: systemd user service
  - macOS: LaunchAgent
  - Windows: Registry Run key
- **IPC Control** — Daemon control via platform-native IPC
  - Linux: D-Bus (existing)
  - macOS: Unix domain sockets
  - Windows: Named pipes
- **Preferences GUI for All Platforms** — egui-based settings window on Linux, macOS, and Windows

**Under the Hood:**

- Modular service management (`src/service/`)
- Cross-platform IPC abstraction (`src/ipc/`)
- Direct FFI bindings for Windows named pipes (no external dependencies)

---

## [0.5.0] - 2025-12-26

### Packaging & Distribution

This release makes OpenHush easy to install on any platform.

**What's New:**

- **D-Bus Service Mode** — Control the daemon via D-Bus (`openhush status`, `openhush stop`)
- **Multi-GPU Support** — AMD (HIP), Apple Metal, and Vulkan backends
- **Flatpak Package** — Install from Flathub (coming soon)
- **AUR Package** — Install on Arch Linux with `yay -S openhush`
- **Debian Package** — Install on Ubuntu/Debian with `.deb`
- **Homebrew Formula** — Install on macOS with `brew install openhush`
- **DMG Installer** — Drag-and-drop installation for macOS
- **MSI Installer** — Windows installer with Start Menu integration

**Documentation:**

- Comprehensive [Wiki](https://github.com/claymore666/openhush/wiki) with architecture docs
- Platform support matrix tracking feature availability
- Market analysis and product vision roadmap

**CI/CD:**

- Automated package builds for all platforms on release
- Checksums for all release artifacts

---

## [0.4.0] - 2025-12-24

### Cross-Platform Support

This release brings OpenHush to macOS and Windows, plus visual improvements.

**What's New:**

- **Dark Mode** — The preferences window now follows your system's light/dark theme, or you can choose manually
- **macOS Support** — OpenHush now runs on Mac (Intel and Apple Silicon)
- **Windows Support** — OpenHush now runs on Windows 10/11
- **Automated Builds** — Pre-built binaries for Linux, macOS, and Windows in every release

**Under the Hood:**

- Migrated system tray from GTK to lightweight D-Bus (smaller binary, fewer dependencies)
- Platform abstraction layer for cleaner cross-platform code
- Release automation with cross-compilation for 6 targets

---

## [0.3.0] - 2025-12-24

### Transcription Quality

This release dramatically improves transcription accuracy and adds smart text processing.

**What's New:**

- **Continuous Dictation** — Keep talking without holding the hotkey; OpenHush detects when you pause and transcribes automatically (using Silero voice detection)
- **Background Noise Removal** — RNNoise AI removes keyboard clicks, fans, and ambient noise before transcription
- **Custom Vocabulary** — Add names, technical terms, or brand names that Whisper often gets wrong
- **Filler Word Cleanup** — Automatically removes "um", "uh", "like", "you know" from your transcription (requires Ollama)
- **Text Snippets** — Type abbreviations that expand to full text (e.g., "addr" → your full address)
- **Quality Presets** — Choose between Instant (fast), Balanced, or Quality (accurate) modes
- **Better Audio Quality** — High-quality resampling for clearer input to Whisper

**Model Management:**

- `openhush model download <name>` — Download models easily
- `openhush model list` — See available and installed models
- `openhush model remove <name>` — Clean up unused models

---

## [0.2.0] - 2025-12-20

### Stability & Production Hardening

This release focuses on reliability and crash recovery.

**What's New:**

- **GPU Auto-Tune** — OpenHush benchmarks your GPU at startup to find the optimal transcription speed
- **Crash Recovery** — If something goes wrong, detailed crash reports are saved to help diagnose issues
- **Smarter Streaming** — Long recordings are split intelligently with no repeated words at chunk boundaries
- **Graceful Degradation** — If your microphone disconnects, OpenHush handles it gracefully instead of crashing

**Reliability Improvements:**

- Configuration validation catches errors at startup, not during use
- Better handling of multiple recordings in quick succession
- Safer process management (no more orphaned daemons)

---

## [0.1.0] - 2025-12-20

### MVP Release

The first public release of OpenHush.

**Core Features:**

- **Push-to-Talk** — Hold Right Ctrl (or your chosen key), speak, release. Your words appear at your cursor.
- **100% Local** — All processing happens on your machine. No cloud, no API keys, no data leaving your computer.
- **GPU Accelerated** — Uses your NVIDIA GPU for fast transcription. Works on CPU too (just slower).
- **Auto-Paste** — Transcribed text is automatically typed where your cursor is.
- **Translation Mode** — Speak in any language and get English text output.
- **System Tray** — Runs quietly in the background with a tray icon.
- **Preferences GUI** — Easy point-and-click configuration.

**Platform Support:**

- Linux with X11 (Ubuntu, Fedora, etc.)
- Linux with Wayland (KDE Plasma, GNOME, Sway)
- Linux TTY (terminal without graphical desktop)

---

## Version Naming

OpenHush follows [Semantic Versioning](https://semver.org/):

- **Major** (1.0, 2.0) — Big changes, may require reconfiguration
- **Minor** (0.1, 0.2, 0.3) — New features, backwards compatible
- **Patch** (0.1.1, 0.1.2) — Bug fixes only
