# Product Vision

This document describes the evolution of OpenHush from a personal voice-to-text tool to an enterprise-ready transcription platform.

---

## The Journey

```
Phase 1          Phase 2          Phase 3          Phase 4          Phase 5
v0.1-0.4         v0.5+            Future           Plugin           Vision
────────────────────────────────────────────────────────────────────────────▶

Personal         File-Based       Plugin           Meeting          Enterprise
Voice-to-Text    Transcription    System           Minutes          Services
                                                   Plugin
```

---

## Phase 1: Personal Voice-to-Text Tool (v0.1-0.4)

**Status:** Completed

The foundation: a fast, local, privacy-first transcription tool for everyday use.

### Delivered Features

- Push-to-talk and toggle recording modes
- Local Whisper model transcription (tiny to large-v3)
- GPU acceleration (NVIDIA CUDA)
- Auto-paste to cursor
- System tray integration
- Cross-platform support (Linux, macOS, Windows in progress)
- Configuration via TOML files

### Design Decisions

- **Privacy-First:** No cloud, no telemetry, no subscriptions
- **Zero Configuration:** Works out of the box with sensible defaults
- **Native Performance:** Rust for speed and reliability

---

## Phase 2: File-Based Transcription (v0.5+)

**Status:** In Progress

Expand from real-time dictation to processing recorded audio files.

### Delivered Features

- `openhush transcribe` command for WAV/MP3 files
- JSON and text output formats
- Performance metrics (RTF, duration)
- Multi-GPU support (AMD ROCm, Apple Metal, Vulkan)
- D-Bus service interface for daemon control

### Use Cases

- Transcribe meeting recordings
- Process voice memos
- Batch transcription of audio archives

### Example

```bash
# Transcribe a meeting recording
openhush transcribe meeting.wav --output json

# Output:
{
  "text": "Welcome to the standup. Let's go around the room...",
  "language": "en",
  "duration_ms": 1847000,
  "audio_duration_secs": 1847.0,
  "transcription_time_ms": 72360,
  "real_time_factor": 0.039,
  "model": "medium"
}
```

---

## Phase 3: Plugin System (Future)

**Status:** Planned

An extensible architecture for community-developed plugins.

### Why Plugins?

- Keep the core lean and focused
- Enable domain-specific features without bloat
- Foster community contributions
- Allow enterprise customization

### Plugin Hooks

| Hook | Purpose | Example |
|------|---------|---------|
| **Audio Pre-processing** | Modify audio before transcription | Custom noise reduction |
| **Transcription Post-processing** | Transform text after Whisper | Grammar correction |
| **Output Handlers** | Custom output destinations | Send to Slack |
| **Command Extensions** | Add new CLI commands | `openhush meeting-minutes` |

### Plugin Lifecycle

```
Discovery ──▶ Loading ──▶ Initialization ──▶ Runtime ──▶ Shutdown
    │            │              │              │            │
  Folder      Dynamic       Config         Hooks        Cleanup
  Scan        Import        Parse          Execute      Resources
```

### Technical Approach

Options under consideration:
1. **Dynamic Libraries** - Native Rust plugins via cdylib
2. **Subprocess** - Plugins as separate executables
3. **WASM** - WebAssembly for sandboxed execution
4. **Scripting** - Embedded Lua or Rhai for simple plugins

See [Plugin System](Plugin-System) for detailed design.

---

## Phase 4: Meeting Minutes Plugin

**Status:** Planned (depends on Phase 3)

A plugin that transforms raw transcriptions into structured meeting notes.

### Features

- LLM integration for intelligent formatting (via Ollama)
- Prompt templates for different meeting types
- Structured output with:
  - Attendees and date
  - Key discussion points
  - Decisions made
  - Action items (who, what, when)

### Example Output

```markdown
# Standup Meeting - 2025-01-15

## Attendees
- Alice (Engineering Lead)
- Bob (Backend Developer)
- Charlie (Product Manager)

## Discussion Points
- Sprint progress review
- API redesign status
- Customer feedback on v2.0

## Decisions
- Push API redesign to next sprint
- Prioritize performance fixes

## Action Items
| Owner | Task | Due |
|-------|------|-----|
| Bob | Fix memory leak in worker | Jan 17 |
| Alice | Review PR #234 | Jan 16 |
| Charlie | Update roadmap doc | Jan 18 |
```

### Templates

- Standup/Daily scrum
- Sprint planning
- Retrospective
- 1-on-1 meetings
- Customer calls
- Custom templates

---

## Phase 5: Enterprise Services (Vision)

**Status:** Vision / Long-term

High-quality transcription services for European enterprises with GDPR compliance.

### Planned Features

- **Large Model Hosting** - Run large-v3 on dedicated GPU infrastructure
- **REST API** - For integration with enterprise systems
- **Plugin Marketplace** - Curated plugins for enterprise use
- **Team Management** - User roles, usage tracking
- **SLA Guarantees** - Uptime, latency, accuracy commitments

### GDPR Compliance

- All processing in EU data centers
- No data retention beyond processing
- Data Processing Agreements (DPA)
- Right to erasure support
- Audit logging

### What We Are NOT Building

**SIP/VoIP Hosting** - Explicitly excluded due to telecom compliance burden:
- Telecom provider licensing requirements vary by country
- Emergency services (112/911) obligations
- Lawful intercept requirements
- Number portability regulations
- Too much regulatory overhead for the value provided

**Alternative Approach:** Provide client-side integrations that work with existing VoIP systems without becoming a telecom provider.

---

## Design Principles

These principles guide all development decisions:

### 1. Privacy-First

> All processing happens locally by default. No cloud required.

- Audio never leaves the user's machine (in personal mode)
- No telemetry or usage tracking
- Open source for auditability

### 2. Local Processing

> GDPR compliance by design, not by policy.

- When enterprise features require server-side processing, it's opt-in
- EU hosting for enterprise customers
- Data minimization and purpose limitation

### 3. Open Source

> Community-driven development with transparent governance.

- MIT license for core functionality
- Plugin API documentation
- Contribution guidelines
- Public roadmap

### 4. Performance

> Native speed, GPU-accelerated, async architecture.

- Rust for reliability and performance
- Zero-copy audio processing where possible
- Async/await for responsiveness
- GPU acceleration for all major vendors

---

## Roadmap Timeline

| Phase | Milestone | Status |
|-------|-----------|--------|
| 1.0 | Personal voice-to-text | Completed |
| 1.1 | GPU acceleration | Completed |
| 1.2 | Cross-platform | In Progress |
| 2.0 | File transcription | Completed |
| 2.1 | Multi-GPU backends | In Progress |
| 3.0 | Plugin system | Planned |
| 3.1 | Plugin API docs | Planned |
| 4.0 | Meeting minutes plugin | Planned |
| 5.0 | Enterprise beta | Vision |

---

## See Also

- [Market Analysis](Market-Analysis) - Target audiences and competitive positioning
- [Plugin System](Plugin-System) - Detailed plugin architecture
- [Architecture](Architecture) - Technical overview
