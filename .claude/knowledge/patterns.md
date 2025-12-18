# Established Patterns

Good patterns we use consistently in this codebase.

---

## Error Handling

### Module-level error types with thiserror

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModuleError {
    #[error("Description of error: {0}")]
    VariantName(String),

    #[error("Wrapped error: {0}")]
    Wrapped(#[from] OtherError),
}
```

### Propagate with context

```rust
use anyhow::{Context, Result};

fn do_thing() -> Result<()> {
    inner_operation()
        .context("Failed to do thing")?;
    Ok(())
}
```

---

## Logging

### Use tracing macros consistently

```rust
use tracing::{debug, info, warn, error, instrument};

#[instrument(skip(large_data))]
fn process(id: u64, large_data: &[u8]) -> Result<()> {
    debug!("Starting processing");
    info!(id, "Processing item");
    warn!("Something unusual");
    error!("Something failed");
}
```

### Log levels

| Level | Use for |
|-------|---------|
| error | Failures that need attention |
| warn | Unusual but recoverable |
| info | Key lifecycle events |
| debug | Detailed flow for debugging |
| trace | Very verbose, rarely used |

---

## Async Patterns

### Prefer channels over shared state

```rust
// Good: Channel-based communication
let (tx, rx) = mpsc::channel(100);
tokio::spawn(async move {
    while let Some(item) = rx.recv().await {
        process(item).await;
    }
});

// Avoid: Shared mutex across async tasks
// let shared = Arc::new(Mutex::new(data));  // Can deadlock
```

### Use tokio::select! for multiple sources

```rust
loop {
    tokio::select! {
        Some(recording) = recording_rx.recv() => {
            handle_recording(recording).await;
        }
        Some(result) = result_rx.recv() => {
            handle_result(result).await;
        }
        _ = shutdown.recv() => {
            break;
        }
    }
}
```

---

## Platform Abstraction

### Trait + platform implementations

```rust
// In mod.rs
pub trait PlatformFeature: Send + Sync {
    fn do_thing(&self) -> Result<(), Error>;
}

// In linux.rs
pub struct LinuxImpl;
impl PlatformFeature for LinuxImpl { ... }

// In main code
#[cfg(target_os = "linux")]
type CurrentPlatform = LinuxImpl;
```

---

## Configuration

### Serde with defaults

```rust
#[derive(Deserialize)]
pub struct Config {
    #[serde(default = "default_value")]
    pub field: Type,
}

fn default_value() -> Type {
    Type::default()
}
```

### Validation at load time

```rust
impl Config {
    pub fn load() -> Result<Self> {
        let config: Config = load_toml()?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.queue.max_pending > 1000 {
            return Err(ConfigError::Invalid("max_pending too high"));
        }
        Ok(())
    }
}
```

---

## Testing

### Arrange-Act-Assert

```rust
#[test]
fn test_thing() {
    // Arrange
    let input = setup();

    // Act
    let result = function(input);

    // Assert
    assert_eq!(result, expected);
}
```

### Test one thing per test

```rust
// Good: Focused tests
#[test]
fn queue_assigns_sequential_ids() { ... }

#[test]
fn queue_respects_max_pending() { ... }

// Avoid: Testing multiple behaviors
#[test]
fn queue_works() { ... }  // Too broad
```

---

## Documentation

### Public API docs

```rust
/// Short description.
///
/// Longer explanation if needed.
///
/// # Arguments
///
/// * `param` - Description
///
/// # Returns
///
/// Description of return value.
///
/// # Errors
///
/// Returns `Error::Variant` when...
///
/// # Examples
///
/// ```
/// let result = function(input);
/// ```
pub fn function(param: Type) -> Result<Output> { ... }
```

---

## Resource Management

### RAII for cleanup

```rust
struct TempFile {
    path: PathBuf,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
```

### Graceful shutdown

```rust
pub async fn run(&mut self) -> Result<()> {
    let shutdown = tokio::signal::ctrl_c();

    tokio::select! {
        result = self.main_loop() => result,
        _ = shutdown => {
            self.cleanup().await?;
            Ok(())
        }
    }
}
```
