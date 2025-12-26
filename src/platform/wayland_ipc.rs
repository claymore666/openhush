//! Wayland compositor IPC integration.
//!
//! Provides integration with Hyprland and Sway compositors for:
//! - Status updates in the compositor
//! - Active window information
//! - Waybar module support

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use thiserror::Error;

/// Errors from compositor IPC operations.
#[derive(Error, Debug)]
pub enum CompositorError {
    #[error("Compositor not detected")]
    NotDetected,

    #[error("Socket not found: {0}")]
    SocketNotFound(String),

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Command failed: {0}")]
    Command(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Detected compositor type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorType {
    Hyprland,
    Sway,
    Other,
    None,
}

impl std::fmt::Display for CompositorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompositorType::Hyprland => write!(f, "Hyprland"),
            CompositorType::Sway => write!(f, "Sway"),
            CompositorType::Other => write!(f, "Other"),
            CompositorType::None => write!(f, "None"),
        }
    }
}

/// Information about the currently focused window.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window title
    pub title: String,
    /// Application class/name
    pub class: String,
    /// Process ID (if available)
    pub pid: Option<u32>,
}

/// Detect the running Wayland compositor.
pub fn detect_compositor() -> CompositorType {
    // Check for Hyprland
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return CompositorType::Hyprland;
    }

    // Check for Sway
    if std::env::var("SWAYSOCK").is_ok() {
        return CompositorType::Sway;
    }

    // Check XDG_CURRENT_DESKTOP as fallback
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();
        if desktop_lower.contains("hyprland") {
            return CompositorType::Hyprland;
        }
        if desktop_lower.contains("sway") {
            return CompositorType::Sway;
        }
        if !desktop_lower.is_empty() {
            return CompositorType::Other;
        }
    }

    CompositorType::None
}

// ============================================================================
// Hyprland IPC
// ============================================================================

/// Get the Hyprland IPC socket path.
fn hyprland_socket_path() -> Result<PathBuf, CompositorError> {
    let signature =
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").map_err(|_| CompositorError::NotDetected)?;

    // Try XDG_RUNTIME_DIR first (standard location)
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let socket = PathBuf::from(runtime_dir)
            .join("hypr")
            .join(&signature)
            .join(".socket.sock");
        if socket.exists() {
            return Ok(socket);
        }
    }

    // Fallback to /tmp
    let socket = PathBuf::from("/tmp")
        .join("hypr")
        .join(&signature)
        .join(".socket.sock");
    if socket.exists() {
        return Ok(socket);
    }

    Err(CompositorError::SocketNotFound(format!(
        "Hyprland socket not found for instance {}",
        signature
    )))
}

/// Send a command to Hyprland and get the response.
fn hyprland_command(cmd: &str) -> Result<String, CompositorError> {
    let socket_path = hyprland_socket_path()?;

    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| CompositorError::Connection(e.to_string()))?;

    stream
        .write_all(cmd.as_bytes())
        .map_err(|e| CompositorError::Command(e.to_string()))?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .map_err(|e| CompositorError::Command(e.to_string()))?;

    Ok(response)
}

/// Get the active window info from Hyprland.
pub fn hyprland_active_window() -> Result<WindowInfo, CompositorError> {
    let response = hyprland_command("activewindow")?;

    // Parse response: "Window <addr> -> <title>:\n\tat: ...\n\tsize: ...\n\tclass: ..."
    let mut info = WindowInfo::default();

    for line in response.lines() {
        let line = line.trim();
        if line.starts_with("Window") {
            // Extract title from "Window <addr> -> <title>:"
            if let Some(arrow_pos) = line.find("->") {
                let title_part = &line[arrow_pos + 2..].trim();
                if let Some(colon_pos) = title_part.rfind(':') {
                    info.title = title_part[..colon_pos].trim().to_string();
                }
            }
        } else if let Some(class) = line.strip_prefix("class:") {
            info.class = class.trim().to_string();
        } else if let Some(pid) = line.strip_prefix("pid:") {
            info.pid = pid.trim().parse().ok();
        }
    }

    Ok(info)
}

/// Execute a Hyprland dispatch command.
#[allow(dead_code)]
pub fn hyprland_dispatch(cmd: &str) -> Result<(), CompositorError> {
    let full_cmd = format!("dispatch {}", cmd);
    hyprland_command(&full_cmd)?;
    Ok(())
}

/// Send a notification via Hyprland (uses notify-send internally).
#[allow(dead_code)]
pub fn hyprland_notify(title: &str, body: &str) -> Result<(), CompositorError> {
    hyprland_dispatch(&format!("exec notify-send '{}' '{}'", title, body))
}

// ============================================================================
// Sway IPC
// ============================================================================

/// Sway IPC message types.
#[repr(u32)]
enum SwayMsgType {
    RunCommand = 0,
    GetWorkspaces = 1,
    Subscribe = 2,
    GetOutputs = 3,
    GetTree = 4,
    GetMarks = 5,
    GetBarConfig = 6,
    GetVersion = 7,
    GetBindingModes = 8,
    GetConfig = 9,
    SendTick = 10,
    Sync = 11,
    GetBindingState = 12,
    GetInputs = 100,
    GetSeats = 101,
}

/// Get the Sway IPC socket path.
fn sway_socket_path() -> Result<PathBuf, CompositorError> {
    std::env::var("SWAYSOCK")
        .map(PathBuf::from)
        .map_err(|_| CompositorError::NotDetected)
}

/// Send a message to Sway IPC and get the response.
fn sway_ipc(msg_type: SwayMsgType, payload: &str) -> Result<String, CompositorError> {
    let socket_path = sway_socket_path()?;

    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| CompositorError::Connection(e.to_string()))?;

    // Sway IPC message format:
    // - Magic string: "i3-ipc" (6 bytes)
    // - Payload length: u32 (4 bytes, little-endian)
    // - Message type: u32 (4 bytes, little-endian)
    // - Payload: UTF-8 string

    let payload_bytes = payload.as_bytes();
    let mut message = Vec::with_capacity(14 + payload_bytes.len());
    message.extend_from_slice(b"i3-ipc");
    message.extend_from_slice(&(payload_bytes.len() as u32).to_le_bytes());
    message.extend_from_slice(&(msg_type as u32).to_le_bytes());
    message.extend_from_slice(payload_bytes);

    stream
        .write_all(&message)
        .map_err(|e| CompositorError::Command(e.to_string()))?;

    // Read response header
    let mut header = [0u8; 14];
    std::io::Read::read_exact(&mut stream, &mut header)
        .map_err(|e| CompositorError::Command(e.to_string()))?;

    // Parse payload length
    let payload_len = u32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;

    // Read payload
    let mut payload = vec![0u8; payload_len];
    std::io::Read::read_exact(&mut stream, &mut payload)
        .map_err(|e| CompositorError::Command(e.to_string()))?;

    String::from_utf8(payload).map_err(|e| CompositorError::Parse(e.to_string()))
}

/// Get the active window info from Sway.
pub fn sway_active_window() -> Result<WindowInfo, CompositorError> {
    let response = sway_ipc(SwayMsgType::GetTree, "")?;

    // Parse JSON response to find focused window
    let tree: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| CompositorError::Parse(e.to_string()))?;

    fn find_focused(node: &serde_json::Value) -> Option<WindowInfo> {
        // Check if this node is a focused container
        if node.get("focused").and_then(|v| v.as_bool()) == Some(true)
            && node.get("type").and_then(|v| v.as_str()) == Some("con")
        {
            return Some(WindowInfo {
                title: node
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                class: node
                    .get("app_id")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        node.get("window_properties")
                            .and_then(|wp| wp.get("class"))
                            .and_then(|v| v.as_str())
                    })
                    .unwrap_or("")
                    .to_string(),
                pid: node.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32),
            });
        }

        // Recursively search children
        if let Some(nodes) = node.get("nodes").and_then(|v| v.as_array()) {
            for child in nodes {
                if let Some(info) = find_focused(child) {
                    return Some(info);
                }
            }
        }
        if let Some(nodes) = node.get("floating_nodes").and_then(|v| v.as_array()) {
            for child in nodes {
                if let Some(info) = find_focused(child) {
                    return Some(info);
                }
            }
        }

        None
    }

    find_focused(&tree).ok_or_else(|| CompositorError::Parse("No focused window found".to_string()))
}

/// Run a Sway command.
#[allow(dead_code)]
pub fn sway_command(cmd: &str) -> Result<String, CompositorError> {
    sway_ipc(SwayMsgType::RunCommand, cmd)
}

// ============================================================================
// Unified API
// ============================================================================

/// Compositor IPC client.
pub struct CompositorClient {
    compositor: CompositorType,
}

impl CompositorClient {
    /// Create a new compositor client, auto-detecting the compositor.
    pub fn new() -> Self {
        Self {
            compositor: detect_compositor(),
        }
    }

    /// Get the detected compositor type.
    pub fn compositor(&self) -> CompositorType {
        self.compositor
    }

    /// Check if a supported compositor is detected.
    pub fn is_supported(&self) -> bool {
        matches!(
            self.compositor,
            CompositorType::Hyprland | CompositorType::Sway
        )
    }

    /// Get information about the currently focused window.
    pub fn active_window(&self) -> Result<WindowInfo, CompositorError> {
        match self.compositor {
            CompositorType::Hyprland => hyprland_active_window(),
            CompositorType::Sway => sway_active_window(),
            _ => Err(CompositorError::NotDetected),
        }
    }
}

impl Default for CompositorClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Waybar Module Support
// ============================================================================

/// Status for Waybar custom module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaybarStatus {
    /// Text to display
    pub text: String,
    /// Tooltip text
    pub tooltip: String,
    /// CSS class
    pub class: String,
    /// Percentage (0-100) for progress bars
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u32>,
}

impl WaybarStatus {
    /// Create an idle status.
    pub fn idle() -> Self {
        Self {
            text: "".to_string(),
            tooltip: "OpenHush: Ready".to_string(),
            class: "idle".to_string(),
            percentage: None,
        }
    }

    /// Create a recording status.
    pub fn recording(duration_secs: f32) -> Self {
        Self {
            text: format!("{:.0}s", duration_secs),
            tooltip: format!("OpenHush: Recording ({:.1}s)", duration_secs),
            class: "recording".to_string(),
            percentage: None,
        }
    }

    /// Create a transcribing status.
    pub fn transcribing() -> Self {
        Self {
            text: "...".to_string(),
            tooltip: "OpenHush: Transcribing".to_string(),
            class: "transcribing".to_string(),
            percentage: None,
        }
    }

    /// Create an error status.
    pub fn error(msg: &str) -> Self {
        Self {
            text: "!".to_string(),
            tooltip: format!("OpenHush Error: {}", msg),
            class: "error".to_string(),
            percentage: None,
        }
    }

    /// Output as JSON for Waybar.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Print a status line for Waybar polling.
pub fn print_waybar_status(status: &WaybarStatus) {
    println!("{}", status.to_json());
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // CompositorType Tests
    // ===================

    #[test]
    fn test_compositor_type_display() {
        assert_eq!(format!("{}", CompositorType::Hyprland), "Hyprland");
        assert_eq!(format!("{}", CompositorType::Sway), "Sway");
        assert_eq!(format!("{}", CompositorType::Other), "Other");
        assert_eq!(format!("{}", CompositorType::None), "None");
    }

    #[test]
    fn test_compositor_type_eq() {
        assert_eq!(CompositorType::Hyprland, CompositorType::Hyprland);
        assert_ne!(CompositorType::Hyprland, CompositorType::Sway);
    }

    // ===================
    // WindowInfo Tests
    // ===================

    #[test]
    fn test_window_info_default() {
        let info = WindowInfo::default();
        assert!(info.title.is_empty());
        assert!(info.class.is_empty());
        assert!(info.pid.is_none());
    }

    #[test]
    fn test_window_info_serialize() {
        let info = WindowInfo {
            title: "Test Window".to_string(),
            class: "test-app".to_string(),
            pid: Some(12345),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("Test Window"));
        assert!(json.contains("test-app"));
        assert!(json.contains("12345"));
    }

    // ===================
    // WaybarStatus Tests
    // ===================

    #[test]
    fn test_waybar_status_idle() {
        let status = WaybarStatus::idle();
        assert_eq!(status.class, "idle");
        assert!(status.text.is_empty());
    }

    #[test]
    fn test_waybar_status_recording() {
        let status = WaybarStatus::recording(5.5);
        assert_eq!(status.class, "recording");
        assert_eq!(status.text, "6s"); // Rounded
        assert!(status.tooltip.contains("5.5"));
    }

    #[test]
    fn test_waybar_status_transcribing() {
        let status = WaybarStatus::transcribing();
        assert_eq!(status.class, "transcribing");
        assert_eq!(status.text, "...");
    }

    #[test]
    fn test_waybar_status_error() {
        let status = WaybarStatus::error("Test error");
        assert_eq!(status.class, "error");
        assert!(status.tooltip.contains("Test error"));
    }

    #[test]
    fn test_waybar_status_to_json() {
        let status = WaybarStatus::idle();
        let json = status.to_json();
        assert!(json.contains("\"class\":\"idle\""));
        assert!(json.contains("\"tooltip\":"));
    }

    // ===================
    // CompositorClient Tests
    // ===================

    #[test]
    fn test_compositor_client_new() {
        let client = CompositorClient::new();
        // Just verify it doesn't panic - actual compositor depends on environment
        let _ = client.compositor();
    }

    #[test]
    fn test_compositor_client_default() {
        let client = CompositorClient::default();
        let _ = client.is_supported();
    }

    // ===================
    // Detection Tests
    // ===================

    #[test]
    fn test_detect_compositor_no_env() {
        // This test depends on the environment - just ensure no panic
        let _ = detect_compositor();
    }

    // ===================
    // Error Tests
    // ===================

    #[test]
    fn test_compositor_error_display() {
        let err = CompositorError::NotDetected;
        assert_eq!(format!("{}", err), "Compositor not detected");

        let err = CompositorError::SocketNotFound("test".to_string());
        assert!(format!("{}", err).contains("test"));
    }
}
