use crate::window_handle::{UserEvent, send_user_event};
use crate::{APP_NAME, ManagerMessages, ToMainMessages};
use anyhow::{Result, bail};
use beacn_lib::crossbeam::channel::{Receiver, Sender};
use beacn_lib::crossbeam::select;
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
    let mut context = None;

    debug!("IPC listener started at {socket_path:?}");
    loop {
        select! {
            recv(manager_rx) -> msg => {
                match msg {
                    Ok(msg) => {
                        match msg {
                            ManagerMessages::SetContext(ctx) => context = ctx,
                            ManagerMessages::Quit => break,
                        }
                    }
                    Err(e) => {
                        warn!("Message Handler channel Broken, bailing: {e}");
                        break;
                    }
                }
            }

            default(poll_duration) => {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut msg = String::new();
                        if let Err(e) = stream.read_to_string(&mut msg) {
                            warn!("Failed to read from message from stream: {e}");
                            break;
                        } else {
                            match msg.as_str() {
                                "TRIGGER" => {
                                    if let Some(context) = &context {
                                        send_user_event(context, UserEvent::FocusWindow);
                                    } else {
                                        let _ = main_tx.send(ToMainMessages::SpawnWindow);
                                    }
                                },
                                _ => {
                                    debug!("Unknown Message, aborting: {msg}");
                                    break;
                                },
                            }
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        // No client, do nothing
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
    env::temp_dir().join(APP_NAME).join(get_socket_file_name())
}

fn get_socket_file_name() -> String {
    format!("{APP_NAME}.socket")
}
