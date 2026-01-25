//! Inter-process communication for CLI ↔ Service
//!
//! Provides message passing between CLI commands (`whis stop`, `whis status`, `whis toggle`)
//! and the background service started by `whis start`.
//!
//! # Protocol
//!
//! - Unix: Domain socket at `$XDG_RUNTIME_DIR/whis.sock` (fallback: `/tmp/whis.sock`)
//! - Windows: Named pipe `whis`
//!
//! # Messages
//!
//! - `Stop` → Terminate the service
//! - `Status` → Query recording state (Idle/Recording/Transcribing)
//! - `Toggle` → Start/stop recording
//!
//! # Components
//!
//! - `IpcServer` - Event-driven async listener for the service
//! - `IpcClient` - Blocking client for CLI commands
//! - `IpcConnection` - Individual client connection handler

use anyhow::{Context, Result};
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName, prelude::*};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use tokio::sync::mpsc;

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcMessage {
    Stop,
    Status,
    Toggle,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcResponse {
    Success,
    Recording,
    Idle,
    Transcribing,
    Error(String),
}

/// Get the socket name for IPC communication
#[cfg(unix)]
fn socket_name() -> String {
    std::env::var("XDG_RUNTIME_DIR")
        .map(|dir| format!("{dir}/whis.sock"))
        .unwrap_or_else(|_| "/tmp/whis.sock".to_string())
}

#[cfg(windows)]
fn socket_name() -> String {
    "whis".to_string()
}

/// IPC Server for the background service
///
/// Uses a background thread to accept connections and sends them through a channel,
/// enabling event-driven async operation without polling.
pub struct IpcServer {
    conn_rx: mpsc::UnboundedReceiver<IpcConnection>,
    #[cfg(unix)]
    socket_path: PathBuf,
}

impl IpcServer {
    pub fn new() -> Result<Self> {
        let name_str = socket_name();

        // On Unix, save socket path for cleanup and remove old socket if it exists
        #[cfg(unix)]
        let socket_path = PathBuf::from(&name_str);
        #[cfg(unix)]
        if socket_path.exists() {
            std::fs::remove_file(&socket_path).context("Failed to remove old socket file")?;
        }

        let name = name_str
            .to_fs_name::<GenericFilePath>()
            .context("Failed to create socket name")?;

        let listener = ListenerOptions::new()
            .name(name)
            .create_sync()
            .context("Failed to create IPC listener")?;

        // Create channel for connections
        let (conn_tx, conn_rx) = mpsc::unbounded_channel();

        // Spawn background thread to accept connections (blocking)
        std::thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok(stream) => {
                        if conn_tx.send(IpcConnection { stream }).is_err() {
                            break; // Receiver dropped, exit thread
                        }
                    }
                    Err(e) => {
                        // Log error but continue accepting - some errors may be transient
                        // (e.g., too many open files, interrupted syscall)
                        eprintln!("IPC accept error: {e}");
                    }
                }
            }
        });

        Ok(Self {
            conn_rx,
            #[cfg(unix)]
            socket_path,
        })
    }

    /// Receive the next connection asynchronously
    pub async fn accept(&mut self) -> Option<IpcConnection> {
        self.conn_rx.recv().await
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // On Unix, clean up the socket file
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(self.socket_path.as_path());
        }
        // On Windows, named pipes are cleaned up automatically by the OS
    }
}

/// IPC Connection for handling individual client connections
pub struct IpcConnection {
    stream: LocalSocketStream,
}

impl IpcConnection {
    /// Receive a message from the client
    pub fn receive(&mut self) -> Result<IpcMessage> {
        let mut reader = BufReader::new(&mut self.stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .context("Failed to read from socket")?;

        serde_json::from_str(line.trim()).context("Failed to deserialize message")
    }

    /// Send a response to the client
    pub fn send(&mut self, response: IpcResponse) -> Result<()> {
        let json = serde_json::to_string(&response)?;
        writeln!(self.stream, "{json}").context("Failed to write to socket")?;
        self.stream.flush().context("Failed to flush socket")?;
        Ok(())
    }
}

/// IPC Client for sending commands to the background service
pub struct IpcClient {
    stream: LocalSocketStream,
}

impl IpcClient {
    pub fn connect() -> Result<Self> {
        let name_str = socket_name();

        // On Unix, check if socket file exists first for better error messages
        #[cfg(unix)]
        {
            let path = PathBuf::from(&name_str);
            if !path.exists() {
                anyhow::bail!(
                    "whis service is not running.\n\
                    Start it with: whis start"
                );
            }
        }

        let name = name_str
            .to_fs_name::<GenericFilePath>()
            .context("Failed to create socket name")?;

        let stream = LocalSocketStream::connect(name).with_context(|| {
            #[cfg(unix)]
            {
                "Failed to connect to whis service.\n\
                The service may have crashed. Try removing stale files:\n\
                  rm -f $XDG_RUNTIME_DIR/whis.*\n\
                Then start the service again with: whis start"
            }
            #[cfg(windows)]
            {
                "Failed to connect to whis service.\n\
                The service may not be running. Start it with: whis start"
            }
        })?;

        Ok(Self { stream })
    }

    pub fn send_message(&mut self, message: IpcMessage) -> Result<IpcResponse> {
        // Send message
        let json = serde_json::to_string(&message)?;
        writeln!(self.stream, "{json}").context("Failed to send message")?;
        self.stream.flush().context("Failed to flush stream")?;

        // Receive response
        let mut reader = BufReader::new(&mut self.stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .context("Failed to read response")?;

        serde_json::from_str(line.trim()).context("Failed to deserialize response")
    }
}

/// Check if the service is already running
pub fn is_service_running() -> bool {
    let name_str = socket_name();

    // On Unix, check if socket file exists first
    #[cfg(unix)]
    let socket_path = PathBuf::from(&name_str);

    #[cfg(unix)]
    if !socket_path.exists() {
        return false;
    }

    // Try to connect to check if service is actually running
    let name = match name_str.to_fs_name::<GenericFilePath>() {
        Ok(n) => n,
        Err(_) => return false,
    };

    match LocalSocketStream::connect(name) {
        Ok(_) => {
            // Successfully connected, service is running
            true
        }
        Err(_) => {
            // Can't connect - service is not running
            // On Unix, clean up stale socket file
            #[cfg(unix)]
            {
                let _ = std::fs::remove_file(&socket_path);
            }
            false
        }
    }
}
