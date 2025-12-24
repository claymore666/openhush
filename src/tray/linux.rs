//! Linux system tray implementation using ksni (D-Bus StatusNotifierItem).

use super::{TrayError, TrayEvent, TrayStatus};
use ksni::menu::*;
use ksni::{Handle, ToolTip, Tray, TrayMethods};
use std::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, error, info};

/// OpenHush tray implementation
struct OpenHushTray {
    status: TrayStatus,
    event_tx: Sender<TrayEvent>,
}

impl Tray for OpenHushTray {
    fn id(&self) -> String {
        "openhush".into()
    }

    fn icon_name(&self) -> String {
        self.status.icon_name().into()
    }

    fn title(&self) -> String {
        "OpenHush".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "OpenHush - Voice to Text".into(),
            description: self.status.as_str().into(),
            icon_name: self.status.icon_name().into(),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let status_text = self.status.as_str();

        vec![
            // Status (disabled, just for display)
            StandardItem {
                label: status_text.into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Preferences
            StandardItem {
                label: "Preferences...".into(),
                activate: Box::new(|tray: &mut Self| {
                    debug!("Tray: Preferences clicked");
                    let _ = tray.event_tx.send(TrayEvent::ShowPreferences);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Quit
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| {
                    debug!("Tray: Quit clicked");
                    let _ = tray.event_tx.send(TrayEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        debug!("Tray icon activated (clicked)");
        let _ = self.event_tx.send(TrayEvent::StatusClicked);
    }
}

/// Manages the system tray icon and menu
pub struct TrayManager {
    handle: Handle<OpenHushTray>,
    event_rx: Receiver<TrayEvent>,
}

impl TrayManager {
    /// Create a new tray manager
    ///
    /// Returns an error if the system tray is not available.
    /// This is an async function because ksni uses D-Bus which requires async.
    pub async fn new() -> Result<Self, TrayError> {
        // Check for D-Bus session bus
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
            return Err(TrayError::NotSupported);
        }

        let (event_tx, event_rx) = mpsc::channel();

        let tray = OpenHushTray {
            status: TrayStatus::Idle,
            event_tx,
        };

        // Create and spawn the tray service
        let handle = tray
            .spawn()
            .await
            .map_err(|e: ksni::Error| TrayError::DBus(e.to_string()))?;

        info!("System tray initialized (D-Bus StatusNotifierItem)");
        Ok(Self { handle, event_rx })
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

    /// Update the status displayed in the tray
    pub fn update_status(&self, status: &str) {
        debug!("Updating tray status: {}", status);
        // Map string to TrayStatus
        let new_status = match status {
            s if s.contains("Recording") => TrayStatus::Recording,
            s if s.contains("Processing") => TrayStatus::Processing,
            s if s.contains("Error") => TrayStatus::Error,
            _ => TrayStatus::Idle,
        };

        self.handle.update(|tray| {
            tray.status = new_status;
        });
    }

    /// Set the tray status directly
    pub fn set_status(&self, status: TrayStatus) {
        debug!("Setting tray status: {:?}", status);
        self.handle.update(|tray| {
            tray.status = status;
        });
    }
}
