//! D-Bus integration for OpenHush daemon.
//!
//! Provides the `org.openhush.Daemon1` interface for remote control of the daemon,
//! enabling commands like `openhush recording start|stop|status`.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐         D-Bus          ┌──────────────┐
//! │  OpenHush       │<───────────────────────│ openhush     │
//! │  Daemon         │  org.openhush.Daemon1  │ recording    │
//! │  (DbusService)  │───────────────────────>│ (DbusClient) │
//! └─────────────────┘                        └──────────────┘
//! ```
//!
//! # Usage
//!
//! ## Server (daemon side)
//!
//! ```ignore
//! let status = Arc::new(RwLock::new(DaemonStatus::default()));
//! let (dbus_service, mut dbus_rx) = DbusService::start(status.clone()).await?;
//!
//! // In event loop:
//! if let Some(cmd) = dbus_rx.try_recv().ok() {
//!     match cmd {
//!         DaemonCommand::StartRecording => { /* ... */ }
//!         DaemonCommand::StopRecording => { /* ... */ }
//!         DaemonCommand::ToggleRecording => { /* ... */ }
//!     }
//! }
//! ```
//!
//! ## Client (CLI side)
//!
//! ```ignore
//! let client = DbusClient::connect().await?;
//! let status = client.get_status().await?;
//! println!("Status: {}", status);
//! ```

mod interface;
mod service;

pub use interface::{DaemonCommand, DaemonStatus};
pub use service::{DbusClient, DbusService};
