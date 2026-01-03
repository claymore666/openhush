//! Windows named pipe IPC implementation.

use super::{IpcCommand, IpcError, IpcEvent, IpcResponse};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, info, warn};

// Windows API types
type HANDLE = isize;
const INVALID_HANDLE_VALUE: HANDLE = -1;

// File access flags
const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;
const OPEN_EXISTING: u32 = 3;

// Pipe constants
const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
const PIPE_TYPE_BYTE: u32 = 0x00000000;
const PIPE_READMODE_BYTE: u32 = 0x00000000;
const PIPE_WAIT: u32 = 0x00000000;
const PIPE_UNLIMITED_INSTANCES: u32 = 255;
const ERROR_PIPE_CONNECTED: u32 = 535;

const PIPE_NAME: &str = r"\\.\pipe\openhush";
const BUFFER_SIZE: u32 = 4096;

// Windows API bindings
#[link(name = "kernel32")]
extern "system" {
    fn CreateNamedPipeW(
        name: *const u16,
        open_mode: u32,
        pipe_mode: u32,
        max_instances: u32,
        out_buffer_size: u32,
        in_buffer_size: u32,
        default_timeout: u32,
        security_attributes: *mut std::ffi::c_void,
    ) -> HANDLE;

    fn ConnectNamedPipe(pipe: HANDLE, overlapped: *mut std::ffi::c_void) -> i32;

    fn DisconnectNamedPipe(pipe: HANDLE) -> i32;

    fn CreateFileW(
        file_name: *const u16,
        desired_access: u32,
        share_mode: u32,
        security_attributes: *mut std::ffi::c_void,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: HANDLE,
    ) -> HANDLE;

    fn ReadFile(
        file: HANDLE,
        buffer: *mut u8,
        bytes_to_read: u32,
        bytes_read: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;

    fn WriteFile(
        file: HANDLE,
        buffer: *const u8,
        bytes_to_write: u32,
        bytes_written: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;

    fn CloseHandle(handle: HANDLE) -> i32;

    fn GetLastError() -> u32;
}

/// Transport layer for IPC server.
pub struct IpcServerTransport;

/// Transport layer for IPC client.
pub struct IpcClientTransport;

/// Internal IPC client implementation for Windows.
pub struct IpcClientInner {
    pipe: HANDLE,
    event_rx: Option<Receiver<IpcEvent>>,
}

impl IpcClientInner {
    /// Connect to daemon.
    pub fn connect() -> Result<Self, IpcError> {
        let pipe_name: Vec<u16> = OsStr::new(PIPE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let pipe = unsafe {
            CreateFileW(
                pipe_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                0,
            )
        };

        if pipe == INVALID_HANDLE_VALUE {
            return Err(IpcError::NotRunning);
        }

        Ok(Self {
            pipe,
            event_rx: None,
        })
    }

    /// Send command and get response.
    pub fn send(&mut self, cmd: IpcCommand) -> Result<IpcResponse, IpcError> {
        let json = serde_json::to_string(&cmd).map_err(|e| IpcError::SendFailed(e.to_string()))?;
        let msg = format!("{}\n", json);
        let bytes = msg.as_bytes();

        let mut written: u32 = 0;
        let write_ok = unsafe {
            WriteFile(
                self.pipe,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            )
        };

        if write_ok == 0 {
            return Err(IpcError::SendFailed("Write failed".into()));
        }

        // Read response
        let mut buffer = [0u8; BUFFER_SIZE as usize];
        let mut bytes_read: u32 = 0;

        let read_ok = unsafe {
            ReadFile(
                self.pipe,
                buffer.as_mut_ptr(),
                BUFFER_SIZE,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };

        if read_ok == 0 || bytes_read == 0 {
            return Err(IpcError::RecvFailed("Read failed".into()));
        }

        let data = String::from_utf8_lossy(&buffer[..bytes_read as usize]);
        let line = data
            .lines()
            .next()
            .ok_or_else(|| IpcError::RecvFailed("Empty response".into()))?;

        serde_json::from_str(line).map_err(|e| IpcError::RecvFailed(e.to_string()))
    }

    /// Subscribe to events (Windows: not yet implemented, returns empty receiver).
    pub fn subscribe(&mut self) -> Result<Receiver<IpcEvent>, IpcError> {
        // TODO: Implement persistent connection with event streaming for Windows
        let (_tx, rx) = mpsc::channel();
        self.event_rx = Some(rx);
        warn!("IPC event subscription not yet implemented on Windows");

        let (tx2, rx2) = mpsc::channel();
        Ok(rx2)
    }

    /// Try to receive an event (non-blocking).
    pub fn try_recv_event(&mut self) -> Option<IpcEvent> {
        self.event_rx.as_ref()?.try_recv().ok()
    }
}

impl Drop for IpcClientInner {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.pipe) };
    }
}
