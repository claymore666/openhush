//! System tray icon and menu.
//!
//! Provides a system tray icon with menu for daemon control.
//! Gracefully degrades on unsupported platforms (GNOME Wayland, TTY).

use std::sync::mpsc::{self, Receiver};
use thiserror::Error;
use tracing::{debug, info, warn};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

mod icon;

pub use icon::create_icon;

#[derive(Error, Debug)]
pub enum TrayError {
    #[error("Failed to create tray icon: {0}")]
    IconCreation(String),

    #[error("Failed to create menu: {0}")]
    MenuCreation(String),

    #[error("Failed to build tray: {0}")]
    TrayBuild(String),

    #[error("System tray not supported on this platform")]
    NotSupported,

    #[error("System tray not available (GNOME Wayland without AppIndicator?)")]
    #[allow(dead_code)]
    NotAvailable,
}

/// Events from the system tray menu
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// User clicked "Preferences..."
    ShowPreferences,
    /// User clicked "Quit"
    Quit,
    /// Status item was clicked (informational)
    StatusClicked,
}

/// Manages the system tray icon and menu
pub struct TrayManager {
    #[allow(dead_code)]
    tray: TrayIcon,
    event_rx: Receiver<TrayEvent>,
    #[allow(dead_code)]
    status_item_id: tray_icon::menu::MenuId,
}

impl TrayManager {
    /// Create a new tray manager
    ///
    /// Returns an error if the system tray is not available.
    pub fn new() -> Result<Self, TrayError> {
        // Check if we're in a TTY (no display)
        #[cfg(target_os = "linux")]
        {
            if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
                return Err(TrayError::NotSupported);
            }
        }

        let (event_tx, event_rx) = mpsc::channel();

        // Create menu items
        let status_item = MenuItem::new("Status: Idle", false, None);
        let status_item_id = status_item.id().clone();

        let preferences_item = MenuItem::new("Preferences...", true, None);
        let preferences_item_id = preferences_item.id().clone();

        let quit_item = MenuItem::new("Quit", true, None);
        let quit_item_id = quit_item.id().clone();

        // Build menu
        let menu = Menu::new();
        menu.append(&status_item)
            .map_err(|e| TrayError::MenuCreation(e.to_string()))?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| TrayError::MenuCreation(e.to_string()))?;
        menu.append(&preferences_item)
            .map_err(|e| TrayError::MenuCreation(e.to_string()))?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| TrayError::MenuCreation(e.to_string()))?;
        menu.append(&quit_item)
            .map_err(|e| TrayError::MenuCreation(e.to_string()))?;

        // Create icon
        let icon = create_icon().map_err(TrayError::IconCreation)?;

        // Build tray icon
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("OpenHush - Voice to Text")
            .with_icon(icon)
            .build()
            .map_err(|e| TrayError::TrayBuild(e.to_string()))?;

        // Set up menu event handler
        let tx = event_tx.clone();
        let status_id_for_thread = status_item_id.clone();
        std::thread::spawn(move || loop {
            if let Ok(event) = MenuEvent::receiver().recv() {
                let tray_event = if event.id == preferences_item_id {
                    Some(TrayEvent::ShowPreferences)
                } else if event.id == quit_item_id {
                    Some(TrayEvent::Quit)
                } else if event.id == status_id_for_thread {
                    Some(TrayEvent::StatusClicked)
                } else {
                    None
                };

                if let Some(e) = tray_event {
                    debug!("Tray menu event: {:?}", e);
                    if tx.send(e).is_err() {
                        break;
                    }
                }
            }
        });

        info!("System tray initialized");
        Ok(Self {
            tray,
            event_rx,
            status_item_id,
        })
    }

    /// Try to receive a tray event (non-blocking)
    pub fn try_recv(&self) -> Option<TrayEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get a reference to the event receiver
    #[allow(dead_code)]
    pub fn event_receiver(&self) -> &Receiver<TrayEvent> {
        &self.event_rx
    }

    /// Update the status text in the menu
    #[allow(dead_code)]
    pub fn update_status(&self, status: &str) {
        // Note: tray-icon doesn't easily support updating menu items after creation
        // This would require recreating the menu or using platform-specific APIs
        debug!("Status update requested: {}", status);
    }
}

/// Check if system tray is likely to be supported
#[allow(dead_code)]
pub fn is_tray_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check for display server
        let has_display =
            std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();

        if !has_display {
            return false;
        }

        // On GNOME Wayland, tray may not work without AppIndicator extension
        if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
            if desktop.to_lowercase().contains("gnome") && std::env::var("WAYLAND_DISPLAY").is_ok()
            {
                warn!("GNOME Wayland detected - system tray may require AppIndicator extension");
            }
        }

        true
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}
