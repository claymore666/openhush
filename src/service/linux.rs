//! Linux systemd user service implementation.

use super::{get_executable_path, ServiceError, ServiceStatus};
use std::path::PathBuf;
use std::process::Command;

const SERVICE_NAME: &str = "openhush";

/// Get the systemd user service directory
fn service_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("systemd/user")
}

/// Get the service file path
fn service_path() -> PathBuf {
    service_dir().join(format!("{}.service", SERVICE_NAME))
}

/// Generate the systemd service unit file
fn generate_service(executable: &str) -> String {
    format!(
        r#"[Unit]
Description=OpenHush Voice-to-Text Daemon
Documentation=https://github.com/claymore666/openhush
After=graphical-session.target

[Service]
Type=simple
ExecStart={} start -f
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
        executable
    )
}

/// Install the systemd user service
pub fn install() -> Result<(), ServiceError> {
    let executable = get_executable_path()?;
    let service_file = service_path();

    // Ensure directory exists
    let dir = service_dir();
    std::fs::create_dir_all(&dir)?;

    // Write service file
    let content = generate_service(&executable.to_string_lossy());
    std::fs::write(&service_file, content)?;

    // Reload systemd
    let output = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServiceError::InstallFailed(format!(
            "daemon-reload failed: {}",
            stderr
        )));
    }

    // Enable the service
    let output = Command::new("systemctl")
        .args(["--user", "enable", SERVICE_NAME])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServiceError::InstallFailed(format!(
            "enable failed: {}",
            stderr
        )));
    }

    println!("Systemd user service installed: {}", service_file.display());
    println!("OpenHush will start automatically on login.");
    println!("\nTo start now: systemctl --user start {}", SERVICE_NAME);
    Ok(())
}

/// Uninstall the systemd user service
pub fn uninstall() -> Result<(), ServiceError> {
    let service_file = service_path();

    if !service_file.exists() {
        return Err(ServiceError::NotInstalled);
    }

    // Stop the service if running
    let _ = Command::new("systemctl")
        .args(["--user", "stop", SERVICE_NAME])
        .output();

    // Disable the service
    let _ = Command::new("systemctl")
        .args(["--user", "disable", SERVICE_NAME])
        .output();

    // Remove service file
    std::fs::remove_file(&service_file)?;

    // Reload systemd
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();

    println!("Systemd user service removed.");
    println!("OpenHush will no longer start automatically.");
    Ok(())
}

/// Check systemd service status
pub fn status() -> Result<ServiceStatus, ServiceError> {
    let service_file = service_path();
    let installed = service_file.exists();

    let running = if installed {
        let output = Command::new("systemctl")
            .args(["--user", "is-active", SERVICE_NAME])
            .output()?;
        output.status.success()
    } else {
        false
    };

    Ok(ServiceStatus {
        installed,
        running,
        path: if installed { Some(service_file) } else { None },
    })
}
