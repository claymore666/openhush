# Anti-Patterns to Avoid

Things we've learned NOT to do.

---

## Error Handling

### ❌ Swallowing errors

```rust
// BAD: Error is lost
let _ = might_fail();

// GOOD: Log or propagate
if let Err(e) = might_fail() {
    warn!("Operation failed: {}", e);
}
```

### ❌ Using .unwrap() in production code

```rust
// BAD: Panics on error
let value = might_fail().unwrap();

// GOOD: Handle the error
let value = might_fail()?;
// or
let value = might_fail().unwrap_or_default();
```

### ❌ Generic error messages

```rust
// BAD: Not helpful
return Err(Error::new("Something went wrong"));

// GOOD: Actionable
return Err(Error::ModelNotFound {
    model: name.to_string(),
    path: model_path.display().to_string(),
    hint: "Run 'openhush model download <name>'".to_string(),
});
```

---

## Concurrency

### ❌ Holding locks across await points

```rust
// BAD: Can deadlock
let guard = mutex.lock().await;
some_async_operation().await;  // Still holding lock!
drop(guard);

// GOOD: Minimize lock scope
let data = {
    let guard = mutex.lock().await;
    guard.clone()
};
some_async_operation().await;
```

### ❌ Unbounded channels

```rust
// BAD: Can consume unlimited memory
let (tx, rx) = mpsc::unbounded_channel();

// GOOD: Bounded with backpressure
let (tx, rx) = mpsc::channel(100);
```

### ❌ Blocking in async context

```rust
// BAD: Blocks the async runtime
async fn process() {
    std::thread::sleep(Duration::from_secs(1));  // Blocks!
}

// GOOD: Use async sleep
async fn process() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}

// GOOD: Spawn blocking work
async fn process() {
    tokio::task::spawn_blocking(|| heavy_computation()).await;
}
```

---

## Security

### ❌ String concatenation for commands

```rust
// BAD: Command injection
let cmd = format!("wtype {}", user_text);
Command::new("sh").arg("-c").arg(cmd);

// GOOD: Pass as argument
Command::new("wtype").arg(user_text);
```

### ❌ Logging sensitive data

```rust
// BAD: Exposes user content
info!("Transcribed: {}", transcription);

// GOOD: Log metadata only
info!("Transcribed {} characters", transcription.len());
```

### ❌ Hardcoded paths

```rust
// BAD: Won't work everywhere
let config = "/home/user/.config/openhush/config.toml";

// GOOD: Use directories crate
let config = Config::config_path()?;
```

---

## Performance

### ❌ Cloning large data unnecessarily

```rust
// BAD: Unnecessary clone
fn process(data: Vec<f32>) {
    let copy = data.clone();  // 64KB+ of audio data
    do_something(copy);
}

// GOOD: Pass reference or move
fn process(data: &[f32]) {
    do_something(data);
}
```

### ❌ Allocating in hot loops

```rust
// BAD: Allocates every iteration
for sample in audio {
    let result = format!("{:.2}", sample);  // Allocates
    ...
}

// GOOD: Reuse buffer
let mut buffer = String::with_capacity(10);
for sample in audio {
    buffer.clear();
    write!(&mut buffer, "{:.2}", sample);
    ...
}
```

---

## API Design

### ❌ Boolean parameters

```rust
// BAD: What does true mean?
transcribe(audio, true, false);

// GOOD: Use enums or structs
transcribe(audio, TranscribeOptions {
    language: Language::Auto,
    include_timestamps: false,
});
```

### ❌ Stringly-typed APIs

```rust
// BAD: Easy to pass wrong string
fn set_model(name: &str);
set_model("larg-v3");  // Typo not caught

// GOOD: Use enum
enum Model { Tiny, Base, Small, Medium, LargeV3 }
fn set_model(model: Model);
```

---

## Platform Code

### ❌ #[cfg] scattered everywhere

```rust
// BAD: Hard to maintain
fn paste(text: &str) {
    #[cfg(target_os = "linux")]
    { ... }
    #[cfg(target_os = "macos")]
    { ... }
    #[cfg(target_os = "windows")]
    { ... }
}

// GOOD: Platform module with trait
// platform/mod.rs defines trait
// platform/linux.rs implements for Linux
// Use CurrentPlatform type alias
```

### ❌ Assuming environment

```rust
// BAD: Assumes X11
Command::new("xdotool").arg("type").arg(text);

// GOOD: Detect and dispatch
match DisplayServer::detect() {
    DisplayServer::X11 => xdotool_paste(text),
    DisplayServer::Wayland => wtype_paste(text),
    DisplayServer::Tty => stdout_paste(text),
}
```

---

## Testing

### ❌ Testing implementation details

```rust
// BAD: Breaks when internals change
assert_eq!(queue.internal_buffer.len(), 5);

// GOOD: Test behavior
assert_eq!(queue.pending_count(), 5);
```

### ❌ Flaky time-dependent tests

```rust
// BAD: Race condition
thread::sleep(Duration::from_millis(100));
assert!(result.is_ready());

// GOOD: Use explicit synchronization
let ready = result.wait_ready(Duration::from_secs(5));
assert!(ready);
```
