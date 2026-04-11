//! Shared state between the Pipeweaver async handler and the egui UI.
//!
//! The handler writes status updates; the UI reads them and sends commands back.

use pipeweaver_ipc::commands::{APICommand, DaemonCommand, DaemonRequest, DaemonStatus};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc::{Sender, Receiver, channel};

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
    /// Channel for the UI to send requests to the Pipeweaver handler.
    /// Uses `DaemonRequest` so both `APICommand` and `DaemonCommand` can be sent.
    command_tx: Sender<DaemonRequest>,
}

impl SharedPipeweaverState {
    /// Create a new shared state and the command receiver for the handler.
    pub fn new() -> (Self, Receiver<DaemonRequest>) {
        let (tx, rx) = channel(32);
        let state = Self {
            inner: Arc::new(RwLock::new(PipeweaverSnapshot::default())),
            command_tx: tx,
        };
        (state, rx)
    }

    // --- UI-facing methods (sync, called from egui thread) ---

    /// Get a snapshot of the current Pipeweaver state.
    pub fn snapshot(&self) -> PipeweaverSnapshot {
        self.inner.read().clone()
    }

    /// Send a PipeWire API command (routing, volumes, etc.).
    pub fn send_command(&self, cmd: APICommand) {
        let _ = self.command_tx.try_send(DaemonRequest::Pipewire(cmd));
    }

    /// Send a daemon-level command (autostart, reset, etc.).
    pub fn send_daemon_command(&self, cmd: DaemonCommand) {
        let _ = self.command_tx.try_send(DaemonRequest::Daemon(cmd));
    }

    // --- Handler-facing methods (called from async Pipeweaver handler) ---

    /// Update the full daemon status.
    pub fn update_status(&self, status: DaemonStatus) {
        let mut state = self.inner.write();
        state.status = Some(status);
        state.connected = true;
        state.error = None;
    }

    /// Mark as disconnected.
    pub fn set_disconnected(&self, error: Option<String>) {
        let mut state = self.inner.write();
        state.connected = false;
        state.error = error;
    }

    /// Mark as connected.
    pub fn set_connected(&self) {
        let mut state = self.inner.write();
        state.connected = true;
        state.error = None;
    }
}
