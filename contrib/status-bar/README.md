# Status Bar Integration Scripts

Scripts for showing OpenHush status in Linux status bars.

## Scripts

| Script | Status Bar | Description |
|--------|------------|-------------|
| `waybar-openhush.sh` | Waybar | JSON output with CSS classes |
| `polybar-openhush.sh` | Polybar | Text output with optional colors |

## Icons (Nerd Font required)

| Icon | State |
|------|-------|
| 󰍬 | Idle (ready to record) |
| 󰍮 | Listening for wake word |
| 󰑊 | Recording |
| 󰔟 | Processing transcription |
| 󰍭 | Daemon not running |

## Installation

See the [User Guide](../../wiki/User-Guide.md#status-bar-integration-waybarpolybar) for configuration examples.

## Requirements

- OpenHush daemon running (`openhush start`)
- D-Bus session bus
- `busctl` command (part of systemd)
- Nerd Font for icons
