//! macOS LaunchAgent implementation.

use super::{get_executable_path, ServiceError, ServiceStatus};
use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "org.openhush.daemon";

/// Get the LaunchAgent plist path
fn plist_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", LABEL))
}

/// Generate the LaunchAgent plist content
fn generate_plist(executable: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>start</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>StandardOutPath</key>
    <string>/tmp/openhush.out</string>
    <key>StandardErrorPath</key>
    <string>/tmp/openhush.err</string>
</dict>
</plist>
"#,
        LABEL, executable
    )
}

/// Install the LaunchAgent
pub fn install() -> Result<(), ServiceError> {
    let executable = get_executable_path()?;
    let plist = plist_path();

    // Ensure LaunchAgents directory exists
    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write plist file
    let content = generate_plist(&executable.to_string_lossy());
    std::fs::write(&plist, content)?;

    // Load the agent
    let output = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Ignore "already loaded" errors
        if !stderr.contains("already loaded") {
            return Err(ServiceError::InstallFailed(stderr.to_string()));
        }
    }

    println!("LaunchAgent installed: {}", plist.display());
    println!("OpenHush will start automatically on login.");
    Ok(())
}

/// Uninstall the LaunchAgent
pub fn uninstall() -> Result<(), ServiceError> {
    let plist = plist_path();

    if !plist.exists() {
        return Err(ServiceError::NotInstalled);
    }

    // Unload the agent first
    let _ = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&plist)
        .output();

    // Remove the plist file
    std::fs::remove_file(&plist)?;

    println!("LaunchAgent removed.");
    println!("OpenHush will no longer start automatically.");
    Ok(())
}

/// Check LaunchAgent status
pub fn status() -> Result<ServiceStatus, ServiceError> {
    let plist = plist_path();
    let installed = plist.exists();

    let running = if installed {
        // Check if the agent is loaded
        let output = Command::new("launchctl").args(["list", LABEL]).output()?;
        output.status.success()
    } else {
        false
    };

    Ok(ServiceStatus {
        installed,
        running,
        path: if installed { Some(plist) } else { None },
    })
}
