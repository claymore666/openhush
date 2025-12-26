# Firejail profile for OpenHush
# Install: sudo cp profiles/firejail/openhush.profile /etc/firejail/
# Usage:   firejail openhush start

# Metadata
quiet
include /etc/firejail/disable-common.inc
include /etc/firejail/disable-devel.inc
include /etc/firejail/disable-exec.inc
include /etc/firejail/disable-interpreters.inc
include /etc/firejail/disable-programs.inc
include /etc/firejail/disable-shell.inc
include /etc/firejail/disable-xdg.inc

# ============================================
# Whitelist - allowed paths
# ============================================

# Configuration
whitelist ${HOME}/.config/openhush
mkdir ${HOME}/.config/openhush
noblacklist ${HOME}/.config/openhush

# Data directory (models, logs)
whitelist ${HOME}/.local/share/openhush
mkdir ${HOME}/.local/share/openhush
noblacklist ${HOME}/.local/share/openhush

# Models are read-only after download
read-only ${HOME}/.local/share/openhush/models

# PulseAudio/PipeWire config
whitelist ${HOME}/.config/pulse
read-only ${HOME}/.config/pulse

# ============================================
# Audio access
# ============================================

# ALSA
whitelist /dev/snd
whitelist /proc/asound

# PulseAudio runtime
whitelist ${RUNUSER}/pulse

# PipeWire runtime
whitelist ${RUNUSER}/pipewire-0
whitelist ${RUNUSER}/pipewire-0-manager

# ============================================
# GPU access (CUDA/ROCm/Vulkan)
# ============================================

# NVIDIA
whitelist /dev/nvidia0
whitelist /dev/nvidia1
whitelist /dev/nvidia2
whitelist /dev/nvidia3
whitelist /dev/nvidiactl
whitelist /dev/nvidia-modeset
whitelist /dev/nvidia-uvm
whitelist /dev/nvidia-uvm-tools
noblacklist /proc/driver/nvidia
whitelist /usr/share/nvidia
whitelist /etc/nvidia

# AMD ROCm
whitelist /dev/kfd
whitelist /dev/dri
whitelist /opt/rocm
read-only /opt/rocm

# Vulkan
whitelist /etc/vulkan
whitelist /usr/share/vulkan

# Mesa/OpenGL
whitelist /usr/share/glvnd
whitelist /usr/share/drirc.d

# ============================================
# D-Bus (notifications, tray)
# ============================================

dbus-user filter
dbus-user.own org.openhush.*
dbus-user.talk org.freedesktop.Notifications
dbus-user.talk org.freedesktop.portal.*
dbus-user.talk org.kde.StatusNotifierWatcher
dbus-user.talk org.kde.StatusNotifierItem-*
dbus-system none

# ============================================
# X11/Wayland (clipboard, paste)
# ============================================

# Allow X11 or Wayland
# (Firejail auto-detects display server)

# xdotool/wtype for paste
whitelist /usr/bin/xdotool
whitelist /usr/bin/wtype

# ============================================
# Network
# ============================================

# Allow localhost only (for Ollama API)
# Note: Use --net=none to fully disable if Ollama not used
netfilter
protocol unix,inet,inet6

# ============================================
# Security hardening
# ============================================

# Capabilities
caps.drop all
caps.keep sys_nice

# Seccomp
seccomp
seccomp.block-secondary

# Namespaces
noroot
ipc-namespace
# no new-privs already implied

# Other hardening
nonewprivs
nogroups
nosound  # We need sound, but this is input only - see dev whitelist above
nou2f
notv
novideo  # We only need audio input

# Disable shell execution within sandbox
shell none

# Machine ID (needed for D-Bus)
machine-id

# ============================================
# Memory/resource limits (optional)
# ============================================

# Uncomment to set limits
# rlimit-as 8000000000
# rlimit-cpu 3600
# rlimit-fsize 1000000000
# rlimit-nproc 100
