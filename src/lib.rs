//! OpenHush library exports for testing and fuzzing.
//!
//! This module re-exports internal types for use by fuzz targets
//! and integration tests.

pub mod api;
pub mod config;
pub mod correction;
pub mod daemon;
#[cfg(target_os = "linux")]
pub mod dbus;
pub mod engine;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod gui;
pub mod input;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod ipc;
pub mod output;
pub mod panic_handler;
pub mod platform;
pub mod queue;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod service;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod tray;
pub mod vad;
pub mod vocabulary;

// Re-export commonly used types for convenience
pub use config::Config;
pub use engine::validation::validate_audio;
pub use input::audio::AudioBuffer;
pub use input::ring_buffer::AudioRingBuffer;
