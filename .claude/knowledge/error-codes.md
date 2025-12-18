# Error Codes

User-facing error codes for documentation and support.

## Format

`OH-XXXX` where:
- `OH` = OpenHush prefix
- First digit = category
- Last 3 digits = specific error

## Categories

| Range | Category |
|-------|----------|
| 1xxx | Configuration |
| 2xxx | Audio/Input |
| 3xxx | Transcription/Engine |
| 4xxx | Output/Paste |
| 5xxx | Platform |
| 6xxx | Network/LLM |
| 9xxx | Internal |

---

## Configuration Errors (1xxx)

### OH-1001: Config file not found
**Message**: Configuration file not found at {path}
**Cause**: First run or file deleted
**Fix**: Run `openhush config --show` to create default config

### OH-1002: Config parse error
**Message**: Failed to parse config: {details}
**Cause**: Invalid TOML syntax
**Fix**: Check config file syntax, compare with example

### OH-1003: Invalid config value
**Message**: Invalid value for {field}: {value}
**Cause**: Value out of range or wrong type
**Fix**: See documentation for valid values

### OH-1004: Config directory not writable
**Message**: Cannot write to config directory: {path}
**Cause**: Permission denied
**Fix**: Check directory permissions

---

## Audio/Input Errors (2xxx)

### OH-2001: No audio device
**Message**: No audio input device found
**Cause**: No microphone connected or permission denied
**Fix**: Connect microphone, check system permissions

### OH-2002: Audio capture failed
**Message**: Failed to capture audio: {details}
**Cause**: Device busy or disconnected
**Fix**: Close other apps using microphone, reconnect device

### OH-2003: Recording too short
**Message**: Recording too short ({duration}s < 0.5s)
**Cause**: Hotkey released too quickly
**Fix**: Hold hotkey longer while speaking

### OH-2004: Recording too long
**Message**: Recording exceeded maximum duration ({max}s)
**Cause**: Hotkey held too long
**Fix**: Release hotkey, increase max_duration in config

### OH-2005: Hotkey conflict
**Message**: Hotkey {key} is already in use
**Cause**: Another app using the same hotkey
**Fix**: Change hotkey in config

---

## Transcription Errors (3xxx)

### OH-3001: Model not found
**Message**: Whisper model not found: {model}
**Cause**: Model not downloaded
**Fix**: Run `openhush model download {model}`

### OH-3002: Model load failed
**Message**: Failed to load model: {details}
**Cause**: Corrupted model file or insufficient memory
**Fix**: Re-download model, check available RAM/VRAM

### OH-3003: Transcription failed
**Message**: Transcription failed: {details}
**Cause**: Engine error
**Fix**: Check logs, try different model

### OH-3004: No GPU available
**Message**: CUDA not available, falling back to CPU
**Cause**: No NVIDIA GPU or drivers not installed
**Fix**: Install NVIDIA drivers or accept slower CPU mode

### OH-3005: GPU out of memory
**Message**: GPU memory exhausted
**Cause**: Model too large for GPU
**Fix**: Use smaller model or close other GPU apps

---

## Output Errors (4xxx)

### OH-4001: Clipboard unavailable
**Message**: Cannot access clipboard
**Cause**: No display server or permission denied
**Fix**: Check DISPLAY/WAYLAND_DISPLAY, run from GUI session

### OH-4002: Paste failed
**Message**: Failed to paste text: {details}
**Cause**: Platform tool missing or permission denied
**Fix**: Install wtype (Wayland) or xdotool (X11)

### OH-4003: Queue overflow
**Message**: Recording queue full ({max} pending)
**Cause**: Transcription slower than recording
**Fix**: Wait for queue to drain or increase max_pending

---

## Platform Errors (5xxx)

### OH-5001: Unsupported platform
**Message**: Platform not supported: {platform}
**Cause**: Running on unsupported OS
**Fix**: Use Linux, macOS, or Windows

### OH-5002: Display server not detected
**Message**: Cannot detect display server
**Cause**: No X11 or Wayland session
**Fix**: Run from GUI session or use TTY mode

### OH-5003: Required tool missing
**Message**: Required tool not found: {tool}
**Cause**: External dependency not installed
**Fix**: Install: `sudo apt install {tool}`

---

## Network/LLM Errors (6xxx)

### OH-6001: Ollama not available
**Message**: Cannot connect to Ollama at {url}
**Cause**: Ollama not running
**Fix**: Start Ollama: `ollama serve`

### OH-6002: LLM request failed
**Message**: LLM correction failed: {details}
**Cause**: Ollama error or timeout
**Fix**: Check Ollama logs, try again

### OH-6003: LLM model not found
**Message**: Ollama model not found: {model}
**Cause**: Model not pulled
**Fix**: Run `ollama pull {model}`

---

## Internal Errors (9xxx)

### OH-9001: Daemon already running
**Message**: OpenHush daemon is already running (PID: {pid})
**Cause**: Another instance running
**Fix**: Stop existing: `openhush stop`

### OH-9002: Daemon not running
**Message**: OpenHush daemon is not running
**Cause**: Daemon not started
**Fix**: Start: `openhush start`

### OH-9003: Internal error
**Message**: Internal error: {details}
**Cause**: Bug in OpenHush
**Fix**: Report issue with debug info: `openhush debug-info`
