//! Windows named pipe IPC implementation.

use super::{IpcCommand, IpcError, IpcResponse};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use tracing::{debug, info, warn};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, OPEN_EXISTING};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_ACCESS_DUPLEX,
    PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

const PIPE_NAME: &str = r"\\.\pipe\openhush";
const BUFFER_SIZE: u32 = 4096;

/// IPC server using Windows named pipe.
pub struct IpcServer {
    pipe: HANDLE,
}

impl IpcServer {
    /// Create named pipe server.
    pub fn new() -> Result<Self, IpcError> {
        let pipe_name: Vec<u16> = OsStr::new(PIPE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let pipe = unsafe {
            CreateNamedPipeW(
                pipe_name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                BUFFER_SIZE,
                BUFFER_SIZE,
                0,
                std::ptr::null_mut(),
            )
        };

        if pipe == INVALID_HANDLE_VALUE {
            return Err(IpcError::BindFailed("Failed to create named pipe".into()));
        }

        info!("IPC listening on {}", PIPE_NAME);
        Ok(Self { pipe })
    }

    /// Try to receive a command (blocking for short duration).
    pub fn try_recv(&self) -> Option<(IpcCommand, Box<dyn FnOnce(IpcResponse) + Send>)> {
        // Try to connect a client (this blocks briefly)
        let connected = unsafe { ConnectNamedPipe(self.pipe, std::ptr::null_mut()) };

        if connected == 0 {
            // Check if client already connected
            let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            if error != 535 {
                // ERROR_PIPE_CONNECTED
                return None;
            }
        }

        // Read command
        let mut buffer = [0u8; BUFFER_SIZE as usize];
        let mut bytes_read: u32 = 0;

        let read_ok = unsafe {
            ReadFile(
                self.pipe,
                buffer.as_mut_ptr() as *mut _,
                BUFFER_SIZE,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };

        if read_ok == 0 || bytes_read == 0 {
            unsafe { DisconnectNamedPipe(self.pipe) };
            return None;
        }

        let data = String::from_utf8_lossy(&buffer[..bytes_read as usize]);
        let line = data.lines().next()?;

        match serde_json::from_str::<IpcCommand>(line) {
            Ok(cmd) => {
                debug!("IPC command: {:?}", cmd);
                let pipe = self.pipe;
                let responder = Box::new(move |response: IpcResponse| {
                    if let Ok(json) = serde_json::to_string(&response) {
                        let msg = format!("{}\n", json);
                        let bytes = msg.as_bytes();
                        let mut written: u32 = 0;
                        unsafe {
                            WriteFile(
                                pipe,
                                bytes.as_ptr() as *const _,
                                bytes.len() as u32,
                                &mut written,
                                std::ptr::null_mut(),
                            );
                            DisconnectNamedPipe(pipe);
                        }
                    }
                });
                Some((cmd, responder))
            }
            Err(e) => {
                warn!("IPC parse error: {}", e);
                unsafe { DisconnectNamedPipe(self.pipe) };
                None
            }
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.pipe) };
    }
}

/// IPC client for Windows.
pub struct IpcClient {
    pipe: HANDLE,
}

impl IpcClient {
    /// Connect to daemon's named pipe.
    pub fn connect() -> Result<Self, IpcError> {
        let pipe_name: Vec<u16> = OsStr::new(PIPE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let pipe = unsafe {
            CreateFileW(
                pipe_name.as_ptr(),
                0x80000000 | 0x40000000, // GENERIC_READ | GENERIC_WRITE
                0,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if pipe == INVALID_HANDLE_VALUE {
            return Err(IpcError::NotRunning);
        }

        Ok(Self { pipe })
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
                bytes.as_ptr() as *const _,
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
                buffer.as_mut_ptr() as *mut _,
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
}

impl Drop for IpcClient {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.pipe) };
    }
}
