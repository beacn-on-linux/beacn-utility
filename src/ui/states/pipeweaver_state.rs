//! Shared state between the Pipeweaver async handler and the egui UI.

use pipeweaver_ipc::commands::{APICommand, DaemonCommand, DaemonRequest, DaemonStatus};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, error::TrySendError};

#[derive(Debug, Clone, Default)]
pub struct PipeweaverSnapshot {
    pub status: Option<DaemonStatus>,
    pub connected: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SharedPipeweaverState {
    inner: Arc<RwLock<PipeweaverSnapshot>>,
    command_tx: mpsc::Sender<DaemonRequest>,
}

impl SharedPipeweaverState {
    const COMMAND_QUEUE_CAPACITY: usize = 256;

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

    pub fn snapshot(&self) -> PipeweaverSnapshot {
        self.inner.read().unwrap().clone()
    }

    pub fn send_command(&self, cmd: APICommand) {
        self.try_send(DaemonRequest::Pipewire(cmd));
    }

    pub fn send_daemon_command(&self, cmd: DaemonCommand) {
        self.try_send(DaemonRequest::Daemon(cmd));
    }

    fn try_send(&self, request: DaemonRequest) {
        match self.command_tx.try_send(request) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => log::warn!("Dropping Pipeweaver UI command because the queue is full"),
            Err(TrySendError::Closed(_)) => log::debug!("Ignoring Pipeweaver UI command because the queue is closed"),
        }
    }

    pub fn update_status(&self, status: DaemonStatus) {
        let mut state = self.inner.write().unwrap();
        state.status = Some(status);
        state.connected = true;
        state.error = None;
    }

    pub fn set_disconnected(&self, error: Option<String>) {
        let mut state = self.inner.write().unwrap();
        state.connected = false;
        state.error = error;
    }

    pub fn set_connected(&self) {
        let mut state = self.inner.write().unwrap();
        state.connected = true;
        state.error = None;
    }
}
