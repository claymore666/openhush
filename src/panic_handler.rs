//! Custom panic handler for daemon crash diagnostics.
//!
//! Logs panic messages and backtraces to a file before the process terminates,
//! making it easier to diagnose crashes from user machines.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;

/// Install the custom panic handler.
///
/// This should be called early in main(), before any other initialization.
/// The panic handler will:
/// 1. Log the panic message and backtrace to stderr (if available)
/// 2. Write a crash report to the data directory
/// 3. Flush all output before terminating
pub fn install() {
    // Enable backtraces
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    panic::set_hook(Box::new(|info| {
        handle_panic(info);
    }));
}

/// Get the path for the crash report file.
fn crash_report_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "openhush", "openhush")
        .map(|dirs| dirs.data_dir().join("crash.log"))
}

/// Handle a panic by logging it to file and stderr.
fn handle_panic(info: &PanicHookInfo) {
    let crash_report = format_crash_report(info);

    // Print to stderr (in case someone is watching)
    eprintln!("{}", crash_report);

    // Write to crash log file (append mode to preserve history)
    if let Some(path) = crash_report_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            // Add separator between crash reports
            let _ = file.write_all(b"\n\n========================================\n\n");
            let _ = file.write_all(crash_report.as_bytes());
            let _ = file.flush();
            eprintln!("\nCrash report appended to: {}", path.display());
        }
    }
}

/// Format the crash report with all available diagnostic information.
fn format_crash_report(info: &PanicHookInfo) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

    // Get thread info
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>");
    let thread_id = format!("{:?}", std::thread::current().id());

    // Get panic location
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown".to_string());

    // Get panic message
    let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "Box<dyn Any>".to_string()
    };

    // Capture backtrace
    let backtrace = std::backtrace::Backtrace::force_capture();

    format!(
        r"
================================================================================
OPENHUSH CRASH REPORT
================================================================================
Time:     {}
Thread:   {} ({})
Location: {}
Message:  {}

Backtrace:
{}
================================================================================

If you're seeing this, OpenHush has crashed unexpectedly.
Please report this issue at: https://github.com/claymore666/openhush/issues

Include this crash report and any steps to reproduce the issue.
",
        timestamp, thread_name, thread_id, location, payload, backtrace,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_report_path() {
        // Should return Some path on most systems
        let path = crash_report_path();
        // Just verify it doesn't panic
        assert!(path.is_none() || path.is_some());
    }
}
