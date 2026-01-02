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
| Auto-paste text | ✅ | ✅ | ✅ | - |
| Hotkey trigger | ✅ | ✅ | ✅ | - |
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
| IPC control (pipes/socket) | N/A | ✅ | ✅ | Closed |
| Autostart (service install) | ✅ | ✅ | ✅ | Closed |

---

## GUI

| Feature | Linux | macOS | Windows | Issue |
|---------|-------|-------|---------|-------|
| Preferences dialog | ✅ | ✅ | ✅ | Closed |
| Onboarding wizard | ✅ | ✅ | ✅ | Closed |

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
| Keyring integration | ✅ | ✅ | ✅ | Closed |

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
| Post-transcription actions | ✅ | ✅ | ✅ | Closed |
| App-aware profiles | ✅ | ✅ | ✅ | Closed |
| Plugin system | ❌ | ❌ | ❌ | [#93](https://github.com/claymore666/openhush/issues/93) |
| Wake word detection | ✅ | ✅ | ✅ | Closed |
| System audio capture | ✅ | ✅ | ❌ | Closed |

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
| Hyprland/Sway IPC | ✅ | N/A | N/A | Closed |
| Waybar/Polybar scripts | ✅ | N/A | N/A | Closed |

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

5. ~~**IPC Control for Windows/macOS**~~ ✅
   - Unix sockets for macOS, named pipes for Windows
   - `openhush status` and `openhush stop` now work on all platforms

6. ~~**Autostart Service**~~ ✅
   - Linux: systemd user service
   - macOS: LaunchAgent
   - Windows: Registry Run key
   - `openhush service install/uninstall/status` commands

7. ~~**Keyring Integration**~~ ✅
   - macOS Keychain, Windows Credential Manager, Linux Secret Service

8. ~~**Wake Word Detection**~~ ✅
   - openWakeWord ONNX models for hands-free activation

9. ~~**Hyprland/Sway IPC**~~ ✅
   - Native compositor integration for status updates

10. ~~**App-Aware Profiles**~~ ✅
    - Per-application configuration switching

11. ~~**Post-Transcription Actions**~~ ✅
    - Shell commands, HTTP requests, file logging

12. ~~**Onboarding Wizard**~~ ✅
    - First-run setup with microphone test, model download, hotkey config

### Low Priority (Future)

13. **Plugin System** ([#93](https://github.com/claymore666/openhush/issues/93))
   - Extensible architecture for community extensions

14. ~~**System Audio Capture**~~ ✅
   - PulseAudio/PipeWire monitor sources for meeting transcription (Linux only)

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
| macOS | 13+ (Ventura), 14 (Sonoma) | x86_64, aarch64 |
| Windows | 10, 11 | x86_64 |

---

## macOS VM Testing (OSX-KVM)

For developers without physical macOS hardware, a KVM-based macOS VM can be used for testing.

### Requirements

- Linux host with KVM support
- AMD or Intel CPU with virtualization (VT-x/AMD-V)
- 16GB+ RAM (VM uses 16GB)
- IOMMU enabled for USB passthrough

### Quick Setup

```bash
# Clone OSX-KVM
git clone --depth 1 https://github.com/kholia/OSX-KVM.git ~/OSX-KVM

# Download macOS Sonoma
cd ~/OSX-KVM
python3 fetch-macOS-v2.py -s sonoma --action download
dmg2img -i com.apple.recovery.boot/BaseSystem.dmg BaseSystem.img

# Create virtual disk
qemu-img create -f qcow2 mac_hdd_ng.img 128G

# Enable KVM parameter
echo 1 | sudo tee /sys/module/kvm/parameters/ignore_msrs
```

### macOS Permissions

OpenHush requires two TCC permissions on macOS:

| Permission | Purpose | How to Grant |
|------------|---------|--------------|
| Microphone | Audio capture | System Settings → Privacy → Microphone |
| Accessibility | Hotkey detection, text paste | System Settings → Privacy → Accessibility |

**Manual TCC database modification** (for automated setup):

```bash
# Grant microphone permission
sudo sqlite3 "/Library/Application Support/com.apple.TCC/TCC.db" \
  "INSERT OR REPLACE INTO access (service, client, client_type, auth_value, auth_reason, auth_version) \
   VALUES ('kTCCServiceMicrophone', '/path/to/openhush', 1, 2, 0, 1);"

# Grant accessibility permission
sudo sqlite3 "/Library/Application Support/com.apple.TCC/TCC.db" \
  "INSERT OR REPLACE INTO access (service, client, client_type, auth_value, auth_reason, auth_version) \
   VALUES ('kTCCServiceAccessibility', '/path/to/openhush', 1, 2, 0, 1);"
```

### USB Audio Passthrough

For microphone testing in VM, pass through a USB audio device:

```bash
# Find USB device
lsusb | grep -i audio

# Add to QEMU command
-device usb-host,vendorid=0x0b0e,productid=0x0e36

# Grant permissions on host
sudo chmod 666 /dev/bus/usb/XXX/YYY
```

### Verified Working

Tested configuration (January 2026):
- macOS Sonoma 14.x in QEMU/KVM
- Skylake-Client-v4 CPU emulation
- vmware-svga display adapter
- USB passthrough for Jabra headset
- All OpenHush features functional

---

## Contributing

To port a feature to a new platform:

1. Check the relevant issue for context
2. Implement in `src/platform/{platform}/` module
3. Update the `CurrentPlatform` type alias
4. Add platform-specific tests
5. Update this matrix document

See [Architecture](Architecture.md) for the platform abstraction design.
