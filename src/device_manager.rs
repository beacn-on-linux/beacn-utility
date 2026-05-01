/*
  This file primarily manages hot-plugging opening devices, and device messaging.

  When a device appears, we open it, create a message handler and throw it upstream. We then
  listen on all the message handlers, and when one pops up, handle the message.

  If a device disappears, we simply drop its channel, upstream should pick up on that and
  handle it appropriately.

  For the moment, for the Beacn Mic + Beacn Studio we're going to have a single message type,
  same applies for the Mix and Mix Create. The devices are too similar to have to worry about
  differences.
*/
use crate::integrations::pipeweaver::spawn_pipeweaver_handler;
use crate::managers::login::{LoginEventTriggers, spawn_login_handler};
use crate::{ManagerMessages, ToMainMessages};
use anyhow::anyhow;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::{BeacnAudioDevice, LinkedApp, open_audio_device};
use beacn_lib::controller::{BeacnControlDevice, ButtonLighting, open_control_device};
use beacn_lib::manager::{
    DeviceLocation, DeviceType, HotPlugMessage, HotPlugThreadManagement, spawn_hotplug_handler,
};
use beacn_lib::types::RGBA;
use beacn_lib::version::VersionNumber;
use beacn_lib::{BeacnError, UsbError};
use flume::select::Selector;
use flume::{Receiver, RecvError, Sender, TryRecvError};
use log::{debug, warn};
use std::panic::catch_unwind;
use std::thread;
use std::time::Duration;
use strum_macros::Display;
use tokio::sync::watch;
use tokio::task;
use tokio::task::JoinHandle;
//const TEMP_SPLASH: &[u8] = include_bytes!("../resources/screens/beacn-splash.jpg");

// Unifies all possible select outcomes into a single return type for the flume Selector.
enum DeviceSelectEvent {
    SelfMsg(Result<ManagerMessages, RecvError>),
    Login(Result<LoginEventTriggers, RecvError>),
    Hotplug(Result<HotPlugMessage, RecvError>),
    Audio(usize, Result<AudioMessage, RecvError>),
    Control(usize, Result<ControlMessage, RecvError>),
}

pub async fn spawn_device_manager(
    self_rx: Receiver<ManagerMessages>,
    self_tx: Sender<ToMainMessages>,
    event_tx: Sender<DeviceMessage>,
) {
    let (plug_tx, plug_rx) = flume::unbounded();
    let (manage_tx, manage_rx) = flume::unbounded();
    let (login_tx, login_rx) = flume::bounded(5);
    let (login_stop_tx, login_stop_rx) = flume::bounded(1);

    // We need a hashmap that'll map a receiver to an object
    let mut receiver_map: Vec<DeviceMap> = vec![];

    spawn_hotplug_handler(plug_tx, manage_rx).expect("Failed to Spawn HotPlug Handler");
    task::spawn(spawn_login_handler(login_tx, login_stop_rx));

    loop {
        // Ok, so when you add a receiver to a selector, it gets a callback. The callback
        // extracts the message and wraps it in a DeviceSelectEvent so all branches share
        // a single return type. After wait() returns the Selector is consumed, releasing
        // all receiver borrows so we can mutate receiver_map freely in the match arms.

        // First, we'll add our own handler, the lock detector, and the hotplug receiver.
        let mut sel = Selector::new()
            .recv(&self_rx, DeviceSelectEvent::SelfMsg)
            .recv(&login_rx, DeviceSelectEvent::Login)
            .recv(&plug_rx, DeviceSelectEvent::Hotplug);

        // Finally, we'll follow up with the 'known' devices, mapping each receiver into
        // a DeviceSelectEvent that carries its index in receiver_map.
        for (i, device) in receiver_map.iter().enumerate() {
            sel = match device {
                DeviceMap::Audio(_, _, rx) => sel.recv(rx, move |r| DeviceSelectEvent::Audio(i, r)),
                DeviceMap::Control(_, _, rx, _, _) => {
                    sel.recv(rx, move |r| DeviceSelectEvent::Control(i, r))
                }
            };
        }

        // Run the Selector (block_in_place signals to tokio that this thread will block,
        // allowing the runtime to schedule other tasks on a different thread).
        let event = tokio::task::block_in_place(|| sel.wait());

        // Ok, something's triggered us in some way, find out what.
        match event {
            DeviceSelectEvent::SelfMsg(Ok(msg)) => match msg {
                ManagerMessages::Quit => break,
            },
            DeviceSelectEvent::SelfMsg(Err(_)) => break,

            DeviceSelectEvent::Login(Ok(msg)) => {
                debug!("Received Login State Message: {msg:?}");
                // Do nothing until we have a full impl
                match msg {
                    LoginEventTriggers::Sleep(tx) => {
                        enable_devices(&receiver_map, false);
                        let _ = tx.send(());
                    }
                    LoginEventTriggers::Wake(tx) => {
                        enable_devices(&receiver_map, true);
                        let _ = tx.send(());
                    }
                    LoginEventTriggers::Lock => {
                        enable_devices(&receiver_map, false);
                    }
                    LoginEventTriggers::Unlock => {
                        enable_devices(&receiver_map, true);
                    }
                }
            }
            DeviceSelectEvent::Login(Err(_)) => {}

            DeviceSelectEvent::Hotplug(Ok(m)) => {
                match m {
                    HotPlugMessage::DeviceAttached(location, device_type) => {
                        match device_type {
                            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                                let (device, state) = match open_audio_device(location) {
                                    Ok(d) => (Some(d), DefinitionState::Running),
                                    Err(e) => (
                                        None,
                                        DefinitionState::Error(match e {
                                            BeacnError::Usb(UsbError::Access) => {
                                                ErrorType::PermissionDenied
                                            }
                                            BeacnError::Usb(UsbError::Busy) => {
                                                ErrorType::ResourceBusy
                                            }
                                            BeacnError::Usb(e) => ErrorType::Other(e.to_string()),
                                            BeacnError::Other(e) => ErrorType::Other(e.to_string()),
                                        }),
                                    ),
                                };

                                let (serial, version) = match &device {
                                    Some(d) => (d.get_serial(), d.get_version()),
                                    None => ("Unknown".to_string(), VersionNumber(0, 0, 0, 0)),
                                };

                                // Firstly, build the device definition
                                let data = DeviceDefinition {
                                    state,
                                    location,
                                    device_type,
                                    device_info: DeviceInfo { serial, version },
                                };

                                // Create a Message Bus for it
                                let (tx, rx) = flume::unbounded();

                                // Add this into our receiver array
                                if let Some(device) = device {
                                    receiver_map.push(DeviceMap::Audio(device, data.clone(), rx));
                                }

                                let arrived = DeviceArriveMessage::Audio(data, tx);
                                let message = DeviceMessage::DeviceArrived(arrived);
                                let _ = event_tx.send(message);
                            }
                            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                                // This is relatively similar, but the code paths are different. In
                                // the future, we'd be setting up button handlers, a pipeweaver
                                // connection and management.
                                let (input_tx, input_rx) = flume::unbounded();

                                let (device, state) =
                                    match open_control_device(location, Some(input_tx)) {
                                        Ok(d) => (Some(d), DefinitionState::Running),
                                        Err(e) => (
                                            None,
                                            DefinitionState::Error(match e {
                                                BeacnError::Usb(UsbError::Access) => {
                                                    ErrorType::PermissionDenied
                                                }
                                                BeacnError::Usb(UsbError::Busy) => {
                                                    ErrorType::ResourceBusy
                                                }
                                                BeacnError::Usb(e) => {
                                                    ErrorType::Other(e.to_string())
                                                }
                                                BeacnError::Other(e) => {
                                                    ErrorType::Other(e.to_string())
                                                }
                                            }),
                                        ),
                                    };

                                let (serial, version) = match &device {
                                    Some(d) => (d.get_serial(), d.get_version()),
                                    None => ("Unknown".to_string(), "Unknown".to_string()),
                                };

                                let data = DeviceDefinition {
                                    state,
                                    location,
                                    device_type,
                                    device_info: DeviceInfo {
                                        serial,
                                        version: VersionNumber::from(version),
                                    },
                                };

                                let (tx, rx) = flume::unbounded();
                                let (stop_tx, stop_rx) = watch::channel(());
                                let img_tx = tx.clone();
                                let task = spawn_pipeweaver_handler(
                                    img_tx,
                                    device_type,
                                    input_rx,
                                    stop_rx,
                                );

                                if let Some(device) = device {
                                    receiver_map.push(DeviceMap::Control(
                                        device,
                                        data.clone(),
                                        rx,
                                        stop_tx,
                                        task,
                                    ));
                                }

                                // Use the async runtime for this
                                debug!("Starting PipeWeaver Handler");

                                let arrived = DeviceArriveMessage::Control(data, tx);
                                let message = DeviceMessage::DeviceArrived(arrived);
                                let _ = event_tx.send(message);
                            }
                        }
                        let _ = self_tx.send(ToMainMessages::RequestRedraw);
                    }
                    HotPlugMessage::DeviceRemoved(location) => {
                        let _ = event_tx.send(DeviceMessage::DeviceRemoved(location));
                        receiver_map.retain(|e| match e {
                            DeviceMap::Audio(_, d, _) => d.location != location,
                            DeviceMap::Control(_, d, _, _, _) => d.location != location,
                        });

                        let _ = self_tx.send(ToMainMessages::RequestRedraw);
                    }
                    HotPlugMessage::ThreadStopped => break,
                }
            }
            DeviceSelectEvent::Hotplug(Err(_)) => break,

            DeviceSelectEvent::Audio(i, Ok(msg)) => {
                #[allow(clippy::collapsible_if)]
                if let Some(DeviceMap::Audio(dev, _, _)) = receiver_map.get(i) {
                    match msg {
                        AudioMessage::Handle(msg, resp) => {
                            let response = catch_unwind(|| dev.handle_message(msg));
                            if let Err(panic) = response {
                                // Downcast this to a standard error
                                let error = panic
                                    .downcast_ref::<String>()
                                    .cloned()
                                    .unwrap_or(String::from("Unknown Error"));
                                let _ = resp.send(Err(anyhow!(error).into()));
                            } else {
                                // Send back the original response
                                let _ = resp.send(response.unwrap());
                            }
                        }
                        AudioMessage::Linked(command) => {
                            // This code doesn't panic, just fails.
                            match command {
                                LinkedCommands::GetLinked(tx) => {
                                    let _ = tx.send(dev.get_linked_app_list());
                                }
                                LinkedCommands::SetLinked(app, tx) => {
                                    let _ = tx.send(dev.set_linked_app(app));
                                }
                            }
                        }
                    }
                }
            }
            DeviceSelectEvent::Audio(_, Err(_)) => {}

            DeviceSelectEvent::Control(i, Ok(msg)) => {
                #[allow(clippy::collapsible_if)]
                if let Some(DeviceMap::Control(dev, _, _, _, _)) = receiver_map.get(i) {
                    match msg {
                        ControlMessage::SendImage(img, x, y, tx) => {
                            let _ = tx.send(dev.set_image(x, y, &img));
                        }
                        ControlMessage::DisplayBrightness(brightness, tx) => {
                            let _ = tx.send(dev.set_display_brightness(brightness));
                        }
                        ControlMessage::ButtonBrightness(brightness, tx) => {
                            let _ = tx.send(dev.set_button_brightness(brightness));
                        }
                        ControlMessage::DimTimeout(timeout, tx) => {
                            let _ = tx.send(dev.set_dim_timeout(timeout));
                        }
                        ControlMessage::ButtonColour(button, colour, tx) => {
                            let _ = tx.send(dev.set_button_colour(button, colour));
                        }
                        ControlMessage::Enabled(enabled, tx) => {
                            let _ = tx.send(dev.set_enabled(enabled));
                        }
                        ControlMessage::KeepAlive(tx) => {
                            let _ = tx.send(dev.send_keepalive());
                        }
                    };
                }
            }
            DeviceSelectEvent::Control(_, Err(_)) => {}
        }
    }

    // Stop the dbus login handler
    let _ = login_stop_tx.send(());

    // Stop any control devices which may be active
    for device in receiver_map.iter_mut() {
        if let DeviceMap::Control(dev, _, rx, stop, task) = device {
            if stop.send(()).is_ok() {
                // This is kinda ugly, but we need to continue processing images until the task
                // is finished, so we can shut down the device cleanly.
                loop {
                    if task.is_finished() {
                        break;
                    }

                    match rx.try_recv() {
                        Ok(msg) => match msg {
                            ControlMessage::SendImage(img, x, y, tx) => {
                                let _ = tx.send(dev.set_image(x, y, &img));
                            }
                            ControlMessage::ButtonColour(button, colour, tx) => {
                                let _ = tx.send(dev.set_button_colour(button, colour));
                            }
                            _ => {}
                        },

                        Err(TryRecvError::Disconnected) => break,
                        Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(10)),
                    }
                }
            } else {
                warn!("Unable to send Stop Message");
            }
        }
    }

    // For some reason, we're stopping. If the manager channel is still open, tell it to stop.
    if !manage_tx.is_disconnected() {
        let _ = manage_tx.send(HotPlugThreadManagement::Quit);
    }

    debug!("Device Manager Stopped");
}

#[allow(unused)]
fn enable_devices(receiver_map: &Vec<DeviceMap>, enabled: bool) {
    for device in receiver_map {
        #[allow(clippy::single_match)]
        match device {
            DeviceMap::Control(dev, _, _, _, _) => {
                let _ = dev.set_enabled(enabled);
            }
            _ => {}
        }
    }
}

enum DeviceMap {
    Audio(
        Box<dyn BeacnAudioDevice>,
        DeviceDefinition,
        Receiver<AudioMessage>,
    ),
    Control(
        Box<dyn BeacnControlDevice>,
        DeviceDefinition,
        Receiver<ControlMessage>,
        watch::Sender<()>,
        JoinHandle<()>,
    ),
}

#[derive(Debug, Clone)]
pub enum DeviceMessage {
    DeviceArrived(DeviceArriveMessage),
    DeviceRemoved(DeviceLocation),
}

#[derive(Debug, Clone)]
pub enum DeviceArriveMessage {
    Audio(DeviceDefinition, Sender<AudioMessage>),
    Control(DeviceDefinition, Sender<ControlMessage>),
}

pub enum AudioMessage {
    Handle(Message, oneshot::Sender<Result<Message, BeacnError>>),
    Linked(LinkedCommands),
}

pub enum LinkedCommands {
    GetLinked(oneshot::Sender<Result<Option<Vec<LinkedApp>>, BeacnError>>),
    SetLinked(LinkedApp, oneshot::Sender<Result<(), BeacnError>>),
}

#[allow(unused)]
pub enum ControlMessage {
    Enabled(bool, oneshot::Sender<Result<(), BeacnError>>),
    KeepAlive(oneshot::Sender<Result<(), BeacnError>>),
    SendImage(Vec<u8>, u32, u32, oneshot::Sender<Result<(), BeacnError>>),
    DisplayBrightness(u8, oneshot::Sender<Result<(), BeacnError>>),
    ButtonBrightness(u8, oneshot::Sender<Result<(), BeacnError>>),
    DimTimeout(Duration, oneshot::Sender<Result<(), BeacnError>>),
    ButtonColour(
        ButtonLighting,
        RGBA,
        oneshot::Sender<Result<(), BeacnError>>,
    ),
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct DeviceDefinition {
    pub state: DefinitionState,
    pub location: DeviceLocation,
    pub device_type: DeviceType,
    pub device_info: DeviceInfo,
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: String,
    pub version: VersionNumber,
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub enum DefinitionState {
    #[default]
    Running,
    Error(ErrorType),
}

#[derive(Display, Debug, Default, Clone, Hash, PartialEq, Eq)]
pub enum ErrorType {
    PermissionDenied,
    ResourceBusy,
    Other(String),
    #[default]
    Unknown,
}
