use tokio::sync::oneshot;

pub mod ipc;

#[cfg(target_os = "linux")]
pub mod login;

#[cfg(target_os = "linux")]
pub mod tray;

#[derive(Debug)]
#[allow(unused)]
pub enum LoginEventTriggers {
    Sleep(oneshot::Sender<()>),
    Wake(oneshot::Sender<()>),
    Lock,
    Unlock,
}
