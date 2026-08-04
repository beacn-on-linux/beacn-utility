use tokio::sync::oneshot;

pub mod ipc;

#[cfg(unix)]
pub mod login;

#[cfg(unix)]
pub mod tray;

#[derive(Debug)]
#[allow(unused)]
pub enum LoginEventTriggers {
    Sleep(oneshot::Sender<()>),
    Wake(oneshot::Sender<()>),
    Lock,
    Unlock,
}
