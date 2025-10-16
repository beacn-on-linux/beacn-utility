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
use crate::{runtime, ManagerMessages, ToMainMessages};
use crate::device_manager::DeviceMessage::DeviceRemoved;
use crate::integrations::pipeweaver::{spawn_pipeweaver_handler};
//use crate::integrations::pipeweaver::perform_test_render;
use crate::device_manager::ControlMessage::SendImage;
use crate::managers::login::spawn_login_handler;
use anyhow::anyhow;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::{BeacnAudioDevice, LinkedApp, open_audio_device};
use beacn_lib::controller::{BeacnControlDevice, ButtonLighting, open_control_device};
use beacn_lib::crossbeam::channel;
use beacn_lib::crossbeam::channel::internal::SelectHandle;
use beacn_lib::crossbeam::channel::{Receiver, Select, Sender, tick};
use beacn_lib::manager::{
    DeviceLocation, DeviceType, HotPlugMessage, HotPlugThreadManagement, spawn_hotplug_handler,
};
use beacn_lib::types::RGBA;
use beacn_lib::version::VersionNumber;
use beacn_lib::{BeacnError, UsbError};
use log::{debug, error};
use std::collections::HashMap;
use std::panic::catch_unwind;
use std::thread;
use std::time::Duration;
use strum_macros::Display;

const TEMP_SPLASH: &[u8] = include_bytes!("../resources/screens/beacn-splash.jpg");

pub fn spawn_device_manager(
    self_rx: Receiver<ManagerMessages>,
    self_tx: Sender<ToMainMessages>,
    event_tx: Sender<DeviceMessage>,
) {
    let (plug_tx, plug_rx) = channel::unbounded();
    let (manage_tx, manage_rx) = channel::unbounded();
    let (login_tx, login_rx) = channel::bounded(5);
    let (login_stop_tx, login_stop_rx) = tokio::sync::mpsc::channel(1);

    // We need a hashmap that'll map a receiver to an object
    let mut receiver_map: Vec<DeviceMap> = vec![];

    let keepalive = tick(Duration::from_secs(10));

    spawn_hotplug_handler(plug_tx, manage_rx).expect("Failed to Spawn HotPlug Handler");
    thread::spawn(|| spawn_login_handler(login_tx, login_stop_rx));

    loop {
        let mut selector = Select::new();
        // Ok, so when you add a receiver to a selector, it gets an index. This index lets us
        // know which receiver has triggered a message.

        // First, we'll add our own handler
        let self_index = selector.recv(&self_rx);

        // Add the Lock Detector
        let lock_index = selector.recv(&login_rx);

        // Next, the hotplug receiver
        let hotplug_index = selector.recv(&plug_rx);

        // Now the Keepalive ticker
        let keepalive_index = selector.recv(&keepalive);

        // Finally, we'll follow up with the 'known' devices, we'll map the crossbeam index with
        // their index in the receiver_map.
        let mut device_indices: HashMap<usize, usize> = HashMap::new();
        for (i, device) in receiver_map.iter().enumerate() {
            let index = match device {
                DeviceMap::Audio(_, _, rx) => selector.recv(rx),
                DeviceMap::Control(_, _, rx) => selector.recv(rx),
            };
            device_indices.insert(index, i);
        }

        // Run the Selector
        let operation = selector.select();

        // Ok, something's triggered us in some way, find out what.
        match operation.index() {
            i if i == self_index => {
                if let Ok(msg) = operation.recv(&self_rx) {
                    match msg {
                        ManagerMessages::Quit => break,
                    }
                }
            }
            i if i == lock_index => {
                if let Ok(msg) = operation.recv(&login_rx) {
                    debug!("Received Login State Message: {msg:?}");
                    // Do nothing until we have a full impl
                    // match msg {
                    //     LoginEventTriggers::Sleep(tx) => {
                    //         enable_devices(&receiver_map, false);
                    //         let _ = tx.send(());
                    //     }
                    //     LoginEventTriggers::Wake(tx) => {
                    //         enable_devices(&receiver_map, true);
                    //         let _ = tx.send(());
                    //     }
                    //     LoginEventTriggers::Lock => {
                    //         enable_devices(&receiver_map, false);
                    //     }
                    //     LoginEventTriggers::Unlock => {
                    //         enable_devices(&receiver_map, true);
                    //     }
                    // }
                }
            }
            i if i == hotplug_index => match operation.recv(&plug_rx) {
                Ok(m) => match m {
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
                                    None => ("Unknown".to_string(), "Unknown".to_string()),
                                };

                                // Firstly, build the device definition
                                let data = DeviceDefinition {
                                    state,
                                    location,
                                    device_type,
                                    device_info: DeviceInfo {
                                        serial,
                                        version: VersionNumber::from(version),
                                    },
                                };

                                // Create a Message Bus for it
                                let (tx, rx) = channel::unbounded();

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
                                let (device, state) = match open_control_device(location, None) {
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

                                let (tx, rx) = channel::unbounded();
                                if let Some(device) = device {
                                    receiver_map.push(DeviceMap::Control(device, data.clone(), rx));
                                }

                                // Send a splash to the device.
                                // let img_tx = tx.clone();
                                // let (tx2, rx2) = oneshot::channel();
                                // let img = Vec::from(TEMP_SPLASH);
                                // let _ = img_tx.send(SendImage(img, 0, 0, tx2));
                                // debug!("Waiting Result..");
                                // let _ = rx2.recv();
                                // debug!("Got Result?");

                                // Use the async runtime for this
                                debug!("Starting PipeWeaver Handler");
                                let img_tx = tx.clone();
                                runtime().spawn(spawn_pipeweaver_handler(img_tx));

                                let arrived = DeviceArriveMessage::Control(data, tx);
                                let message = DeviceMessage::DeviceArrived(arrived);
                                let _ = event_tx.send(message);
                            }
                        }
                        let _ = self_tx.send(ToMainMessages::RequestRedraw);
                    }
                    HotPlugMessage::DeviceRemoved(location) => {
                        let _ = event_tx.send(DeviceRemoved(location));
                        receiver_map.retain(|e| match e {
                            DeviceMap::Audio(_, d, _) => d.location != location,
                            DeviceMap::Control(_, d, _) => d.location != location,
                        });

                        let _ = self_tx.send(ToMainMessages::RequestRedraw);
                    }
                    HotPlugMessage::ThreadStopped => break,
                },
                Err(_) => break,
            },
            i if i == keepalive_index => match operation.recv(&keepalive) {
                Ok(_instant) => {
                    // Disable the keepalive for now, show the message then let the device turn off
                    for device in &receiver_map {
                        if let DeviceMap::Control(device, _, _) = device {
                            let _ = device.send_keepalive();
                        }
                    }
                }
                Err(e) => {
                    error!("KeepAlive Poller Failed, {e}");
                    break;
                }
            },
            i => {
                // Find the specific device for this index
                #[allow(clippy::collapsible_if)]
                if let Some(device) = device_indices.get(&i) {
                    if let Some(device) = receiver_map.get(*device) {
                        match device {
                            DeviceMap::Audio(dev, _, rx) => {
                                if let Ok(msg) = operation.recv(rx) {
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
                            DeviceMap::Control(dev, _, rx) => {
                                if let Ok(msg) = operation.recv(rx) {
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
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // For some reason, we're stopping. If the manager channel is still open, tell it to stop.
    if manage_tx.is_ready() {
        let _ = manage_tx.send(HotPlugThreadManagement::Quit);
    }

    // Stop the dbus login handler
    let _ = login_stop_tx.blocking_send(());

    debug!("Device Manager Stopped");
}

#[allow(unused)]
fn enable_devices(receiver_map: &Vec<DeviceMap>, enabled: bool) {
    for device in receiver_map {
        #[allow(clippy::single_match)]
        match device {
            DeviceMap::Control(dev, _, _) => {
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
