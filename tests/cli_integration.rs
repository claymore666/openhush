//! Integration tests for CLI commands.
//!
//! These tests verify that CLI commands work correctly without
//! requiring a running daemon or audio hardware.

use assert_cmd::Command;
use predicates::prelude::*;

/// Get a Command for the openhush binary
fn openhush() -> Command {
    Command::cargo_bin("openhush").unwrap()
}

#[test]
fn test_help_command() {
    openhush()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Voice-to-text whisper keyboard"))
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("config"));
}

#[test]
fn test_version_command() {
    openhush()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("openhush"));
}

#[test]
fn test_config_show() {
    // Should work even without an existing config (uses defaults)
    openhush()
        .args(["config", "--show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hotkey"))
        .stdout(predicate::str::contains("model"));
}

#[test]
fn test_model_list() {
    openhush()
        .args(["model", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tiny"))
        .stdout(predicate::str::contains("base"))
        .stdout(predicate::str::contains("small"))
        .stdout(predicate::str::contains("medium"))
        .stdout(predicate::str::contains("largev3")); // Display name is lowercased without hyphen
}

#[test]
fn test_status_no_daemon() {
    // When no daemon is running, status should indicate that
    openhush()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("not running").or(predicate::str::contains("No PID")));
}

#[test]
fn test_stop_no_daemon() {
    // Stopping when no daemon is running returns error
    openhush()
        .arg("stop")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not running"));
}

#[test]
fn test_invalid_model_download() {
    // Trying to download an invalid model should fail with helpful message
    openhush()
        .args(["model", "download", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown model"));
}

#[test]
fn test_invalid_model_remove() {
    // Trying to remove an invalid model should fail with helpful message
    openhush()
        .args(["model", "remove", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown model"));
}

#[test]
fn test_config_set_hotkey() {
    // Setting a valid hotkey should succeed
    openhush()
        .args(["config", "--hotkey", "f12"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration updated"));
}

#[test]
fn test_start_help() {
    openhush()
        .args(["start", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--foreground"))
        .stdout(predicate::str::contains("--no-tray"));
}

// ===================
// Secret Management Tests
// ===================

#[test]
fn test_secret_help() {
    openhush()
        .args(["secret", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("set"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("show"));
}

#[test]
fn test_secret_list() {
    // List command should work and show helpful message
    openhush()
        .args(["secret", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("keyring"));
}

#[test]
fn test_secret_check() {
    // Check command shows keyring availability
    openhush()
        .args(["secret", "check"])
        .assert()
        // May succeed or fail depending on system keyring
        .stdout(predicate::str::contains("Keyring").or(predicate::str::contains("keyring")));
}

#[test]
fn test_secret_show_nonexistent() {
    // Showing a non-existent secret with --force should fail
    openhush()
        .args(["secret", "show", "nonexistent-test-secret-12345", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("error")));
}

#[test]
fn test_secret_delete_nonexistent() {
    // Deleting a non-existent secret should fail
    openhush()
        .args(["secret", "delete", "nonexistent-test-secret-12345"])
        .assert()
        .failure();
}

// ===================
// Service Management Tests
// ===================

#[test]
fn test_service_help() {
    openhush()
        .args(["service", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("uninstall"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn test_service_status() {
    // Service status should work
    openhush()
        .args(["service", "status"])
        .assert()
        .success();
}

// ===================
// Preferences GUI Tests
// ===================

#[test]
fn test_preferences_help() {
    openhush()
        .args(["preferences", "--help"])
        .assert()
        .success();
}

// ===================
// Recording Control Tests
// ===================

#[test]
fn test_recording_help() {
    openhush()
        .args(["recording", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("toggle"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn test_recording_status_no_daemon() {
    // Recording status when daemon not running
    openhush()
        .args(["recording", "status"])
        .assert()
        // Should fail gracefully when daemon not running
        .failure();
}

// ===================
// Config Wake Word Tests
// ===================

#[test]
fn test_config_shows_wake_word() {
    openhush()
        .args(["config", "--show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wake_word"));
}

// ===================
// Transcribe Tests
// ===================

#[test]
fn test_transcribe_help() {
    openhush()
        .args(["transcribe", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--model"))
        .stdout(predicate::str::contains("--output"));
}

#[test]
fn test_transcribe_missing_file() {
    openhush()
        .args(["transcribe", "nonexistent.wav"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("error")));
}
