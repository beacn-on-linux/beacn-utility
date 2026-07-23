use crate::{APP_NAME, ManagerMessages, ToMainMessages};
use anyhow::{Result, bail};
use beacn_lib::flume::{Receiver, Selector, Sender};

use directories::BaseDirs;
use log::{debug, warn};
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;
use std::{
    env, fs,
    io::{Read, Write},
    path::PathBuf,
};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

pub fn handle_ipc(
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

    if let Err(e) = listener.set_nonblocking(true) {
        warn!("Failed to set socket non-blocking: {e}");
        bail!("Failed to set socket non-blocking: {e}");
    }

    let poll_duration = Duration::from_millis(50);

    debug!("IPC listener started at {socket_path:?}");
    loop {
        let should_quit = Selector::new()
            .recv(&manager_rx, |msg| match msg {
                Ok(ManagerMessages::Quit) => true,

                Err(e) => {
                    warn!("Message Handler channel Broken, bailing: {e}");
                    true
                }
            })
            .wait_timeout(poll_duration)
            .is_ok_and(|should_quit| should_quit);

        if should_quit {
            break;
        }

        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut msg = String::new();

                if let Err(e) = stream.read_to_string(&mut msg) {
                    warn!("Failed to read message from stream: {e}");
                    break;
                }

                match msg.as_str() {
                    "TRIGGER" => {
                        let _ = main_tx.send(ToMainMessages::SpawnWindow);
                    }

                    _ => {
                        debug!("Unknown Message, aborting: {msg}");
                        break;
                    }
                }
            }

            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // No client, continue polling
            }

            Err(e) => {
                warn!("Unexpected socket error: {e}");
                break;
            }
        }
    }

    let _ = fs::remove_file(&socket_path);
    debug!("IPC Socket closed");
    Ok(())
}

pub fn handle_active_instance() -> bool {
    let socket_path = get_socket_file_path();
    debug!("Looking for Socket at {socket_path:?}");

    if !socket_path.exists() {
        debug!("Existing socket is not present");
        // The socket file doesn't exist, so the socket can't exist.
        return false;
    }

    debug!("Attempting to Connect to Existing Socket");
    // The socket exists, let's see if we can connect to it
    match UnixStream::connect(&socket_path) {
        Ok(mut stream) => {
            debug!("Connected to Existing Socket at {socket_path:?}, Sending Trigger");
            let _ = stream.write_all(b"TRIGGER");
            return true;
        }
        Err(e) => {
            debug!("Failed to Connect to Socket: {e}");
            debug!("Removing Stale Socket File");
            let _ = fs::remove_file(socket_path);
        }
    }
    false
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
