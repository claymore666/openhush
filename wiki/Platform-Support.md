# Platform Support Matrix

This document tracks feature availability across all supported platforms.

---

## Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Fully implemented |
| 🔶 | Partial / limited support |
| ❌ | Not implemented |
| 🚧 | In progress |
| N/A | Not applicable to this platform |

---

## Core Features

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| Voice transcription | ✅ | ✅ | ✅ | - |
| Whisper model inference | ✅ | ✅ | ✅ | - |
| File transcription | ✅ | ✅ | ✅ | - |
| Audio input (microphone) | ✅ | ✅ | ✅ | - |
| Clipboard copy | ✅ | ✅ | ✅ | - |
| Auto-paste text | ✅ | 🔶 | 🔶 | - |
| Hotkey trigger | ✅ | 🔶 | 🔶 | - |
| Configuration file | ✅ | ✅ | ✅ | - |
| CLI commands | ✅ | ✅ | ✅ | - |

---

## GPU Acceleration

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| CUDA (NVIDIA) | ✅ | N/A | ✅ | - |
| Metal (Apple Silicon) | N/A | ✅ | N/A | - |
| Vulkan | ✅ | ❌ | 🔶 | - |
| CPU fallback | ✅ | ✅ | ✅ | - |

---

## System Integration

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| System tray icon | ✅ | ✅ | ✅ | Closed |
| Tray menu | ✅ | ✅ | ✅ | Closed |
| Desktop notifications | ✅ | ✅ | ✅ | - |
| Audio feedback beeps | ✅ | ✅ | ✅ | - |

---

## Daemon / Background Service

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| Daemon mode | ✅ | ✅ | ✅ | - |
| D-Bus control | ✅ | N/A | N/A | - |
| IPC control (pipes/socket) | N/A | ❌ | ❌ | [#135](https://github.com/claymore666/openhush/issues/135) |
| Systemd service | ✅ | N/A | N/A | - |
| LaunchAgent | N/A | ❌ | N/A | [#133](https://github.com/claymore666/openhush/issues/133) |
| Windows Service | N/A | N/A | ❌ | [#132](https://github.com/claymore666/openhush/issues/132) |

---

## GUI

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| Preferences dialog | ✅ | ✅ | ✅ | Closed |
| Onboarding wizard | ❌ | ❌ | ❌ | [#76](https://github.com/claymore666/openhush/issues/76) |

---

## Security / Permissions

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| Microphone permission | Auto | ✅ | Auto | Closed |
| Accessibility permission | N/A | ✅ | N/A | Closed |
| AppArmor profile | ✅ | N/A | N/A | Closed |
| SELinux policy | ✅ | N/A | N/A | Closed |
| Firejail profile | ✅ | N/A | N/A | Closed |
| Sandbox detection | ✅ | N/A | N/A | Closed |
| Keyring integration | ❌ | ❌ | ❌ | [#96](https://github.com/claymore666/openhush/issues/96) |

---

## Packaging / Distribution

| Format | Linux | macOS | Windows | Issue |
|--------|-------|-------|---------|-------|
| Binary tarball | ✅ | ✅ | ✅ | - |
| .deb package | ✅ | N/A | N/A | Closed |
| Flatpak | ✅ | N/A | N/A | Closed |
| AUR (PKGBUILD) | ✅ | N/A | N/A | Closed |
| Homebrew formula | N/A | ✅ | N/A | Closed |
| DMG installer | N/A | ✅ | N/A | Closed |
| MSI installer | N/A | N/A | ✅ | Closed |

---

## Advanced Features

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| Streaming transcription | ✅ | ✅ | ✅ | - |
| Voice Activity Detection | ✅ | ✅ | ✅ | - |
| RNNoise denoising | ✅ | ✅ | ✅ | - |
| Custom vocabulary | ✅ | ✅ | ✅ | - |
| Filler word removal | ✅ | ✅ | ✅ | - |
| Text replacement | ✅ | ✅ | ✅ | - |
| Plugin system | ❌ | ❌ | ❌ | [#93](https://github.com/claymore666/openhush/issues/93) |
| Wake word detection | ❌ | ❌ | ❌ | [#63](https://github.com/claymore666/openhush/issues/63) |
| System audio capture | ❌ | ❌ | ❌ | [#61](https://github.com/claymore666/openhush/issues/61) |

---

## Linux-Specific Features

| Feature | X11 | Wayland | TTY | Issue |
|---------|-----|---------|-----|-------|
| Hotkey trigger | ✅ | ✅ | ✅ | - |
| Auto-paste (xdotool) | ✅ | N/A | N/A | - |
| Auto-paste (wtype) | N/A | ✅ | N/A | - |
| Auto-paste (TTY) | N/A | N/A | ✅ | - |
| System tray (D-Bus SNI) | ✅ | ✅ | N/A | - |
| D-Bus service mode | ✅ | ✅ | ✅ | - |
| Hyprland/Sway IPC | N/A | ❌ | N/A | [#78](https://github.com/claymore666/openhush/issues/78) |

---

## Priority Porting Tasks

### Completed in v0.6.0

1. ~~**System Tray for Windows/macOS**~~ ✅
   - Windows: `tray-icon` crate
   - macOS: `tray-icon` crate with menu bar integration

2. ~~**Preferences GUI for Windows/macOS**~~ ✅
   - Cross-platform egui implementation

3. ~~**macOS Accessibility Permission**~~ ✅
   - Uses `macos-accessibility-client` crate
   - Prompts user and guides to System Preferences

4. ~~**Security Sandboxing (Linux)**~~ ✅
   - AppArmor profile for Ubuntu/Debian/SUSE
   - SELinux policy for Fedora/RHEL
   - Firejail profile for any distro
   - Runtime sandbox detection

### Medium Priority (v0.7.0)

4. **IPC Control for Windows/macOS** ([#135](https://github.com/claymore666/openhush/issues/135))
   - D-Bus alternative using named pipes (Windows) or Unix sockets (macOS)
   - Enable `openhush status`, `openhush stop` commands

5. **Windows Service** ([#132](https://github.com/claymore666/openhush/issues/132))
   - Auto-start on login
   - Background operation without console window

6. **macOS LaunchAgent** ([#133](https://github.com/claymore666/openhush/issues/133))
   - Auto-start on login
   - Proper macOS service lifecycle

### Low Priority (Future)

7. **Keyring Integration** ([#96](https://github.com/claymore666/openhush/issues/96))
   - macOS Keychain, Windows Credential Manager, Linux Secret Service

---

## Implementation Notes

### System Tray

| Platform | Library | Status |
|----------|---------|--------|
| Linux | `ksni` (D-Bus StatusNotifierItem) | ✅ Implemented |
| macOS | `tray-icon` | ✅ Implemented |
| Windows | `tray-icon` | ✅ Implemented |

### GUI Toolkit

| Platform | Library | Status |
|----------|---------|--------|
| Linux | `egui` + `eframe` | ✅ Implemented |
| macOS | `egui` + `eframe` | ✅ Implemented |
| Windows | `egui` + `eframe` | ✅ Implemented |

### Security Sandboxing

| Platform | Profiles | Status |
|----------|----------|--------|
| Linux | AppArmor, SELinux, Firejail | ✅ Implemented |
| macOS | App Sandbox (future) | ❌ Not implemented |
| Windows | N/A | N/A |

### Hotkey Handling

| Platform | Library | Notes |
|----------|---------|-------|
| Linux | `evdev` (raw input) | Works in X11, Wayland, TTY |
| macOS | `rdev` or Core Graphics | Requires accessibility permission |
| Windows | `rdev` or WinAPI | Works system-wide |

---

## Testing Matrix

To ensure cross-platform compatibility, test on:

| Platform | Version | Architecture |
|----------|---------|--------------|
| Ubuntu | 22.04, 24.04 | x86_64 |
| Fedora | 40+ | x86_64 |
| Arch Linux | Rolling | x86_64 |
| macOS | 13+ (Ventura) | x86_64, aarch64 |
| Windows | 10, 11 | x86_64 |

---

## Contributing

To port a feature to a new platform:

1. Check the relevant issue for context
2. Implement in `src/platform/{platform}/` module
3. Update the `CurrentPlatform` type alias
4. Add platform-specific tests
5. Update this matrix document

See [Architecture](Architecture.md) for the platform abstraction design.
