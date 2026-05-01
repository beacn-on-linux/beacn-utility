use crate::{APP_NAME, ManagerMessages, ToMainMessages};
use anyhow::{Result, bail};
use directories::BaseDirs;
use flume::{Receiver, Sender};
use log::{debug, warn};
use std::{env, fs, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::select;

pub async fn handle_ipc(
    manager_rx: Receiver<ManagerMessages>,
    main_tx: Sender<ToMainMessages>,
) -> Result<()> {
    debug!("Spawning IPC Socket");

    let socket_path = get_socket_file_path();
    if let Some(parent) = socket_path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        warn!("Failed to create socket directory {parent:?}: {e}");
        bail!("Failed to Open IPC Socket");
    }

    if socket_path.exists() {
        let _ = fs::remove_file(&socket_path);
    }

    let listener = match UnixListener::bind(&socket_path) {
        Ok(listener) => listener,
        Err(e) => {
            warn!("Failed to bind to socket: {e}");
            bail!("Failed to bind to socket: {e}");
        }
    };

    debug!("IPC listener started at {socket_path:?}");
    loop {
        select! {
            msg = manager_rx.recv_async() => {
                match msg {
                    Ok(ManagerMessages::Quit) => break,

                    Err(e) => {
                        warn!("Manager channel closed: {e}");
                        break;
                    }
                }
            }

            result = listener.accept() => {
                match result {
                    Ok((mut stream, _)) => {
                        let mut msg = String::new();

                        if let Err(e) = stream.read_to_string(&mut msg).await {
                            warn!("Failed to read message from stream: {e}");
                            continue;
                        }

                        match msg.as_str() {
                            "TRIGGER" => {
                                let _ = main_tx.send(ToMainMessages::SpawnWindow);
                            }

                            _ => {
                                debug!("Unknown message: {msg}");
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

    let _ = fs::remove_file(&socket_path);
    debug!("IPC Socket closed");
    Ok(())
}

pub async fn handle_active_instance() -> bool {
    let socket_path = get_socket_file_path();

    debug!("Looking for Socket at {socket_path:?}");
    if !socket_path.exists() {
        // The socket file doesn't exist, so the socket can't exist.
        debug!("Existing socket is not present");
        return false;
    }

    debug!("Attempting to connect to existing socket");
    match UnixStream::connect(&socket_path).await {
        Ok(mut stream) => {
            debug!("Connected to existing socket at {socket_path:?}, sending trigger");
            if let Err(e) = stream.write_all(b"TRIGGER").await {
                debug!("Failed to send trigger message: {e}");
                return false;
            }
            true
        }

        Err(e) => {
            debug!("Failed to connect to socket, removing stale file: {e}");
            let _ = fs::remove_file(socket_path);
            false
        }
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
