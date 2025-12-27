# API Reference

OpenHush provides multiple interfaces for control and integration.

---

## Table of Contents

1. [CLI](#cli)
2. [REST API](#rest-api)
3. [D-Bus (Linux)](#d-bus-linux)
4. [Unix Socket (Linux/macOS)](#unix-socket-linuxmacos)
5. [Named Pipe (Windows)](#named-pipe-windows)

---

## Interface Comparison

| Feature | CLI | REST API | D-Bus | Unix Socket |
|---------|-----|----------|-------|-------------|
| Start/stop recording | ✅ | ✅ | ✅ | ❌ |
| Toggle recording | ✅ | ✅ | ✅ | ❌ |
| Get status | ✅ | ✅ | ✅ | ✅ |
| Stop daemon | ✅ | ❌ | ❌ | ✅ |
| Model management | ✅ | ❌ | ❌ | ❌ |
| Config management | ✅ | ❌ | ❌ | ❌ |
| File transcription | ✅ | ❌ | ❌ | ❌ |
| Real-time events | ❌ | ❌ | ✅ | ❌ |
| Remote access | ❌ | ✅ | ❌ | ❌ |
| Authentication | N/A | API key | N/A | N/A |

---

## CLI

The primary interface for interactive use.

### Daemon Control

```bash
# Start daemon (background)
openhush start

# Start daemon (foreground, for debugging)
openhush start -f

# Stop daemon
openhush stop

# Check status
openhush status
```

### Recording Control

```bash
# Start recording (requires running daemon)
openhush recording start

# Stop recording
openhush recording stop

# Toggle recording
openhush recording toggle

# Check recording status
openhush recording status
```

### Model Management

```bash
# List available models
openhush model list

# Download a model
openhush model download small
openhush model download large-v3

# Remove a model
openhush model remove tiny
```

### Configuration

```bash
# Show current config
openhush config

# Set hotkey
openhush config hotkey ControlLeft

# Set model
openhush config model medium

# Set language
openhush config language de

# Enable/disable options
openhush config paste on
openhush config clipboard off
```

### File Transcription

```bash
# Transcribe audio file
openhush transcribe recording.wav

# JSON output
openhush transcribe recording.wav --output json

# Specify model
openhush transcribe recording.wav --model large-v3

# Specify language
openhush transcribe recording.wav --language en
```

### Service Management

```bash
# Install autostart
openhush service install

# Remove autostart
openhush service uninstall

# Check service status
openhush service status
```

### API Key Management

```bash
# Generate new API key
openhush api-key generate

# Show current key hash
openhush api-key show
```

### Secret Management

```bash
# Store a secret in system keyring
openhush secret set ollama-api

# List stored secrets
openhush secret list

# Delete a secret
openhush secret delete ollama-api
```

---

## REST API

HTTP API for remote control and integrations. Disabled by default.

### Configuration

```toml
# ~/.config/openhush/config.toml

[api]
enabled = true
bind = "127.0.0.1:8080"
swagger_ui = true
cors_origins = []  # Empty = same-origin only, ["*"] = allow all
```

### Authentication

All endpoints except `/api/v1/health` require an API key.

```bash
# Generate API key
openhush api-key generate
# Output: API key generated: oh_abc123...

# Use in requests
curl -H "X-API-Key: oh_abc123..." http://localhost:8080/api/v1/status
```

### Endpoints

#### Health Check

```http
GET /api/v1/health
```

No authentication required.

**Response:**
```json
{
  "status": "ok",
  "version": "0.6.0"
}
```

#### Get Status

```http
GET /api/v1/status
```

**Response:**
```json
{
  "running": true,
  "recording": false,
  "queue_depth": 0,
  "version": "0.6.0"
}
```

#### Start Recording

```http
POST /api/v1/recording/start
```

**Response:**
```json
{
  "success": true,
  "message": "Recording started"
}
```

#### Stop Recording

```http
POST /api/v1/recording/stop
```

**Response:**
```json
{
  "success": true,
  "message": "Recording stopped"
}
```

#### Toggle Recording

```http
POST /api/v1/recording/toggle
```

**Response:**
```json
{
  "success": true,
  "message": "Recording toggled",
  "recording": true
}
```

### Swagger UI

When enabled, interactive API documentation is available at:

```
http://localhost:8080/swagger-ui/
```

### Examples

**curl:**
```bash
# Check status
curl -H "X-API-Key: $API_KEY" http://localhost:8080/api/v1/status

# Start recording
curl -X POST -H "X-API-Key: $API_KEY" http://localhost:8080/api/v1/recording/start

# Stop recording
curl -X POST -H "X-API-Key: $API_KEY" http://localhost:8080/api/v1/recording/stop
```

**Python:**
```python
import requests

API_KEY = "oh_abc123..."
BASE_URL = "http://localhost:8080/api/v1"
headers = {"X-API-Key": API_KEY}

# Get status
r = requests.get(f"{BASE_URL}/status", headers=headers)
print(r.json())

# Toggle recording
r = requests.post(f"{BASE_URL}/recording/toggle", headers=headers)
print(r.json())
```

**Home Assistant REST command:**
```yaml
rest_command:
  openhush_toggle:
    url: "http://localhost:8080/api/v1/recording/toggle"
    method: POST
    headers:
      X-API-Key: "oh_abc123..."
```

---

## D-Bus (Linux)

Native Linux IPC with real-time signals. Automatically started with the daemon.

### Bus Information

| Property | Value |
|----------|-------|
| Bus | Session bus |
| Service name | `org.openhush.Daemon1` |
| Object path | `/org/openhush/Daemon1` |
| Interface | `org.openhush.Daemon1` |

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `StartRecording` | `() → ()` | Begin audio capture |
| `StopRecording` | `() → ()` | End audio capture |
| `ToggleRecording` | `() → ()` | Toggle recording state |
| `GetStatus` | `() → s` | Get status as JSON string |

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `IsRecording` | `b` | Current recording state |
| `QueueDepth` | `u` | Pending transcriptions |
| `Version` | `s` | Daemon version |

### Signals

| Signal | Description |
|--------|-------------|
| `IsRecordingChanged` | Emitted when recording starts/stops |

### Examples

**dbus-send:**
```bash
# Check if daemon is running
dbus-send --session --print-reply \
  --dest=org.openhush.Daemon1 \
  /org/openhush/Daemon1 \
  org.freedesktop.DBus.Peer.Ping

# Get status
dbus-send --session --print-reply \
  --dest=org.openhush.Daemon1 \
  /org/openhush/Daemon1 \
  org.openhush.Daemon1.GetStatus

# Start recording
dbus-send --session --print-reply \
  --dest=org.openhush.Daemon1 \
  /org/openhush/Daemon1 \
  org.openhush.Daemon1.StartRecording

# Toggle recording
dbus-send --session --print-reply \
  --dest=org.openhush.Daemon1 \
  /org/openhush/Daemon1 \
  org.openhush.Daemon1.ToggleRecording

# Get IsRecording property
dbus-send --session --print-reply \
  --dest=org.openhush.Daemon1 \
  /org/openhush/Daemon1 \
  org.freedesktop.DBus.Properties.Get \
  string:"org.openhush.Daemon1" string:"IsRecording"
```

**busctl:**
```bash
# Introspect interface
busctl --user introspect org.openhush.Daemon1 /org/openhush/Daemon1

# Call method
busctl --user call org.openhush.Daemon1 /org/openhush/Daemon1 \
  org.openhush.Daemon1 ToggleRecording

# Get property
busctl --user get-property org.openhush.Daemon1 /org/openhush/Daemon1 \
  org.openhush.Daemon1 IsRecording

# Monitor signals
busctl --user monitor org.openhush.Daemon1
```

**Python (pydbus):**
```python
from pydbus import SessionBus

bus = SessionBus()
daemon = bus.get("org.openhush.Daemon1", "/org/openhush/Daemon1")

# Check recording state
print(f"Recording: {daemon.IsRecording}")

# Toggle recording
daemon.ToggleRecording()

# Subscribe to signals
daemon.IsRecordingChanged.connect(lambda: print("Recording changed!"))
```

---

## Unix Socket (Linux/macOS)

JSON-based IPC for local daemon control.

### Socket Location

| Platform | Path |
|----------|------|
| Linux | `$XDG_RUNTIME_DIR/openhush.sock` or `/tmp/openhush.sock` |
| macOS | `/tmp/openhush.sock` |

### Protocol

Line-delimited JSON over Unix domain socket.

**Request format:**
```json
{"cmd": "command_name"}
```

**Response format:**
```json
{"ok": true, "running": true, "recording": false, "version": "0.6.0"}
```

### Commands

| Command | Description |
|---------|-------------|
| `status` | Get daemon status |
| `stop` | Stop the daemon |

### Examples

**netcat:**
```bash
# Get status
echo '{"cmd":"status"}' | nc -U /tmp/openhush.sock

# Stop daemon
echo '{"cmd":"stop"}' | nc -U /tmp/openhush.sock
```

**socat:**
```bash
# Interactive session
socat - UNIX-CONNECT:/tmp/openhush.sock
{"cmd":"status"}
```

**Python:**
```python
import socket
import json

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect("/tmp/openhush.sock")

# Send command
cmd = {"cmd": "status"}
sock.send((json.dumps(cmd) + "\n").encode())

# Read response
response = sock.recv(4096).decode()
print(json.loads(response))

sock.close()
```

---

## Named Pipe (Windows)

JSON-based IPC for Windows daemon control.

### Pipe Path

```
\\.\pipe\openhush
```

### Protocol

Same JSON protocol as Unix socket.

### Commands

| Command | Description |
|---------|-------------|
| `status` | Get daemon status |
| `stop` | Stop the daemon |

### Example (PowerShell)

```powershell
$pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", "openhush", [System.IO.Pipes.PipeDirection]::InOut)
$pipe.Connect(5000)

$writer = New-Object System.IO.StreamWriter($pipe)
$reader = New-Object System.IO.StreamReader($pipe)

$writer.WriteLine('{"cmd":"status"}')
$writer.Flush()

$response = $reader.ReadLine()
Write-Host $response

$pipe.Close()
```

---

## See Also

- [User Guide](User-Guide) - Configuration and usage
- [Components](Components) - Module documentation
- [Architecture](Architecture) - System overview
