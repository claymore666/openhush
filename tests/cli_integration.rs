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
