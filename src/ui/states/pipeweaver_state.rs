//! Shared state between the Pipeweaver async handler and the egui UI.
<<<<<<< feature/pipeweaver-preflight-setup
=======
//!
//! The handler writes status updates; the UI reads them and sends commands back.
>>>>>>> main

use pipeweaver_ipc::commands::{APICommand, DaemonCommand, DaemonRequest, DaemonStatus};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, error::TrySendError};

<<<<<<< feature/pipeweaver-preflight-setup
#[derive(Debug, Clone, Default)]
pub struct PipeweaverSnapshot {
    pub status: Option<DaemonStatus>,
    pub connected: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SharedPipeweaverState {
    inner: Arc<RwLock<PipeweaverSnapshot>>,
=======
/// Snapshot of Pipeweaver state for the UI to render.
#[derive(Debug, Clone, Default)]
pub struct PipeweaverSnapshot {
    /// Full daemon status (channels, volumes, routing, apps).
    pub status: Option<DaemonStatus>,
    /// Whether we're connected to the Pipeweaver daemon.
    pub connected: bool,
    /// Last error message, if any.
    pub error: Option<String>,
}

/// Thread-safe shared state handle.
/// The Pipeweaver handler holds a clone of this and calls `update_*` methods.
/// The UI holds a clone and calls `snapshot()` to read current state.
#[derive(Debug, Clone)]
pub struct SharedPipeweaverState {
    inner: Arc<RwLock<PipeweaverSnapshot>>,
    /// Bounded channel for the UI to send requests to the Pipeweaver handler.
    /// Uses `DaemonRequest` so both `APICommand` and `DaemonCommand` can be sent.
>>>>>>> main
    command_tx: mpsc::Sender<DaemonRequest>,
}

impl SharedPipeweaverState {
    const COMMAND_QUEUE_CAPACITY: usize = 256;

<<<<<<< feature/pipeweaver-preflight-setup
    pub fn new() -> (Self, mpsc::Receiver<DaemonRequest>) {
        let (tx, rx) = mpsc::channel(Self::COMMAND_QUEUE_CAPACITY);
        (
            Self {
                inner: Arc::new(RwLock::new(PipeweaverSnapshot::default())),
                command_tx: tx,
            },
            rx,
        )
    }

=======
    /// Create a new shared state and the command receiver for the handler.
    pub fn new() -> (Self, mpsc::Receiver<DaemonRequest>) {
        let (tx, rx) = mpsc::channel(Self::COMMAND_QUEUE_CAPACITY);
        let state = Self {
            inner: Arc::new(RwLock::new(PipeweaverSnapshot::default())),
            command_tx: tx,
        };
        (state, rx)
    }

    /// Get a snapshot of the current Pipeweaver state.
>>>>>>> main
    pub fn snapshot(&self) -> PipeweaverSnapshot {
        self.inner.read().unwrap().clone()
    }

<<<<<<< feature/pipeweaver-preflight-setup
=======
    /// Send a PipeWire API command (routing, volumes, etc.).
>>>>>>> main
    pub fn send_command(&self, cmd: APICommand) {
        self.try_send(DaemonRequest::Pipewire(cmd));
    }

<<<<<<< feature/pipeweaver-preflight-setup
=======
    /// Send a daemon-level command (autostart, reset, etc.).
>>>>>>> main
    pub fn send_daemon_command(&self, cmd: DaemonCommand) {
        self.try_send(DaemonRequest::Daemon(cmd));
    }

    fn try_send(&self, request: DaemonRequest) {
        match self.command_tx.try_send(request) {
            Ok(()) => {}
<<<<<<< feature/pipeweaver-preflight-setup
            Err(TrySendError::Full(_)) => log::warn!("Dropping Pipeweaver UI command because the queue is full"),
            Err(TrySendError::Closed(_)) => log::debug!("Ignoring Pipeweaver UI command because the queue is closed"),
        }
    }

=======
            Err(TrySendError::Full(_)) => {
                log::warn!("Dropping Pipeweaver UI command because the queue is full");
            }
            Err(TrySendError::Closed(_)) => {
                log::debug!("Ignoring Pipeweaver UI command because the queue is closed");
            }
        }
    }

    /// Update the full daemon status.
>>>>>>> main
    pub fn update_status(&self, status: DaemonStatus) {
        let mut state = self.inner.write().unwrap();
        state.status = Some(status);
        state.connected = true;
        state.error = None;
    }

<<<<<<< feature/pipeweaver-preflight-setup
=======
    /// Mark as disconnected.
>>>>>>> main
    pub fn set_disconnected(&self, error: Option<String>) {
        let mut state = self.inner.write().unwrap();
        state.connected = false;
        state.error = error;
    }

<<<<<<< feature/pipeweaver-preflight-setup
=======
    /// Mark as connected.
>>>>>>> main
    pub fn set_connected(&self) {
        let mut state = self.inner.write().unwrap();
        state.connected = true;
        state.error = None;
    }
}
