//! Application context detection.
//!
//! Detects the currently focused application for app-aware configuration.
//! Supports X11, Wayland (Hyprland/Sway), macOS, and Windows.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors from context detection.
#[derive(Error, Debug)]
pub enum ContextError {
    #[error("No display server detected")]
    NoDisplayServer,

    #[error("Command failed: {0}")]
    Command(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Information about the currently focused application.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppContext {
    /// Application name/class (e.g., "firefox", "Code", "Terminal")
    pub app_name: String,
    /// Window title
    pub window_title: String,
}

impl AppContext {
    /// Create a new app context.
    pub fn new(app_name: impl Into<String>, window_title: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            window_title: window_title.into(),
        }
    }

    /// Check if this context matches an app name (case-insensitive).
    pub fn matches(&self, app_pattern: &str) -> bool {
        let pattern = app_pattern.to_lowercase();
        let app = self.app_name.to_lowercase();

        // Exact match
        if app == pattern {
            return true;
        }

        // Partial match (e.g., "code" matches "code-oss")
        if app.contains(&pattern) || pattern.contains(&app) {
            return true;
        }

        false
    }

    /// Check if this context matches any of the given app names.
    pub fn matches_any(&self, apps: &[String]) -> bool {
        apps.iter().any(|app| self.matches(app))
    }
}

/// Context detector for the current platform.
pub struct ContextDetector {
    /// Display server type
    display_server: DisplayServer,
}

/// Supported display servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    X11,
    Wayland,
    MacOS,
    Windows,
    Tty,
    Unknown,
}

impl ContextDetector {
    /// Create a new context detector, auto-detecting the display server.
    pub fn new() -> Self {
        Self {
            display_server: detect_display_server(),
        }
    }

    /// Get the detected display server.
    pub fn display_server(&self) -> DisplayServer {
        self.display_server
    }

    /// Get the currently focused application context.
    pub fn get_active_context(&self) -> Result<AppContext, ContextError> {
        match self.display_server {
            DisplayServer::X11 => get_x11_active_context(),
            DisplayServer::Wayland => get_wayland_active_context(),
            DisplayServer::MacOS => get_macos_active_context(),
            DisplayServer::Windows => get_windows_active_context(),
            DisplayServer::Tty => Ok(AppContext::new("tty", "Terminal")),
            DisplayServer::Unknown => Err(ContextError::NoDisplayServer),
        }
    }

    /// Check if context detection is supported.
    pub fn is_supported(&self) -> bool {
        !matches!(self.display_server, DisplayServer::Unknown)
    }
}

impl Default for ContextDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect the current display server.
fn detect_display_server() -> DisplayServer {
    #[cfg(target_os = "macos")]
    return DisplayServer::MacOS;

    #[cfg(target_os = "windows")]
    return DisplayServer::Windows;

    #[cfg(target_os = "linux")]
    {
        // Check for Wayland
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return DisplayServer::Wayland;
        }

        // Check for X11
        if std::env::var("DISPLAY").is_ok() {
            return DisplayServer::X11;
        }

        // Check if running in TTY
        if std::env::var("TERM").is_ok() && std::env::var("DISPLAY").is_err() {
            return DisplayServer::Tty;
        }

        DisplayServer::Unknown
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    DisplayServer::Unknown
}

// ============================================================================
// X11 Implementation
// ============================================================================

/// Get active window context on X11 using xdotool.
fn get_x11_active_context() -> Result<AppContext, ContextError> {
    // Get active window ID
    let window_id = Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
        .map_err(|e| ContextError::Command(format!("xdotool getactivewindow: {}", e)))?;

    if !window_id.status.success() {
        return Err(ContextError::Command(
            "xdotool getactivewindow failed".into(),
        ));
    }

    let window_id = String::from_utf8_lossy(&window_id.stdout)
        .trim()
        .to_string();

    // Get window class (application name)
    let class_output = Command::new("xdotool")
        .args(["getwindowclassname", &window_id])
        .output()
        .map_err(|e| ContextError::Command(format!("xdotool getwindowclassname: {}", e)))?;

    let app_name = if class_output.status.success() {
        String::from_utf8_lossy(&class_output.stdout)
            .trim()
            .to_string()
    } else {
        String::new()
    };

    // Get window title
    let title_output = Command::new("xdotool")
        .args(["getwindowname", &window_id])
        .output()
        .map_err(|e| ContextError::Command(format!("xdotool getwindowname: {}", e)))?;

    let window_title = if title_output.status.success() {
        String::from_utf8_lossy(&title_output.stdout)
            .trim()
            .to_string()
    } else {
        String::new()
    };

    debug!("X11 active window: {} - {}", app_name, window_title);

    Ok(AppContext {
        app_name,
        window_title,
    })
}

// ============================================================================
// Wayland Implementation
// ============================================================================

/// Get active window context on Wayland.
fn get_wayland_active_context() -> Result<AppContext, ContextError> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::wayland_ipc::{detect_compositor, CompositorType};

        match detect_compositor() {
            CompositorType::Hyprland => get_hyprland_active_context(),
            CompositorType::Sway => get_sway_active_context(),
            _ => {
                warn!("Unsupported Wayland compositor for context detection");
                Err(ContextError::NoDisplayServer)
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    Err(ContextError::NoDisplayServer)
}

#[cfg(target_os = "linux")]
fn get_hyprland_active_context() -> Result<AppContext, ContextError> {
    use crate::platform::wayland_ipc::hyprland_active_window;

    let info = hyprland_active_window().map_err(|e| ContextError::Command(e.to_string()))?;

    debug!("Hyprland active window: {} - {}", info.class, info.title);

    Ok(AppContext {
        app_name: info.class,
        window_title: info.title,
    })
}

#[cfg(target_os = "linux")]
fn get_sway_active_context() -> Result<AppContext, ContextError> {
    use crate::platform::wayland_ipc::sway_active_window;

    let info = sway_active_window().map_err(|e| ContextError::Command(e.to_string()))?;

    debug!("Sway active window: {} - {}", info.class, info.title);

    Ok(AppContext {
        app_name: info.class,
        window_title: info.title,
    })
}

// ============================================================================
// macOS Implementation
// ============================================================================

/// Get active window context on macOS.
fn get_macos_active_context() -> Result<AppContext, ContextError> {
    #[cfg(target_os = "macos")]
    {
        // Use osascript to get frontmost application
        let output = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of first application process whose frontmost is true",
            ])
            .output()
            .map_err(|e| ContextError::Command(format!("osascript: {}", e)))?;

        if !output.status.success() {
            return Err(ContextError::Command("osascript failed".into()));
        }

        let app_name = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Get window title
        let title_output = Command::new("osascript")
            .args([
                "-e",
                &format!(
                    "tell application \"System Events\" to get title of front window of process \"{}\"",
                    app_name
                ),
            ])
            .output()
            .ok();

        let window_title = title_output
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        debug!("macOS active window: {} - {}", app_name, window_title);

        Ok(AppContext {
            app_name,
            window_title,
        })
    }

    #[cfg(not(target_os = "macos"))]
    Err(ContextError::NoDisplayServer)
}

// ============================================================================
// Windows Implementation
// ============================================================================

/// Get active window context on Windows.
fn get_windows_active_context() -> Result<AppContext, ContextError> {
    #[cfg(target_os = "windows")]
    {
        // Use PowerShell to get active window info
        let output = Command::new("powershell")
            .args([
                "-Command",
                "(Get-Process | Where-Object {$_.MainWindowHandle -eq (Add-Type -MemberDefinition '[DllImport(\"user32.dll\")] public static extern IntPtr GetForegroundWindow();' -Name 'Win32' -Namespace 'Native' -PassThru)::GetForegroundWindow()}).ProcessName",
            ])
            .output()
            .map_err(|e| ContextError::Command(format!("powershell: {}", e)))?;

        if !output.status.success() {
            return Err(ContextError::Command("PowerShell failed".into()));
        }

        let app_name = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Get window title
        let title_output = Command::new("powershell")
            .args([
                "-Command",
                "(Get-Process | Where-Object {$_.MainWindowHandle -eq (Add-Type -MemberDefinition '[DllImport(\"user32.dll\")] public static extern IntPtr GetForegroundWindow();' -Name 'Win32' -Namespace 'Native' -PassThru)::GetForegroundWindow()}).MainWindowTitle",
            ])
            .output()
            .ok();

        let window_title = title_output
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        debug!("Windows active window: {} - {}", app_name, window_title);

        Ok(AppContext {
            app_name,
            window_title,
        })
    }

    #[cfg(not(target_os = "windows"))]
    Err(ContextError::NoDisplayServer)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // AppContext Tests
    // ===================

    #[test]
    fn test_app_context_new() {
        let ctx = AppContext::new("firefox", "Mozilla Firefox");
        assert_eq!(ctx.app_name, "firefox");
        assert_eq!(ctx.window_title, "Mozilla Firefox");
    }

    #[test]
    fn test_app_context_default() {
        let ctx = AppContext::default();
        assert!(ctx.app_name.is_empty());
        assert!(ctx.window_title.is_empty());
    }

    #[test]
    fn test_app_context_matches_exact() {
        let ctx = AppContext::new("firefox", "");
        assert!(ctx.matches("firefox"));
        assert!(ctx.matches("Firefox")); // case-insensitive
    }

    #[test]
    fn test_app_context_matches_partial() {
        let ctx = AppContext::new("code-oss", "");
        assert!(ctx.matches("code"));
        assert!(ctx.matches("Code"));
    }

    #[test]
    fn test_app_context_matches_no_match() {
        let ctx = AppContext::new("firefox", "");
        assert!(!ctx.matches("chrome"));
    }

    #[test]
    fn test_app_context_matches_any() {
        let ctx = AppContext::new("Code", "");
        let apps = vec!["vim".to_string(), "code".to_string(), "nvim".to_string()];
        assert!(ctx.matches_any(&apps));
    }

    #[test]
    fn test_app_context_matches_any_none() {
        let ctx = AppContext::new("Firefox", "");
        let apps = vec!["vim".to_string(), "code".to_string()];
        assert!(!ctx.matches_any(&apps));
    }

    #[test]
    fn test_app_context_serialize() {
        let ctx = AppContext::new("test", "Test Window");
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("Test Window"));
    }

    // ===================
    // ContextDetector Tests
    // ===================

    #[test]
    fn test_context_detector_new() {
        let detector = ContextDetector::new();
        // Just ensure it doesn't panic
        let _ = detector.display_server();
    }

    #[test]
    fn test_context_detector_default() {
        let detector = ContextDetector::default();
        let _ = detector.is_supported();
    }

    // ===================
    // ContextError Tests
    // ===================

    #[test]
    fn test_context_error_display() {
        let err = ContextError::NoDisplayServer;
        assert_eq!(format!("{}", err), "No display server detected");

        let err = ContextError::Command("test".to_string());
        assert!(format!("{}", err).contains("test"));
    }
}
