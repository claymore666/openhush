#!/bin/bash
#
# OpenHush Waybar Status Module
#
# Shows current daemon status with icons:
#   󰍬 - Idle (ready)
#   󰍮 - Listening for wake word
#   󰑊 - Recording
#   󰔟 - Processing transcription
#
# Installation:
#   1. Copy this script to ~/.config/waybar/scripts/
#   2. Make executable: chmod +x waybar-openhush.sh
#   3. Add to Waybar config (see example below)
#
# Waybar config example (~/.config/waybar/config):
#
#   "custom/openhush": {
#       "exec": "~/.config/waybar/scripts/waybar-openhush.sh",
#       "return-type": "json",
#       "interval": 1,
#       "on-click": "openhush recording toggle"
#   }
#
# Waybar style example (~/.config/waybar/style.css):
#
#   #custom-openhush.recording {
#       color: #f38ba8;
#       animation: pulse 1s ease-in-out infinite;
#   }
#   #custom-openhush.listening {
#       color: #a6e3a1;
#   }
#   #custom-openhush.processing {
#       color: #f9e2af;
#   }
#   @keyframes pulse {
#       0%, 100% { opacity: 1; }
#       50% { opacity: 0.5; }
#   }

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

# Check if daemon is running
if ! busctl --user introspect "$DBUS_DEST" "$DBUS_PATH" &>/dev/null; then
    # Daemon not running
    echo '{"text": "'"$ICON_OFFLINE"'", "tooltip": "OpenHush: Not running", "class": "offline"}'
    exit 0
fi

# Get status from D-Bus
STATUS=$(busctl --user get-property "$DBUS_DEST" "$DBUS_PATH" "$DBUS_IFACE" "IsRecording" 2>/dev/null | awk '{print $2}')
QUEUE=$(busctl --user get-property "$DBUS_DEST" "$DBUS_PATH" "$DBUS_IFACE" "QueueDepth" 2>/dev/null | awk '{print $2}')

# Determine state and icon
if [[ "$STATUS" == "true" ]]; then
    ICON="$ICON_RECORDING"
    CLASS="recording"
    TOOLTIP="OpenHush: Recording..."
elif [[ "${QUEUE:-0}" -gt 0 ]]; then
    ICON="$ICON_PROCESSING"
    CLASS="processing"
    TOOLTIP="OpenHush: Processing ($QUEUE in queue)"
else
    # Check if wake word is enabled (via config file)
    CONFIG_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/openhush/config.toml"
    if [[ -f "$CONFIG_FILE" ]] && grep -q 'enabled.*=.*true' "$CONFIG_FILE" 2>/dev/null | grep -q 'wake_word' 2>/dev/null; then
        ICON="$ICON_LISTENING"
        CLASS="listening"
        TOOLTIP="OpenHush: Listening for wake word"
    else
        ICON="$ICON_IDLE"
        CLASS="idle"
        TOOLTIP="OpenHush: Ready (press hotkey to record)"
    fi
fi

# Output JSON for Waybar
echo "{\"text\": \"$ICON\", \"tooltip\": \"$TOOLTIP\", \"class\": \"$CLASS\"}"
