use crate::{APP_NAME, ManagerMessages, WindowMessage};
use anyhow::{Result, bail};
use beacn_lib::flume::{Receiver, Sender};
use directories::BaseDirs;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Name, NameType, ToFsName, ToNsName,
    tokio::prelude::{LocalSocketListener, LocalSocketStream},
    traits::tokio::{Listener, Stream},
};
use log::{debug, warn};
use std::io::ErrorKind;
use std::{env, fs, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn handle_ipc(
    manager_rx: Receiver<ManagerMessages>,
    main_tx: Sender<WindowMessage>,
) -> Result<()> {
    debug!("Spawning IPC Socket");

    let name = get_socket_name()?;
    let listener = match bind_listener(&name) {
        Ok(listener) => listener,
        Err(e) => {
            warn!("Failed to bind to socket: {e}");
            bail!("Failed to bind to socket: {e}");
        }
    };

    debug!("IPC listener started at {name:?}");
    loop {
        tokio::select! {
            msg = manager_rx.recv_async() => {
                match msg {
                    Ok(ManagerMessages::Quit) => break,
                    Err(_) => {
                        warn!("Message Handler channel broken, bailing");
                        break;
                    }
                }
            }

            accepted = listener.accept() => {
                match accepted {
                    Ok(mut stream) => {
                        let mut msg = String::new();
                        if let Err(e) = stream.read_to_string(&mut msg).await {
                            warn!("Failed to read message from stream: {e}");
                            break;
                        }
                        match msg.as_str() {
                            "TRIGGER" => {
                                let _ = main_tx.send(WindowMessage::OpenWindow);
                            }
                            _ => {
                                debug!("Unknown Message, aborting: {msg}");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Unexpected socket error: {e}");
                        break;
                    }
                }
            }
        }
    }

    debug!("IPC Socket closed");
    Ok(())
}

/// Binds the listener, transparently recovering from a stale socket left behind
/// by a previous, uncleanly-terminated instance.
fn bind_listener(name: &Name<'static>) -> std::io::Result<LocalSocketListener> {
    match ListenerOptions::new().name(name.clone()).create_tokio() {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            debug!("Socket appears to be in use; treating as stale and retrying bind");
            remove_stale_file_socket(name);
            ListenerOptions::new().name(name.clone()).create_tokio()
        }
        Err(e) => Err(e),
    }
}

/// Removes the on-disk socket file for the `GenericFilePath` fallback name type.
/// A no-op for namespaced names, which have nothing on the filesystem to remove.
fn remove_stale_file_socket(_name: &Name<'static>) {
    let path = get_socket_file_path();
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
}

/// Checks whether another instance is already running by attempting to connect
/// to its socket. If so, forwards a trigger message to it and returns `true`.
pub async fn handle_active_instance() -> bool {
    let name = match get_socket_name() {
        Ok(name) => name,
        Err(e) => {
            debug!("Failed to build socket name: {e}");
            return false;
        }
    };

    debug!("Attempting to Connect to Existing Socket at {name:?}");
    match LocalSocketStream::connect(name.clone()).await {
        Ok(mut stream) => {
            debug!("Connected to Existing Socket, Sending Trigger");
            let _ = stream.write_all(b"TRIGGER").await;
            let _ = stream.shutdown().await;
            true
        }
        Err(e) => {
            debug!("Failed to Connect to Socket: {e}");
            debug!("Removing Stale Socket File (if any)");
            remove_stale_file_socket(&name);
            false
        }
    }
}

fn get_socket_name() -> Result<Name<'static>> {
    let socket_file_name = get_socket_file_name();

    if GenericNamespaced::is_supported() {
        Ok(socket_file_name.to_ns_name::<GenericNamespaced>()?)
    } else {
        let path = get_socket_file_path();
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            warn!("Failed to create socket directory {parent:?}: {e}");
            bail!("Failed to create socket directory");
        }
        Ok(path
            .to_string_lossy()
            .into_owned()
            .to_fs_name::<GenericFilePath>()?)
    }
}

fn get_socket_file_path() -> PathBuf {
    let base_path = BaseDirs::new()
        .and_then(|base| base.runtime_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            let tmp_dir = env::temp_dir();
            if !tmp_dir.exists() {
                let _ = fs::create_dir_all(&tmp_dir);
            }
            tmp_dir
        });

    base_path.join(APP_NAME).join(get_socket_file_name())
}

fn get_socket_file_name() -> String {
    format!("{APP_NAME}.socket")
}
