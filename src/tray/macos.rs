//! macOS menu bar (system tray) implementation using tray-icon.
//!
//! Uses the tray-icon crate which provides cross-platform support.
//! On macOS, the tray icon appears in the menu bar at the top of the screen.

use super::{icon, TrayError, TrayEvent, TrayStatus};
use std::sync::mpsc::{self, Receiver};
use tracing::{debug, info, warn};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// Menu item IDs
const MENU_STATUS: &str = "status";
const MENU_PREFERENCES: &str = "preferences";
const MENU_QUIT: &str = "quit";

/// Manages the menu bar icon and menu on macOS
pub struct TrayManager {
    #[allow(dead_code)]
    tray_icon: TrayIcon,
    event_rx: Receiver<TrayEvent>,
    status_item: MenuItem,
    status: TrayStatus,
}

impl TrayManager {
    /// Create a new tray manager
    ///
    /// Note: On macOS, the menu bar item appears at the top right of the screen.
    pub async fn new() -> Result<Self, TrayError> {
        let (event_tx, event_rx) = mpsc::channel();

        // Create menu
        let status_item = MenuItem::with_id(MENU_STATUS, "Status: Idle", false, None);
        let preferences_item = MenuItem::with_id(MENU_PREFERENCES, "Preferences...", true, None);
        let quit_item = MenuItem::with_id(MENU_QUIT, "Quit OpenHush", true, None);

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

        // Create icon from embedded data
        let icon_data = icon::ICON_DATA;
        let icon = Icon::from_rgba(icon_data.to_vec(), icon::ICON_WIDTH, icon::ICON_HEIGHT)
            .map_err(|e| TrayError::IconCreation(e.to_string()))?;

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("OpenHush - Voice to Text")
            .with_icon(icon)
            .with_title("OpenHush") // macOS-specific: shows text next to icon
            .build()
            .map_err(|e| TrayError::TrayBuild(e.to_string()))?;

        // Set up menu event handling
        let menu_rx = MenuEvent::receiver().clone();

        // Spawn menu event handler thread
        let event_tx_clone = event_tx.clone();
        std::thread::spawn(move || loop {
            if let Ok(event) = menu_rx.recv() {
                match event.id.0.as_str() {
                    MENU_PREFERENCES => {
                        debug!("Menu bar: Preferences clicked");
                        let _ = event_tx_clone.send(TrayEvent::ShowPreferences);
                    }
                    MENU_QUIT => {
                        debug!("Menu bar: Quit clicked");
                        let _ = event_tx_clone.send(TrayEvent::Quit);
                    }
                    MENU_STATUS => {
                        debug!("Menu bar: Status clicked");
                        let _ = event_tx_clone.send(TrayEvent::StatusClicked);
                    }
                    _ => {}
                }
            }
        });

        info!("Menu bar initialized (macOS tray-icon)");
        Ok(Self {
            tray_icon,
            event_rx,
            status_item,
            status: TrayStatus::Idle,
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

    /// Update the status displayed in the menu bar
    #[allow(dead_code)]
    pub fn update_status(&self, status: &str) {
        debug!("Updating menu bar status: {}", status);
        let new_status = match status {
            s if s.contains("Recording") => TrayStatus::Recording,
            s if s.contains("Processing") => TrayStatus::Processing,
            s if s.contains("Error") => TrayStatus::Error,
            _ => TrayStatus::Idle,
        };

        self.status_item.set_text(new_status.as_str());
        if let Err(e) = self
            .tray_icon
            .set_tooltip(Some(format!("OpenHush - {}", new_status.as_str())))
        {
            warn!("Failed to update menu bar tooltip: {}", e);
        }
    }

    /// Set the tray status directly
    #[allow(dead_code)]
    pub fn set_status(&mut self, status: TrayStatus) {
        debug!("Setting menu bar status: {:?}", status);
        self.status = status;
        self.status_item.set_text(status.as_str());
        if let Err(e) = self
            .tray_icon
            .set_tooltip(Some(format!("OpenHush - {}", status.as_str())))
        {
            warn!("Failed to update menu bar tooltip: {}", e);
        }
    }
}
