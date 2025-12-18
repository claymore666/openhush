# Threat Model

## Assets

| Asset | Description | Sensitivity |
|-------|-------------|-------------|
| Transcribed text | User's spoken words | High (may contain PII, passwords, secrets) |
| Audio buffer | Raw microphone input | High |
| Config file | User preferences | Low |
| Model files | Whisper weights | None (public) |
| Clipboard | System clipboard | Medium (shared with other apps) |

## Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│  User's Machine (Trusted)                                       │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  OpenHush Process                                        │  │
│  │  ├── Audio capture                                       │  │
│  │  ├── Whisper transcription                               │  │
│  │  ├── Clipboard/paste                                     │  │
│  │  └── Config management                                   │  │
│  └──────────────────────────────────────────────────────────┘  │
│                           │                                     │
│                           │ localhost (if LLM enabled)          │
│                           ▼                                     │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Ollama (Optional, localhost)                            │  │
│  │  └── LLM for text correction                             │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                            │
                            │ NONE by default
                            ▼
                    ┌───────────────┐
                    │   Internet    │
                    │   (Untrusted) │
                    └───────────────┘
```

## Threat Categories

### T1: Malicious Audio Input
**Vector**: Adversarial audio crafted to produce specific transcription
**Impact**: Whisper outputs attacker-controlled text
**Likelihood**: Low (requires physical access to mic)
**Mitigation**:
- Transcribed text never executed
- User reviews before use

### T2: Config File Tampering
**Vector**: Attacker modifies ~/.config/openhush/config.toml
**Impact**: Could change Ollama endpoint to attacker server
**Likelihood**: Low (requires local access)
**Mitigation**:
- Validate config values
- Warn on non-localhost Ollama URL
- File permissions 600

### T3: Clipboard Sniffing
**Vector**: Other apps read clipboard after transcription
**Impact**: Sensitive transcribed text leaked
**Likelihood**: Medium (clipboard is shared)
**Mitigation**:
- Document that clipboard is shared
- Option to paste-only without clipboard
- Clear clipboard after paste (optional)

### T4: LLM Prompt Injection
**Vector**: User speaks text that manipulates LLM correction
**Impact**: LLM produces unexpected output
**Likelihood**: Medium
**Mitigation**:
- System prompt instructs "only fix grammar"
- Validate LLM output length (≤ 2x input)
- User can disable correction

### T5: Model Supply Chain
**Vector**: Compromised Whisper model file
**Impact**: Malicious transcription behavior
**Likelihood**: Very Low
**Mitigation**:
- Download from official sources
- Verify checksums
- Document model provenance

### T6: Memory Disclosure
**Vector**: Audio/text remains in memory after use
**Impact**: Memory dump reveals sensitive content
**Likelihood**: Low (requires memory access)
**Mitigation**:
- Clear audio buffers after transcription
- Clear text after paste
- Consider `zeroize` crate for sensitive data

### T7: Denial of Service (Self)
**Vector**: Unbounded queue growth exhausts memory
**Impact**: System becomes unresponsive
**Likelihood**: Medium (rapid recording)
**Mitigation**:
- `max_pending` configuration
- Bounded channels
- Reject recordings when queue full

### T8: Privilege Escalation via Paste
**Vector**: Transcribed text pasted into terminal executes commands
**Impact**: Arbitrary command execution
**Likelihood**: Medium (user error)
**Mitigation**:
- Document risk
- No auto-paste in detected terminal windows (optional)
- Confirmation before paste in TTY mode

## Attack Scenarios

### Scenario A: Evil Maid
1. Attacker has brief physical access
2. Modifies config to point Ollama to external server
3. User's transcriptions sent to attacker

**Detection**: Warn on startup if Ollama URL is non-localhost
**Prevention**: Config file integrity check (optional)

### Scenario B: Shoulder Surfing + Clipboard
1. Attacker observes user dictating password
2. Accesses clipboard from another app

**Detection**: N/A
**Prevention**: User education, optional clipboard clearing

### Scenario C: Terminal Injection
1. User dictates "semicolon rm minus rf slash"
2. Whisper transcribes: `; rm -rf /`
3. User pastes into terminal without looking

**Detection**: Detect dangerous patterns before paste
**Prevention**: Warn before pasting shell metacharacters

## Security Controls Summary

| Control | Status | Priority |
|---------|--------|----------|
| No network by default | ✅ Implemented | Must |
| Localhost-only Ollama | ✅ Default | Must |
| Bounded queue | ✅ Configurable | Must |
| Config validation | 🔲 TODO | Should |
| Clipboard clearing | 🔲 TODO | Could |
| Shell metachar warning | 🔲 TODO | Could |
| Model checksum verify | 🔲 TODO | Could |
| Memory zeroing | 🔲 TODO | Could |
