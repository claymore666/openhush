# Security Guidelines

## Input Validation

### Audio Input
- **Max duration**: Configurable, default 30s (prevents memory exhaustion)
- **Sample rate**: Validate 16kHz, reject others
- **Format**: Only accept expected PCM format

### Config Files
- **Path traversal**: Sanitize paths, no `..` sequences
- **TOML parsing**: Use safe parser, limit nesting depth
- **Values**: Validate ranges (e.g., `max_pending` 0-1000)

### User Text (transcription output)
- **No shell execution**: Never pass transcribed text to shell
- **Clipboard**: Safe (no execution risk)
- **Paste via tools**: Use argument passing, not string interpolation

## Command Execution

### Safe Pattern
```rust
// GOOD: Arguments are escaped
Command::new("wtype")
    .arg(user_text)  // Safe: treated as single argument
    .status()?;
```

### Dangerous Pattern
```rust
// BAD: Shell injection possible
Command::new("sh")
    .arg("-c")
    .arg(format!("wtype {}", user_text))  // DANGER!
    .status()?;
```

## File System

### Paths
- Use `directories` crate for platform-appropriate paths
- Never construct paths from user input without validation
- Use `Path::join()`, not string concatenation

### Permissions
- Config files: 600 (user read/write only)
- Model files: 644 (readable)
- PID file: 644

### Temp Files
- Use `tempfile` crate
- Clean up on exit (RAII pattern)

## Network

### Ollama Connection
- Localhost only by default
- Validate URL scheme (http/https only)
- Timeout on requests
- Don't log request/response bodies

### No Outbound by Default
- No telemetry
- No update checks
- No analytics
- User explicitly enables LLM correction

## Memory Safety

### Audio Buffers
- Bounded size (max duration × sample rate × sizeof(f32))
- Clear after transcription
- No unbounded growth

### Queue Limits
- `max_pending` configuration
- Reject new recordings if queue full (don't OOM)

## Logging

### What to Log
- Errors with context
- Lifecycle events (start, stop)
- Performance metrics

### What NOT to Log
- Transcribed text content
- Audio data
- User configuration values
- File paths containing usernames

### Log Levels in Production
```
error  → Always shown
warn   → Shown
info   → Shown (default)
debug  → Hidden unless -v
trace  → Hidden unless -vv
```

## Dependencies

### Audit Regularly
```bash
cargo audit        # Known vulnerabilities
cargo deny check   # License + banned crates
cargo outdated     # Old versions
```

### Minimize Attack Surface
- Prefer well-maintained crates
- Check download counts, recent updates
- Audit `unsafe` usage with `cargo-geiger`

## Unsafe Code

### Current Policy
- Minimize `unsafe` blocks
- Document why `unsafe` is necessary
- Wrap in safe abstractions
- Test with Miri when possible

### Review Checklist for Unsafe
- [ ] Is `unsafe` actually necessary?
- [ ] Are all invariants documented?
- [ ] Are bounds checked before pointer access?
- [ ] Is memory properly initialized?
- [ ] Is there a safe alternative?
