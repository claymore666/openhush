//! Windows autostart via Registry Run key.

use super::{get_executable_path, ServiceError, ServiceStatus};
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "OpenHush";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Install autostart via Registry
pub fn install() -> Result<(), ServiceError> {
    let executable = get_executable_path()?;
    let command = format!("\"{}\" start", executable.to_string_lossy());

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_WRITE)
        .map_err(|e| ServiceError::InstallFailed(e.to_string()))?;

    run_key
        .set_value(APP_NAME, &command)
        .map_err(|e| ServiceError::InstallFailed(e.to_string()))?;

    println!("Autostart enabled via Registry.");
    println!("OpenHush will start automatically on login.");
    println!("Registry key: HKCU\\{}", RUN_KEY);
    Ok(())
}

/// Remove autostart from Registry
pub fn uninstall() -> Result<(), ServiceError> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_WRITE)
        .map_err(|e| ServiceError::UninstallFailed(e.to_string()))?;

    match run_key.delete_value(APP_NAME) {
        Ok(()) => {
            println!("Autostart disabled.");
            println!("OpenHush will no longer start automatically.");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ServiceError::NotInstalled),
        Err(e) => Err(ServiceError::UninstallFailed(e.to_string())),
    }
}

/// Check autostart status
pub fn status() -> Result<ServiceStatus, ServiceError> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let (installed, path) = match hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) {
        Ok(run_key) => match run_key.get_value::<String, _>(APP_NAME) {
            Ok(value) => {
                // Extract path from command (remove quotes and "start" argument)
                let path = value
                    .trim_start_matches('"')
                    .split('"')
                    .next()
                    .map(PathBuf::from);
                (true, path)
            }
            Err(_) => (false, None),
        },
        Err(_) => (false, None),
    };

    // Check if daemon is running (try IPC)
    let running = if installed {
        use crate::ipc::{IpcClient, IpcCommand};
        match IpcClient::connect() {
            Ok(mut client) => client
                .send(IpcCommand::Status)
                .map(|r| r.ok)
                .unwrap_or(false),
            Err(_) => false,
        }
    } else {
        false
    };

    Ok(ServiceStatus {
        installed,
        running,
        path,
    })
}
