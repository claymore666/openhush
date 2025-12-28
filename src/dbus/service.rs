//! D-Bus service setup and client for OpenHush daemon.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};
use zbus::{connection::Builder, Connection, Result};

use super::interface::{DaemonCommand, DaemonInterface, DaemonStatus};

/// Well-known bus name for the OpenHush daemon.
pub const BUS_NAME: &str = "org.openhush.Daemon1";

/// Object path for the daemon interface.
pub const OBJECT_PATH: &str = "/org/openhush/Daemon1";

/// D-Bus service handle.
///
/// Keeps the D-Bus connection alive and provides methods for emitting signals.
pub struct DbusService {
    connection: Connection,
}

impl DbusService {
    /// Start the D-Bus service.
    ///
    /// Registers the daemon interface on the session bus and requests the well-known name.
    /// Returns a channel receiver for commands from D-Bus clients.
    pub async fn start(
        status: Arc<RwLock<DaemonStatus>>,
    ) -> Result<(Self, mpsc::Receiver<DaemonCommand>)> {
        let (command_tx, command_rx) = mpsc::channel(32);

        let interface = DaemonInterface::new(command_tx, status);

        let connection = Builder::session()?
            .name(BUS_NAME)?
            .serve_at(OBJECT_PATH, interface)?
            .build()
            .await?;

        info!("D-Bus service started: {} at {}", BUS_NAME, OBJECT_PATH);

        Ok((Self { connection }, command_rx))
    }

    /// Emit a signal that recording state has changed.
    pub async fn emit_recording_changed(&self) -> Result<()> {
        debug!("Emitting IsRecording property change");
        let iface_ref = self
            .connection
            .object_server()
            .interface::<_, DaemonInterface>(OBJECT_PATH)
            .await?;

        // Emit property change signal
        let iface = iface_ref.get().await;
        iface
            .is_recording_changed(iface_ref.signal_emitter())
            .await?;
        Ok(())
    }

    /// Get a reference to the connection for advanced usage.
    #[allow(dead_code)]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// D-Bus client for controlling the daemon.
///
/// Used by the CLI to send commands to a running daemon.
pub struct DbusClient {
    connection: Connection,
}

#[allow(dead_code)]
impl DbusClient {
    /// Connect to the daemon's D-Bus interface.
    pub async fn connect() -> Result<Self> {
        let connection = Connection::session().await?;
        Ok(Self { connection })
    }

    /// Check if the daemon is running (owns the bus name).
    pub async fn is_daemon_running(&self) -> bool {
        let proxy = zbus::fdo::DBusProxy::new(&self.connection).await.ok();

        if let Some(proxy) = proxy {
            proxy
                .name_has_owner(BUS_NAME.try_into().unwrap())
                .await
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Start recording.
    pub async fn start_recording(&self) -> Result<()> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.start_recording().await
    }

    /// Stop recording.
    pub async fn stop_recording(&self) -> Result<()> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.stop_recording().await
    }

    /// Toggle recording.
    pub async fn toggle_recording(&self) -> Result<()> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.toggle_recording().await
    }

    /// Load the Whisper model into GPU memory.
    pub async fn load_model(&self) -> Result<()> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.load_model().await
    }

    /// Unload the Whisper model to free GPU memory.
    pub async fn unload_model(&self) -> Result<()> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.unload_model().await
    }

    /// Get current status.
    pub async fn get_status(&self) -> Result<String> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.get_status().await
    }

    /// Get recording state.
    pub async fn is_recording(&self) -> Result<bool> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.is_recording().await
    }

    /// Get queue depth.
    pub async fn queue_depth(&self) -> Result<u32> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.queue_depth().await
    }

    /// Get model loaded state.
    pub async fn model_loaded(&self) -> Result<bool> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.model_loaded().await
    }

    /// Get daemon version.
    pub async fn version(&self) -> Result<String> {
        let proxy = DaemonProxy::new(&self.connection).await?;
        proxy.version().await
    }
}

/// Auto-generated proxy for calling D-Bus methods.
#[zbus::proxy(
    interface = "org.openhush.Daemon1",
    default_service = "org.openhush.Daemon1",
    default_path = "/org/openhush/Daemon1"
)]
trait Daemon {
    fn start_recording(&self) -> zbus::Result<()>;
    fn stop_recording(&self) -> zbus::Result<()>;
    fn toggle_recording(&self) -> zbus::Result<()>;
    fn load_model(&self) -> zbus::Result<()>;
    fn unload_model(&self) -> zbus::Result<()>;
    fn get_status(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn is_recording(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn queue_depth(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn model_loaded(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
}
