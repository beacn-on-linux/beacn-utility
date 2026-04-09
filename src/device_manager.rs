/*
  This file primarily manages hot-plugging opening devices, and device messaging.
*/
use crate::integrations::pipeweaver::spawn_pipeweaver_handler;
use crate::managers::login::{LoginEventTriggers, spawn_login_handler};
use crate::ui::states::pipeweaver_state::SharedPipeweaverState;
use crate::{ManagerMessages, ToMainMessages};
use anyhow::anyhow;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::{BeacnAudioDevice, LinkedApp, open_audio_device};
use beacn_lib::controller::{BeacnControlDevice, ButtonLighting, open_control_device};
use beacn_lib::crossbeam::channel;
use beacn_lib::crossbeam::channel::{Receiver, Select, Sender};
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

pub fn spawn_device_manager(
    self_rx: Receiver<ManagerMessages>,
    self_tx: Sender<ToMainMessages>,
    event_tx: Sender<DeviceMessage>,
) {
    let (plug_tx, plug_rx) = channel::unbounded();
    let (manage_tx, manage_rx) = channel::unbounded();
    let (login_tx, login_rx) = channel::bounded(5);
    let (login_stop_tx, login_stop_rx) = tokio::sync::mpsc::channel(1);
    let mut receiver_map: Vec<DeviceMap> = vec![];

    if let Err(e) = spawn_hotplug_handler(plug_tx, manage_rx) {
        error!("Failed to spawn HotPlug Handler: {e}");
        let _ = self_tx.send(ToMainMessages::Quit);
        return;
    }

    thread::spawn(move || {
        if let Err(e) = spawn_login_handler(login_tx, login_stop_rx) {
            error!("Login handler exited with error: {e}");
        }
    });

    loop {
        let mut selector = Select::new();
        let self_index = selector.recv(&self_rx);
        let lock_index = selector.recv(&login_rx);
        let hotplug_index = selector.recv(&plug_rx);

        let mut device_indices: HashMap<usize, usize> = HashMap::new();
        for (i, device) in receiver_map.iter().enumerate() {
            let index = match device {
                DeviceMap::Audio(_, _, rx) => selector.recv(rx),
                DeviceMap::Control(_, _, rx) => selector.recv(rx),
            };
            device_indices.insert(index, i);
        }

        let operation = selector.select();
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
                    match msg {
                        LoginEventTriggers::Sleep(tx) => {
                            enable_devices(&receiver_map, false);
                            let _ = tx.send(());
                        }
                        LoginEventTriggers::Wake(tx) => {
                            enable_devices(&receiver_map, true);
                            let _ = tx.send(());
                        }
                        LoginEventTriggers::Lock => enable_devices(&receiver_map, false),
                        LoginEventTriggers::Unlock => enable_devices(&receiver_map, true),
                    }
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
                                    None => ("Unknown".to_string(), VersionNumber(0, 0, 0, 0)),
                                };

                                let data = DeviceDefinition {
                                    state,
                                    location,
                                    device_type,
                                    device_info: DeviceInfo { serial, version },
                                };

                                let (tx, rx) = channel::unbounded();
                                if let Some(device) = device {
                                    receiver_map.push(DeviceMap::Audio(device, data.clone(), rx));
                                }
                                let _ = event_tx.send(DeviceMessage::DeviceArrived(
                                    DeviceArriveMessage::Audio(data, tx),
                                ));
                            }
                            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                                let (input_tx, input_rx) = channel::unbounded();
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
                                let (tx, rx) = channel::unbounded();
                                let mut pipeweaver_state = None;
                                if let Some(device) = device {
                                    receiver_map.push(DeviceMap::Control(device, data.clone(), rx));
                                    let (shared_state, cmd_rx) = SharedPipeweaverState::new();
                                    spawn_pipeweaver_handler(
                                        tx.clone(),
                                        device_type,
                                        input_rx,
                                        shared_state.clone(),
                                        cmd_rx,
                                    );
                                    pipeweaver_state = Some(shared_state);
                                }
                                let _ = event_tx.send(DeviceMessage::DeviceArrived(
                                    DeviceArriveMessage::Control(data, tx, pipeweaver_state),
                                ));
                            }
                        }
                        let _ = self_tx.send(ToMainMessages::RequestRedraw);
                    }
                    HotPlugMessage::DeviceRemoved(location) => {
                        let _ = event_tx.send(DeviceMessage::DeviceRemoved(location));
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
            i => {
                if let Some(device) = device_indices.get(&i)
                    && let Some(device) = receiver_map.get(*device)
                {
                    match device {
                        DeviceMap::Audio(dev, _, rx) => {
                            if let Ok(msg) = operation.recv(rx) {
                                match msg {
                                    AudioMessage::Handle(msg, resp) => {
                                        let response = catch_unwind(|| dev.handle_message(msg));
                                        if let Err(panic) = response {
                                            let error = panic
                                                .downcast_ref::<String>()
                                                .cloned()
                                                .unwrap_or(String::from("Unknown Error"));
                                            let _ = resp.send(Err(anyhow!(error).into()));
                                        } else {
                                            let _ = resp.send(response.unwrap());
                                        }
                                    }
                                    AudioMessage::Linked(command) => match command {
                                        LinkedCommands::GetLinked(tx) => {
                                            let _ = tx.send(dev.get_linked_app_list());
                                        }
                                        LinkedCommands::SetLinked(app, tx) => {
                                            let _ = tx.send(dev.set_linked_app(app));
                                        }
                                    },
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
                                    ControlMessage::KeepAlive(tx) => {
                                        let _ = tx.send(dev.send_keepalive());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = manage_tx.send(HotPlugThreadManagement::Quit);
    let _ = login_stop_tx.blocking_send(());
    debug!("Device Manager Stopped");
}

#[allow(unused)]
fn enable_devices(receiver_map: &Vec<DeviceMap>, enabled: bool) {
    for device in receiver_map {
        if let DeviceMap::Control(dev, _, _) = device {
            let _ = dev.set_enabled(enabled);
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
    Control(
        DeviceDefinition,
        Sender<ControlMessage>,
        Option<SharedPipeweaverState>,
    ),
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
