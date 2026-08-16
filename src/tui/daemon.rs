//! Daemon communication for TUI.
//!
//! Provides a high-level interface for the TUI to communicate with the daemon
//! via IPC, handling connection management and event processing.

use crate::ipc::{DaemonStatus, IpcClient, IpcCommand, IpcError, IpcEvent, IpcResponse};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// Connection state to the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected, never tried.
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Connected and subscribed to events.
    Connected,
    /// Connection lost, will retry.
    Reconnecting,
    /// Daemon is not running.
    DaemonNotRunning,
}

/// High-level daemon client for the TUI.
pub struct DaemonClient {
    client: Option<IpcClient>,
    event_rx: Option<Receiver<IpcEvent>>,
    state: ConnectionState,
    last_status: Option<DaemonStatus>,
    last_connect_attempt: Option<Instant>,
    reconnect_delay: Duration,
}

#[allow(dead_code)]
impl DaemonClient {
    /// Create a new daemon client (not yet connected).
    pub fn new() -> Self {
        Self {
            client: None,
            event_rx: None,
            state: ConnectionState::Disconnected,
            last_status: None,
            last_connect_attempt: None,
            reconnect_delay: Duration::from_secs(1),
        }
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Check if connected to daemon.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Get the last known status.
    pub fn last_status(&self) -> Option<&DaemonStatus> {
        self.last_status.as_ref()
    }

    /// Try to connect to the daemon.
    pub fn connect(&mut self) -> Result<(), IpcError> {
        // Don't retry too quickly
        if let Some(last) = self.last_connect_attempt {
            if last.elapsed() < self.reconnect_delay {
                return Err(IpcError::ConnectFailed("Rate limited".into()));
            }
        }

        self.last_connect_attempt = Some(Instant::now());
        self.state = ConnectionState::Connecting;

        match IpcClient::connect() {
            Ok(client) => {
                self.client = Some(client);

                // Subscribe to events
                if let Some(ref mut c) = self.client {
                    match c.subscribe() {
                        Ok(rx) => {
                            self.event_rx = Some(rx);
                            self.state = ConnectionState::Connected;
                        }
                        Err(_) => {
                            // Still connected, just no events
                            self.state = ConnectionState::Connected;
                        }
                    }
                }

                // Get initial status
                self.refresh_status();

                Ok(())
            }
            Err(IpcError::NotRunning) => {
                self.state = ConnectionState::DaemonNotRunning;
                Err(IpcError::NotRunning)
            }
            Err(e) => {
                self.state = ConnectionState::Disconnected;
                Err(e)
            }
        }
    }

    /// Disconnect from the daemon.
    pub fn disconnect(&mut self) {
        self.client = None;
        self.event_rx = None;
        self.state = ConnectionState::Disconnected;
    }

    /// Send a command and get response.
    pub fn send(&mut self, cmd: IpcCommand) -> Result<IpcResponse, IpcError> {
        let client = self.client.as_mut().ok_or(IpcError::NotRunning)?;

        match client.send(cmd) {
            Ok(response) => Ok(response),
            Err(e) => {
                // Connection might be broken
                self.state = ConnectionState::Reconnecting;
                Err(e)
            }
        }
    }

    /// Refresh daemon status.
    pub fn refresh_status(&mut self) -> Option<&DaemonStatus> {
        if let Ok(response) = self.send(IpcCommand::Status) {
            if response.ok {
                if let Some(crate::ipc::IpcResponseData::Status(status)) = response.data {
                    self.last_status = Some(status);
                }
            }
        }
        self.last_status.as_ref()
    }

    /// Start recording.
    pub fn start_recording(&mut self) -> Result<(), String> {
        match self.send(IpcCommand::StartRecording) {
            Ok(r) if r.ok => Ok(()),
            Ok(r) => Err(r.error.unwrap_or_else(|| "Unknown error".into())),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Stop recording.
    pub fn stop_recording(&mut self) -> Result<(), String> {
        match self.send(IpcCommand::StopRecording) {
            Ok(r) if r.ok => Ok(()),
            Ok(r) => Err(r.error.unwrap_or_else(|| "Unknown error".into())),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Toggle recording state.
    pub fn toggle_recording(&mut self) -> Result<(), String> {
        match self.send(IpcCommand::ToggleRecording) {
            Ok(r) if r.ok => Ok(()),
            Ok(r) => Err(r.error.unwrap_or_else(|| "Unknown error".into())),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Load the Whisper model.
    pub fn load_model(&mut self) -> Result<(), String> {
        match self.send(IpcCommand::LoadModel) {
            Ok(r) if r.ok => Ok(()),
            Ok(r) => Err(r.error.unwrap_or_else(|| "Unknown error".into())),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Unload the Whisper model.
    pub fn unload_model(&mut self) -> Result<(), String> {
        match self.send(IpcCommand::UnloadModel) {
            Ok(r) if r.ok => Ok(()),
            Ok(r) => Err(r.error.unwrap_or_else(|| "Unknown error".into())),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Poll for events (non-blocking).
    pub fn poll_events(&mut self) -> Vec<IpcEvent> {
        let mut events = Vec::new();

        if let Some(ref rx) = self.event_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }

        events
    }

    /// Handle reconnection if needed.
    pub fn handle_reconnect(&mut self) {
        // Try to connect/reconnect in all non-connected states
        if self.state != ConnectionState::Connected && self.state != ConnectionState::Connecting {
            // Try to reconnect (connect() handles rate limiting)
            let _ = self.connect();
        }
    }
}

impl Default for DaemonClient {
    fn default() -> Self {
        Self::new()
    }
}
