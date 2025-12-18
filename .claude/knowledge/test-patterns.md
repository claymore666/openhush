# Test Patterns

## Unit Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptive_name() {
        // Arrange
        let input = ...;

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

## Async Test Structure

```rust
#[tokio::test]
async fn test_async_operation() {
    // Use tokio::time::pause() for time-sensitive tests
    tokio::time::pause();

    let result = async_function().await;

    assert!(result.is_ok());
}
```

## Mocking Audio Input

```rust
/// Generate test audio: 1 second of 440Hz sine wave
fn generate_test_audio(duration_secs: f32) -> Vec<f32> {
    let sample_rate = 16000;
    let samples = (sample_rate as f32 * duration_secs) as usize;
    (0..samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (t * 440.0 * 2.0 * std::f32::consts::PI).sin()
        })
        .collect()
}

/// Generate silence
fn generate_silence(duration_secs: f32) -> Vec<f32> {
    let samples = (16000.0 * duration_secs) as usize;
    vec![0.0; samples]
}

/// Load test fixture
fn load_test_audio(name: &str) -> Vec<f32> {
    let path = format!("tests/fixtures/{}.wav", name);
    // Load and convert to f32 mono 16kHz
    todo!()
}
```

## Mocking GPU (Feature Flag)

```rust
#[cfg(test)]
pub struct MockGpuWorker {
    pub response: String,
    pub delay: Duration,
    pub should_fail: bool,
}

#[cfg(test)]
impl GpuWorker for MockGpuWorker {
    fn transcribe(&mut self, _audio: &[f32]) -> Result<String, GpuError> {
        std::thread::sleep(self.delay);
        if self.should_fail {
            Err(GpuError::TranscriptionFailed("mock failure".into()))
        } else {
            Ok(self.response.clone())
        }
    }
}
```

## Mocking Clipboard

```rust
#[cfg(test)]
pub struct MockClipboard {
    pub contents: std::sync::Mutex<String>,
}

#[cfg(test)]
impl TextOutput for MockClipboard {
    fn copy_to_clipboard(&self, text: &str) -> Result<(), PlatformError> {
        *self.contents.lock().unwrap() = text.to_string();
        Ok(())
    }

    fn paste_text(&self, _text: &str) -> Result<(), PlatformError> {
        // In tests, paste is a no-op
        Ok(())
    }
}
```

## Mocking HTTP (Ollama)

```rust
// Use wiremock for HTTP mocking
#[tokio::test]
async fn test_ollama_correction() {
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({"response": "Corrected text."})))
        .mount(&mock_server)
        .await;

    let corrector = OllamaCorrector::new(&mock_server.uri());
    let result = corrector.correct("uncorrected text").await;

    assert_eq!(result.unwrap(), "Corrected text.");
}
```

## Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    /// Queue always preserves order
    #[test]
    fn queue_preserves_order(recordings in prop::collection::vec(any::<Vec<f32>>(), 1..100)) {
        let queue = RecordingQueue::new();

        let ids: Vec<u64> = recordings.iter()
            .map(|r| queue.enqueue(r.clone()).unwrap())
            .collect();

        // IDs must be strictly increasing
        for window in ids.windows(2) {
            prop_assert!(window[0] < window[1]);
        }
    }

    /// Result aggregator outputs in sequence order
    #[test]
    fn results_output_in_order(mut results in prop::collection::vec(
        (0u64..1000, ".*"),
        1..50
    )) {
        let aggregator = ResultAggregator::new();

        // Shuffle input order
        results.shuffle(&mut thread_rng());

        for (seq_id, text) in &results {
            aggregator.submit(TranscriptionResult {
                sequence_id: *seq_id,
                text: text.clone(),
            });
        }

        // Output must be sorted by sequence_id
        let mut last_id = 0;
        while let Some(result) = aggregator.drain_ready().first() {
            prop_assert!(result.sequence_id >= last_id);
            last_id = result.sequence_id;
        }
    }
}
```

## Integration Test Structure

```rust
// tests/integration/queue_to_output.rs

#[tokio::test]
async fn full_pipeline_preserves_order() {
    // Setup
    let config = Config::default();
    let mut daemon = Daemon::new(config).unwrap();

    // Simulate 5 rapid recordings
    for i in 0..5 {
        let audio = generate_test_audio(1.0);
        daemon.queue_recording(audio).unwrap();
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Verify output order
    let output = daemon.get_output();
    assert!(output.contains("text1") && output.find("text1") < output.find("text2"));
}
```

## Fault Injection

```rust
#[test]
fn gpu_failure_triggers_fallback() {
    let mut pool = GpuPool::new();

    // Configure first GPU to fail
    pool.set_gpu_behavior(0, GpuBehavior::FailAfter(1));

    let recording = Recording::new(generate_test_audio(1.0));
    pool.submit(recording).unwrap();

    // Should still get result (from fallback)
    let result = pool.receive_timeout(Duration::from_secs(30));
    assert!(result.is_some());
}
```

## Test Fixtures Location

```
tests/
├── fixtures/
│   ├── audio/
│   │   ├── hello_world.wav      # Clear speech
│   │   ├── noisy.wav            # Background noise
│   │   ├── silence.wav          # Pure silence
│   │   └── rapid_speech.wav     # Fast talking
│   └── config/
│       ├── minimal.toml
│       └── full.toml
├── integration/
│   ├── queue_test.rs
│   ├── gpu_test.rs
│   └── e2e_test.rs
└── common/
    └── mod.rs                    # Shared test utilities
```
