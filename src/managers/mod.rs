#[cfg(target_arch = "wasm32")]
use tokio_with_wasm as tokio;

use tokio::sync::oneshot;

#[cfg(not(target_arch = "wasm32"))]
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
