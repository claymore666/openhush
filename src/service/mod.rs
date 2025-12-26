//! Cross-platform service/autostart management.
//!
//! - Linux: systemd user service
//! - macOS: LaunchAgent
//! - Windows: Registry Run key

use std::path::PathBuf;
use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(target_os = "windows")]
pub use windows::*;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Failed to install service: {0}")]
    InstallFailed(String),

    #[error("Failed to uninstall service: {0}")]
    #[allow(dead_code)]
    UninstallFailed(String),

    #[error("Service not installed")]
    NotInstalled,

    #[error("Could not find executable path")]
    ExecutableNotFound,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Permission denied: {0}")]
    #[allow(dead_code)]
    PermissionDenied(String),
}

/// Service status information
#[derive(Debug)]
pub struct ServiceStatus {
    pub installed: bool,
    pub running: bool,
    pub path: Option<PathBuf>,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.installed {
            writeln!(f, "Service: installed")?;
            if let Some(ref path) = self.path {
                writeln!(f, "Location: {}", path.display())?;
            }
            writeln!(f, "Running: {}", if self.running { "yes" } else { "no" })?;
        } else {
            writeln!(f, "Service: not installed")?;
        }
        Ok(())
    }
}

/// Get the path to the current executable
pub fn get_executable_path() -> Result<PathBuf, ServiceError> {
    std::env::current_exe().map_err(|_| ServiceError::ExecutableNotFound)
}
