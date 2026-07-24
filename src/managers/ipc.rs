use crate::{APP_NAME, ManagerMessages, ToMainMessages};
use anyhow::{Result, bail};
use beacn_lib::flume::{Receiver, Sender};

use directories::BaseDirs;
use log::{debug, warn};

use std::{env, path::PathBuf};

use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};

pub async fn handle_ipc(
    manager_rx: Receiver<ManagerMessages>,
    main_tx: Sender<ToMainMessages>,
) -> Result<()> {
    debug!("Spawning IPC Socket");

    let socket_path = get_socket_file_path();
    if let Some(parent) = socket_path.parent()
        && let Err(e) = fs::create_dir_all(parent).await
    {
        warn!("Failed to create socket directory {parent:?}: {e}");
        bail!("Failed to open IPC Socket");
    }

    if socket_path.exists() {
        let _ = fs::remove_file(&socket_path).await;
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
        tokio::select! {
            msg = manager_rx.recv_async() => {
                match msg {
                    Ok(ManagerMessages::Quit) => {
                        break;
                    }

                    Err(e) => {
                        warn!("Message handler channel broken: {e}");
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
                                if let Err(e) = main_tx.send_async(ToMainMessages::SpawnWindow).await {
                                    warn!("Failed to send main message: {e}");
                                }
                            }

                            _ => {
                                debug!("Unknown Message, aborting: {msg}");
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

    let _ = fs::remove_file(&socket_path).await;
    debug!("IPC Socket closed");
    Ok(())
}

pub async fn handle_active_instance() -> bool {
    let socket_path = get_socket_file_path();
    debug!("Looking for Socket at {socket_path:?}");

    if !socket_path.exists() {
        debug!("Existing socket is not present");
        return false;
    }

    debug!("Attempting to connect to existing socket");

    match UnixStream::connect(&socket_path).await {
        Ok(mut stream) => {
            debug!("Connected, sending trigger");

            let _ = stream.write_all(b"TRIGGER").await;

            true
        }

        Err(e) => {
            debug!("Failed to connect to socket: {e}");
            debug!("Removing stale socket file");

            let _ = fs::remove_file(socket_path).await;

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
                let _ = std::fs::create_dir_all(&tmp_dir);
            }

            tmp_dir
        });

    base_path.join(APP_NAME).join(get_socket_file_name())
}

fn get_socket_file_name() -> String {
    format!("{APP_NAME}.socket")
}
