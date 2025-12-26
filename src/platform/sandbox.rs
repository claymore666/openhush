//! Sandbox detection utilities.
//!
//! Detects if OpenHush is running inside a security sandbox (AppArmor, SELinux,
//! Firejail, Flatpak, etc.) and provides utilities for sandbox-aware behavior.

use std::path::Path;
use tracing::{debug, info};

/// Types of sandboxes that can be detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxType {
    /// Flatpak container
    Flatpak,
    /// Firejail sandbox
    Firejail,
    /// AppArmor profile (Linux)
    AppArmor,
    /// SELinux confined domain (Linux)
    SELinux,
    /// Snap package sandbox
    Snap,
    /// bubblewrap (used by Flatpak, but also standalone)
    Bubblewrap,
    /// systemd-nspawn container
    SystemdNspawn,
    /// Docker/Podman container
    Container,
    /// No sandbox detected
    None,
}

impl SandboxType {
    /// Returns a human-readable name for the sandbox type.
    pub fn name(&self) -> &'static str {
        match self {
            SandboxType::Flatpak => "Flatpak",
            SandboxType::Firejail => "Firejail",
            SandboxType::AppArmor => "AppArmor",
            SandboxType::SELinux => "SELinux",
            SandboxType::Snap => "Snap",
            SandboxType::Bubblewrap => "bubblewrap",
            SandboxType::SystemdNspawn => "systemd-nspawn",
            SandboxType::Container => "Container",
            SandboxType::None => "None",
        }
    }

    /// Returns true if this is a sandbox (not None).
    pub fn is_sandboxed(&self) -> bool {
        !matches!(self, SandboxType::None)
    }
}

/// Detect if running inside a Flatpak container.
fn detect_flatpak() -> bool {
    // Flatpak sets FLATPAK_ID environment variable
    if std::env::var("FLATPAK_ID").is_ok() {
        return true;
    }
    // Also check for /.flatpak-info file
    Path::new("/.flatpak-info").exists()
}

/// Detect if running inside Firejail.
fn detect_firejail() -> bool {
    // Firejail creates /run/firejail directory
    if Path::new("/run/firejail").exists() {
        return true;
    }
    // Check for firejail in process tree
    if let Ok(status) = std::fs::read_to_string("/proc/1/comm") {
        if status.trim() == "firejail" {
            return true;
        }
    }
    false
}

/// Detect if running under AppArmor confinement.
fn detect_apparmor() -> bool {
    // Check /proc/self/attr/current for AppArmor profile
    if let Ok(profile) = std::fs::read_to_string("/proc/self/attr/current") {
        let profile = profile.trim();
        // "unconfined" means no profile, anything else means confined
        if !profile.is_empty() && profile != "unconfined" {
            debug!("AppArmor profile detected: {}", profile);
            return true;
        }
    }
    false
}

/// Detect if running under SELinux confinement.
fn detect_selinux() -> bool {
    // Check if SELinux is enabled
    if !Path::new("/sys/fs/selinux").exists() {
        return false;
    }

    // Check current context
    if let Ok(context) = std::fs::read_to_string("/proc/self/attr/current") {
        let context = context.trim();
        // Check if we're in a confined domain (not unconfined_t)
        if !context.is_empty() && !context.contains("unconfined_t") {
            debug!("SELinux context detected: {}", context);
            return true;
        }
    }
    false
}

/// Detect if running inside a Snap package.
fn detect_snap() -> bool {
    // Snap sets SNAP environment variable
    std::env::var("SNAP").is_ok()
}

/// Detect if running inside bubblewrap.
fn detect_bubblewrap() -> bool {
    // bubblewrap creates /.bwrap directory or sets up specific namespaces
    if Path::new("/.bwrap").exists() {
        return true;
    }
    // Check for bwrap in parent processes
    if let Ok(cmdline) = std::fs::read_to_string("/proc/1/cmdline") {
        if cmdline.contains("bwrap") {
            return true;
        }
    }
    false
}

/// Detect if running inside systemd-nspawn.
fn detect_systemd_nspawn() -> bool {
    // systemd-nspawn sets container=systemd-nspawn
    if let Ok(container) = std::env::var("container") {
        if container == "systemd-nspawn" {
            return true;
        }
    }
    false
}

/// Detect if running inside a Docker/Podman container.
fn detect_container() -> bool {
    // Check for /.dockerenv
    if Path::new("/.dockerenv").exists() {
        return true;
    }
    // Check for container environment variable (set by podman)
    if std::env::var("container").is_ok() {
        return true;
    }
    // Check cgroup for docker/lxc
    if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
        if cgroup.contains("docker") || cgroup.contains("lxc") || cgroup.contains("kubepods") {
            return true;
        }
    }
    false
}

/// Detect the current sandbox environment.
///
/// Checks for various sandboxing mechanisms in order of specificity.
/// Returns the most specific sandbox type detected, or `SandboxType::None`.
///
/// # Example
///
/// ```no_run
/// use openhush::platform::sandbox::detect_sandbox;
///
/// let sandbox = detect_sandbox();
/// if sandbox.is_sandboxed() {
///     println!("Running in {} sandbox", sandbox.name());
/// }
/// ```
pub fn detect_sandbox() -> SandboxType {
    // Check in order of specificity (most specific first)

    // Flatpak (also uses bubblewrap, but Flatpak is more specific)
    if detect_flatpak() {
        info!("Sandbox detected: Flatpak");
        return SandboxType::Flatpak;
    }

    // Snap
    if detect_snap() {
        info!("Sandbox detected: Snap");
        return SandboxType::Snap;
    }

    // Firejail
    if detect_firejail() {
        info!("Sandbox detected: Firejail");
        return SandboxType::Firejail;
    }

    // systemd-nspawn
    if detect_systemd_nspawn() {
        info!("Sandbox detected: systemd-nspawn");
        return SandboxType::SystemdNspawn;
    }

    // bubblewrap (standalone, not Flatpak)
    if detect_bubblewrap() {
        info!("Sandbox detected: bubblewrap");
        return SandboxType::Bubblewrap;
    }

    // Docker/Podman container
    if detect_container() {
        info!("Sandbox detected: Container (Docker/Podman/LXC)");
        return SandboxType::Container;
    }

    // AppArmor (can be combined with other sandboxes)
    if detect_apparmor() {
        info!("Sandbox detected: AppArmor");
        return SandboxType::AppArmor;
    }

    // SELinux (can be combined with other sandboxes)
    if detect_selinux() {
        info!("Sandbox detected: SELinux");
        return SandboxType::SELinux;
    }

    debug!("No sandbox detected");
    SandboxType::None
}

/// Log sandbox status at startup.
///
/// Call this during daemon initialization to log the current sandbox state.
pub fn log_sandbox_status() {
    let sandbox = detect_sandbox();
    if sandbox.is_sandboxed() {
        info!(
            "Running in {} sandbox - security profile active",
            sandbox.name()
        );
    } else {
        debug!("No sandbox detected - consider using AppArmor/Firejail for security");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_type_name() {
        assert_eq!(SandboxType::Flatpak.name(), "Flatpak");
        assert_eq!(SandboxType::Firejail.name(), "Firejail");
        assert_eq!(SandboxType::AppArmor.name(), "AppArmor");
        assert_eq!(SandboxType::SELinux.name(), "SELinux");
        assert_eq!(SandboxType::Snap.name(), "Snap");
        assert_eq!(SandboxType::Bubblewrap.name(), "bubblewrap");
        assert_eq!(SandboxType::SystemdNspawn.name(), "systemd-nspawn");
        assert_eq!(SandboxType::Container.name(), "Container");
        assert_eq!(SandboxType::None.name(), "None");
    }

    #[test]
    fn test_sandbox_type_is_sandboxed() {
        assert!(SandboxType::Flatpak.is_sandboxed());
        assert!(SandboxType::Firejail.is_sandboxed());
        assert!(SandboxType::AppArmor.is_sandboxed());
        assert!(SandboxType::SELinux.is_sandboxed());
        assert!(!SandboxType::None.is_sandboxed());
    }

    #[test]
    fn test_sandbox_type_equality() {
        assert_eq!(SandboxType::Flatpak, SandboxType::Flatpak);
        assert_ne!(SandboxType::Flatpak, SandboxType::Firejail);
    }

    #[test]
    fn test_sandbox_type_debug() {
        assert_eq!(format!("{:?}", SandboxType::AppArmor), "AppArmor");
    }

    #[test]
    fn test_sandbox_type_clone() {
        let sandbox = SandboxType::SELinux;
        let cloned = sandbox;
        assert_eq!(sandbox, cloned);
    }

    #[test]
    fn test_detect_sandbox_returns_valid() {
        // Just verify it returns a valid type without panicking
        let sandbox = detect_sandbox();
        let _ = sandbox.name();
        let _ = sandbox.is_sandboxed();
    }
}
