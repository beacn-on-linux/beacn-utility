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
use crate::{ManagerMessages, ToMainMessages, runtime};
use anyhow::anyhow;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::{BeacnAudioDevice, LinkedApp, open_audio_device};
use beacn_lib::controller::{BeacnControlDevice, ButtonLighting, open_control_device};
use beacn_lib::flume::{Receiver, Selector, Sender, bounded, unbounded};
use beacn_lib::manager::{
    DeviceLocation, DeviceType, HotPlugMessage, HotPlugThreadManagement, spawn_hotplug_handler,
};
use beacn_lib::types::RGBA;
use beacn_lib::version::VersionNumber;
use beacn_lib::{BeacnError, UsbError};
use log::{debug, error};
use std::panic::catch_unwind;
use std::thread;
use std::time::Duration;
use strum_macros::Display;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::sleep;
//const TEMP_SPLASH: &[u8] = include_bytes!("../resources/screens/beacn-splash.jpg");

pub fn spawn_device_manager(
    self_rx: Receiver<ManagerMessages>,
    self_tx: Sender<ToMainMessages>,
    event_tx: Sender<DeviceMessage>,
) {
    let (plug_tx, plug_rx) = unbounded();
    let (manage_tx, manage_rx) = unbounded();
    let (login_tx, login_rx) = bounded(5);
    let (login_stop_tx, login_stop_rx) = tokio::sync::mpsc::channel(1);

    // We need a hashmap that'll map a receiver to an object
    let mut receiver_map: Vec<DeviceMap> = vec![];

    spawn_hotplug_handler(plug_tx, manage_rx).expect("Failed to Spawn HotPlug Handler");
    thread::spawn(|| spawn_login_handler(login_tx, login_stop_rx));

    let mut suspended = false;
    let mut pending_attachments: Vec<(DeviceLocation, DeviceType, Sender<()>)> = vec![];

    loop {
        let mut selector = Selector::new();
        selector = selector.recv(&self_rx, |msg| DeviceEvent::Manager(msg.ok()));
        selector = selector.recv(&login_rx, |msg| DeviceEvent::Login(msg.ok()));
        selector = selector.recv(&plug_rx, |msg| DeviceEvent::Hotplug(msg.ok()));

        for (index, device) in receiver_map.iter().enumerate() {
            match device {
                DeviceMap::Audio(_, _, rx) => {
                    selector = selector.recv(rx, move |msg| DeviceEvent::Audio(index, msg.ok()));
                }

                DeviceMap::Control(_, _, rx, _, _, _) => {
                    selector = selector.recv(rx, move |msg| DeviceEvent::Control(index, msg.ok()));
                }
            }
        }

        match selector.wait() {
            DeviceEvent::Manager(Some(ManagerMessages::Quit)) | DeviceEvent::Manager(None) => {
                break;
            }

            DeviceEvent::Login(Some(msg)) => {
                debug!("Received Login State Message: {msg:?}");

                match msg {
                    LoginEventTriggers::Sleep(tx) => {
                        suspended = true;
                        set_pipeweaver_draw_suspended(&receiver_map, true);
                        enable_devices(&receiver_map, false);
                        let _ = tx.send(());
                    }

                    LoginEventTriggers::Wake(tx) => {
                        suspended = false;

                        for (location, device_type, health_tx) in pending_attachments.drain(..) {
                            handle_device_attached(
                                location,
                                device_type,
                                health_tx,
                                &mut receiver_map,
                                &event_tx,
                                &self_tx,
                            );
                        }

                        set_pipeweaver_draw_suspended(&receiver_map, false);
                        enable_devices(&receiver_map, true);
                        let _ = tx.send(());
                    }

                    LoginEventTriggers::Lock => {
                        set_pipeweaver_draw_suspended(&receiver_map, true);
                        enable_devices(&receiver_map, false);
                    }

                    LoginEventTriggers::Unlock => {
                        set_pipeweaver_draw_suspended(&receiver_map, false);
                        enable_devices(&receiver_map, true);
                    }
                }
            }

            DeviceEvent::Login(None) => {
                break;
            }

            DeviceEvent::Hotplug(Some(msg)) => match msg {
                HotPlugMessage::DeviceAttached(location, device_type, health_tx) => {
                    if suspended {
                        pending_attachments.push((location, device_type, health_tx));
                    } else {
                        handle_device_attached(
                            location,
                            device_type,
                            health_tx,
                            &mut receiver_map,
                            &event_tx,
                            &self_tx,
                        );
                    }
                }

                HotPlugMessage::DeviceRemoved(location) => {
                    pending_attachments.retain(|(loc, _, _)| *loc != location);

                    let _ = event_tx.send(DeviceMessage::DeviceRemoved(location));

                    receiver_map.retain(|device| match device {
                        DeviceMap::Audio(_, definition, _) => definition.location != location,

                        DeviceMap::Control(_, definition, _, _, _, _) => {
                            definition.location != location
                        }
                    });

                    let _ = self_tx.send(ToMainMessages::RequestRedraw);
                }

                HotPlugMessage::ThreadStopped => {
                    break;
                }
            },

            DeviceEvent::Hotplug(None) => {
                break;
            }

            DeviceEvent::Audio(index, Some(msg)) => {
                if let Some(DeviceMap::Audio(dev, _, _)) = receiver_map.get(index) {
                    match msg {
                        AudioMessage::Handle(msg, resp) => {
                            let response = catch_unwind(|| dev.handle_message(msg));

                            match response {
                                Ok(result) => {
                                    let _ = resp.send(result);
                                }

                                Err(panic) => {
                                    let error = panic
                                        .downcast_ref::<String>()
                                        .cloned()
                                        .unwrap_or_else(|| "Unknown Error".to_string());

                                    let _ = resp.send(Err(anyhow!(error).into()));
                                }
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

            DeviceEvent::Audio(_, None) => {
                // Same behaviour as previous crossbeam implementation:
                // ignore closed device channels.
            }

            DeviceEvent::Control(index, Some(msg)) => {
                if let Some(DeviceMap::Control(dev, _, _, _, _, _)) = receiver_map.get(index) {
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

            DeviceEvent::Control(_, None) => {
                // Ignore closed device channels.
            }
        }
    }

    // Stop the dbus login handler
    let _ = login_stop_tx.blocking_send(());

    // Stop any control devices which may be active
    for device in receiver_map.iter_mut() {
        if let DeviceMap::Control(_, _, _, stop, _, _) = device {
            let _ = stop.send(());
        }
    }

    // Drain the devices until they're finished.
    runtime().block_on(async {
        loop {
            let all_done = receiver_map.iter().all(|d| match d {
                DeviceMap::Control(_, _, _, _, _, task) => task.is_finished(),
                _ => true,
            });
            if all_done {
                break;
            }

            for device in receiver_map.iter_mut() {
                if let DeviceMap::Control(dev, _, rx, _, _, _) = device {
                    match rx.try_recv() {
                        Ok(ControlMessage::SendImage(img, x, y, tx)) => {
                            let _ = tx.send(dev.set_image(x, y, &img));
                        }
                        Ok(ControlMessage::ButtonColour(button, colour, tx)) => {
                            let _ = tx.send(dev.set_button_colour(button, colour));
                        }
                        _ => {}
                    }
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
    });

    // For some reason, we're stopping. If the manager channel is still open, tell it to stop.
    let _ = manage_tx.send(HotPlugThreadManagement::Quit);
    debug!("Device Manager Stopped");
}

fn handle_device_attached(
    location: DeviceLocation,
    device_type: DeviceType,
    health_tx: Sender<()>,
    receiver_map: &mut Vec<DeviceMap>,
    event_tx: &Sender<DeviceMessage>,
    self_tx: &Sender<ToMainMessages>,
) {
    match device_type {
        DeviceType::BeacnMic | DeviceType::BeacnStudio => {
            let (device, state) = match open_audio_device(location) {
                Ok(d) => (Some(d), DefinitionState::Running),
                Err(e) => {
                    error!("Failed to open audio device: {e}");
                    (
                        None,
                        DefinitionState::Error(match e {
                            BeacnError::Usb(UsbError::Access) => ErrorType::PermissionDenied,
                            BeacnError::Usb(UsbError::Busy) => ErrorType::ResourceBusy,
                            BeacnError::Usb(e) => ErrorType::Other(e.to_string()),
                            BeacnError::Other(e) => ErrorType::Other(e.to_string()),
                        }),
                    )
                }
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
            let (tx, rx) = unbounded();

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
            let (input_tx, input_rx) = unbounded();

            let (device, state) = match open_control_device(location, Some(input_tx), health_tx) {
                Ok(d) => (Some(d), DefinitionState::Running),
                Err(e) => {
                    error!("Failed to open control device: {e}");

                    (
                        None,
                        DefinitionState::Error(match e {
                            BeacnError::Usb(UsbError::Access) => ErrorType::PermissionDenied,
                            BeacnError::Usb(UsbError::Busy) => ErrorType::ResourceBusy,
                            BeacnError::Usb(e) => ErrorType::Other(e.to_string()),
                            BeacnError::Other(e) => ErrorType::Other(e.to_string()),
                        }),
                    )
                }
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

            let (tx, rx) = unbounded();
            let (stop_tx, stop_rx) = watch::channel(());
            let (suspended_tx, suspended_rx) = watch::channel(false);
            let img_tx = tx.clone();
            let task =
                spawn_pipeweaver_handler(img_tx, device_type, input_rx, stop_rx, suspended_rx);

            if let Some(device) = device {
                receiver_map.push(DeviceMap::Control(
                    device,
                    data.clone(),
                    rx,
                    stop_tx,
                    suspended_tx,
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

#[allow(unused)]
fn enable_devices(receiver_map: &Vec<DeviceMap>, enabled: bool) {
    for device in receiver_map {
        #[allow(clippy::single_match)]
        match device {
            DeviceMap::Control(dev, _, _, _, _, _) => {
                let _ = dev.set_enabled(enabled);
            }
            _ => {}
        }
    }
}

fn set_pipeweaver_draw_suspended(receiver_map: &Vec<DeviceMap>, suspended: bool) {
    for device in receiver_map {
        if let DeviceMap::Control(_, _, _, _, draw_suspend, _) = device {
            let _ = draw_suspend.send(suspended);
        }
    }
}

enum DeviceEvent {
    Manager(Option<ManagerMessages>),
    Login(Option<LoginEventTriggers>),
    Hotplug(Option<HotPlugMessage>),
    Audio(usize, Option<AudioMessage>),
    Control(usize, Option<ControlMessage>),
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
        watch::Sender<bool>,
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
