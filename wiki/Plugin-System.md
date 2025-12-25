# Plugin System

> **Status:** Future Design Document
>
> This documents the planned plugin system architecture. Implementation is targeted for Phase 3 of the [Product Vision](Product-Vision).

---

## Overview

The plugin system will enable OpenHush to be extended without modifying the core application. This keeps the core lean while allowing domain-specific features through community plugins.

### Goals

1. **Extensibility** - Add features without bloating core
2. **Community** - Enable third-party contributions
3. **Customization** - Enterprise-specific integrations
4. **Safety** - Sandboxed execution for untrusted plugins

### Non-Goals

- Real-time audio processing (latency-critical)
- Core transcription engine replacement
- GUI framework replacement

---

## Plugin Hooks

Plugins can hook into four extension points in the OpenHush pipeline:

```
┌──────────────────────────────────────────────────────────────────────┐
│                          Plugin Hooks                                 │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  Audio ──▶ [Pre-Process] ──▶ Whisper ──▶ [Post-Process] ──▶ Output   │
│                │                              │               │       │
│            Hook 1                         Hook 2          Hook 3      │
│                                                                       │
│  CLI ──▶ [Command Extension]                                         │
│                    │                                                  │
│                Hook 4                                                 │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### 1. Audio Pre-processing

**When:** Before audio is sent to Whisper

**Use Cases:**
- Custom noise reduction algorithms
- Audio enhancement
- Speaker isolation
- Audio format conversion

**Example:**
```rust
trait AudioPreProcessor {
    fn process(&self, audio: &[f32], sample_rate: u32) -> Vec<f32>;
    fn name(&self) -> &str;
}
```

### 2. Transcription Post-processing

**When:** After Whisper returns text, before output

**Use Cases:**
- Meeting minutes formatting
- Custom vocabulary replacement
- Domain-specific corrections
- Text summarization

**Example:**
```rust
trait TextPostProcessor {
    fn process(&self, text: &str, metadata: &TranscriptionMetadata) -> String;
    fn name(&self) -> &str;
}
```

### 3. Output Handlers

**When:** When transcription result is ready

**Use Cases:**
- Send to Slack/Discord/Teams
- Save to database
- Write to custom file format
- Trigger webhooks

**Example:**
```rust
trait OutputHandler {
    async fn handle(&self, result: &TranscriptionResult) -> Result<()>;
    fn name(&self) -> &str;
}
```

### 4. Command Extensions

**When:** User invokes a custom CLI command

**Use Cases:**
- `openhush meeting-minutes --template standup`
- `openhush batch-transcribe ./recordings/`
- `openhush export --format obsidian`

**Example:**
```rust
trait CommandExtension {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn run(&self, args: &[String]) -> Result<()>;
}
```

---

## Example Plugin: Meeting Minutes

The flagship plugin demonstrating the plugin system.

### Workflow

```
Audio File ──▶ OpenHush Transcribe ──▶ Plugin Post-Process ──▶ Formatted Output
                      │                        │                     │
                 Raw text              LLM + Template          Markdown file
```

### Input

Raw transcription from `openhush transcribe`:

```
Welcome to the standup. Let me go around the room. Alice, how are you doing?
Good morning everyone. Yesterday I finished the API redesign PR and today I'll
be working on the database migration. No blockers for me. Bob? Thanks Alice.
I'm still working on the performance issue from last week. I might need some
help from Charlie on the profiling setup. Charlie here. Happy to help Bob,
let's sync after this call. I'll also be reviewing Alice's PR today...
```

### Processing

1. Send to Ollama with template prompt
2. LLM extracts structure:
   - Attendees mentioned
   - Discussion topics
   - Decisions made
   - Action items with owners

### Output

```markdown
# Standup Meeting - 2025-01-15

## Attendees
- Alice
- Bob
- Charlie

## Updates

### Alice
- Yesterday: Finished API redesign PR
- Today: Working on database migration
- Blockers: None

### Bob
- Yesterday: Working on performance issue
- Today: Continue performance work
- Blockers: Needs help with profiling setup

### Charlie
- Today: Help Bob with profiling, review Alice's PR
- Blockers: None

## Action Items

| Owner | Task | Due |
|-------|------|-----|
| Charlie | Help Bob with profiling | Today |
| Charlie | Review Alice's PR | Today |
```

### Templates

Different meeting types require different prompts:

| Template | Focus |
|----------|-------|
| `standup` | Updates, blockers, action items |
| `planning` | User stories, sprint goals, capacity |
| `retro` | What went well, what to improve, actions |
| `1on1` | Discussion points, feedback, goals |
| `customer` | Requirements, concerns, next steps |

---

## Technical Architecture

### Plugin Discovery

Plugins are discovered from a configured directory:

```
~/.config/openhush/plugins/
├── meeting-minutes/
│   ├── plugin.toml       # Plugin manifest
│   ├── plugin.so         # Dynamic library (or .wasm)
│   └── templates/        # Plugin assets
├── slack-output/
│   ├── plugin.toml
│   └── plugin.so
└── custom-noise-reduction/
    ├── plugin.toml
    └── plugin.so
```

### Plugin Manifest

```toml
# plugin.toml
[plugin]
name = "meeting-minutes"
version = "1.0.0"
description = "Format transcriptions as meeting minutes"
author = "OpenHush Community"
license = "MIT"

[hooks]
post_process = true
command_extension = true

[commands]
meeting-minutes = "Format transcription as meeting minutes"

[dependencies]
ollama = "required"  # Requires Ollama for LLM

[config]
template = { type = "string", default = "standup" }
ollama_model = { type = "string", default = "llama3.2:3b" }
```

### Implementation Options

| Approach | Pros | Cons |
|----------|------|------|
| **Dynamic Libraries** | Native speed, full Rust API | ABI stability, platform-specific |
| **Subprocess** | Language-agnostic, isolated | IPC overhead, startup time |
| **WASM** | Sandboxed, portable | Limited APIs, performance |
| **Scripting (Lua/Rhai)** | Easy to write, safe | Limited functionality |

**Recommended:** Start with subprocess for safety and language flexibility, add dynamic library support for performance-critical plugins later.

### Security Considerations

#### Sandboxing

Plugins run in restricted environments:
- No direct filesystem access (except designated directories)
- No network access (except whitelisted endpoints)
- Resource limits (CPU, memory, time)

#### Permissions

Plugins declare required permissions in manifest:

```toml
[permissions]
filesystem = ["~/.config/openhush/plugins/meeting-minutes/"]
network = ["localhost:11434"]  # Ollama
clipboard = false
```

User approves permissions on install.

#### Code Signing

For plugin marketplace:
- Plugins signed by developers
- Signature verified on install
- Revocation list for malicious plugins

---

## Plugin Development

### Minimal Example

```rust
use openhush_plugin::{Plugin, TextPostProcessor, TranscriptionMetadata};

pub struct UppercasePlugin;

impl Plugin for UppercasePlugin {
    fn name(&self) -> &str {
        "uppercase"
    }
}

impl TextPostProcessor for UppercasePlugin {
    fn process(&self, text: &str, _metadata: &TranscriptionMetadata) -> String {
        text.to_uppercase()
    }
}

openhush_plugin::export_plugin!(UppercasePlugin);
```

### Plugin SDK

```toml
# Cargo.toml for plugin
[dependencies]
openhush-plugin = "0.1"
```

The SDK provides:
- Trait definitions for hooks
- Helper functions for common tasks
- Configuration parsing
- Logging integration
- Error handling

---

## Configuration

### Per-Plugin Config

```toml
# ~/.config/openhush/config.toml

[plugins]
enabled = true
directory = "~/.config/openhush/plugins"

[plugins.meeting-minutes]
enabled = true
template = "standup"
ollama_model = "llama3.2:3b"

[plugins.slack-output]
enabled = true
webhook_url = "https://hooks.slack.com/..."
channel = "#transcriptions"
```

### Plugin Priority

Multiple plugins of the same type are chained:

```toml
[plugins.post_process_order]
plugins = ["grammar-fix", "meeting-minutes", "uppercase"]
```

---

## Roadmap

| Milestone | Description | Status |
|-----------|-------------|--------|
| 3.0 | Plugin system core | Planned |
| 3.1 | Plugin API documentation | Planned |
| 3.2 | Plugin SDK release | Planned |
| 3.3 | Example plugins | Planned |
| 4.0 | Meeting minutes plugin | Planned |
| 5.0 | Plugin marketplace | Vision |

---

## See Also

- [Product Vision](Product-Vision) - Roadmap context
- [Architecture](Architecture) - System overview
- [Components](Components) - Core module documentation
