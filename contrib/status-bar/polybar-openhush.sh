#!/bin/bash
#
# OpenHush Polybar Status Module
#
# Shows current daemon status with icons and colors:
#   󰍬 - Idle (ready)
#   󰍮 - Listening for wake word
#   󰑊 - Recording
#   󰔟 - Processing transcription
#
# Installation:
#   1. Copy this script to ~/.config/polybar/scripts/
#   2. Make executable: chmod +x polybar-openhush.sh
#   3. Add to Polybar config (see example below)
#
# Polybar config example (~/.config/polybar/config.ini):
#
#   [module/openhush]
#   type = custom/script
#   exec = ~/.config/polybar/scripts/polybar-openhush.sh
#   interval = 1
#   click-left = openhush recording toggle
#
# With colors (requires format-foreground support):
#
#   [module/openhush]
#   type = custom/script
#   exec = ~/.config/polybar/scripts/polybar-openhush.sh --polybar-colors
#   interval = 1
#   click-left = openhush recording toggle

set -euo pipefail

# D-Bus destination and interface
DBUS_DEST="org.openhush.Daemon1"
DBUS_PATH="/org/openhush/Daemon1"
DBUS_IFACE="org.openhush.Daemon1"

# Icons (Nerd Font)
ICON_IDLE="󰍬"
ICON_LISTENING="󰍮"
ICON_RECORDING="󰑊"
ICON_PROCESSING="󰔟"
ICON_OFFLINE="󰍭"

# Colors (Catppuccin Mocha)
COLOR_IDLE="#cdd6f4"      # Text
COLOR_LISTENING="#a6e3a1" # Green
COLOR_RECORDING="#f38ba8" # Red
COLOR_PROCESSING="#f9e2af" # Yellow
COLOR_OFFLINE="#6c7086"   # Overlay0

# Check for color flag
USE_COLORS=false
if [[ "${1:-}" == "--polybar-colors" ]]; then
    USE_COLORS=true
fi

# Function to output with optional Polybar color formatting
output() {
    local icon="$1"
    local color="$2"

    if $USE_COLORS; then
        echo "%{F$color}$icon%{F-}"
    else
        echo "$icon"
    fi
}

# Check if daemon is running
if ! busctl --user introspect "$DBUS_DEST" "$DBUS_PATH" &>/dev/null; then
    output "$ICON_OFFLINE" "$COLOR_OFFLINE"
    exit 0
fi

# Get status from D-Bus
STATUS=$(busctl --user get-property "$DBUS_DEST" "$DBUS_PATH" "$DBUS_IFACE" "IsRecording" 2>/dev/null | awk '{print $2}')
QUEUE=$(busctl --user get-property "$DBUS_DEST" "$DBUS_PATH" "$DBUS_IFACE" "QueueDepth" 2>/dev/null | awk '{print $2}')

# Determine state and output
if [[ "$STATUS" == "true" ]]; then
    output "$ICON_RECORDING" "$COLOR_RECORDING"
elif [[ "${QUEUE:-0}" -gt 0 ]]; then
    output "$ICON_PROCESSING" "$COLOR_PROCESSING"
else
    # Check if wake word is enabled
    CONFIG_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/openhush/config.toml"
    if [[ -f "$CONFIG_FILE" ]] && grep -qE '^\s*\[wake_word\]' "$CONFIG_FILE" && \
       sed -n '/^\s*\[wake_word\]/,/^\s*\[/p' "$CONFIG_FILE" | grep -qE '^\s*enabled\s*=\s*true'; then
        output "$ICON_LISTENING" "$COLOR_LISTENING"
    else
        output "$ICON_IDLE" "$COLOR_IDLE"
    fi
fi
