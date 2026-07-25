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
use beacn_lib::flume::{Receiver, Sender, bounded, unbounded};
use beacn_lib::manager::{
    DeviceLocation, DeviceType, HotPlugMessage, HotPlugThreadManagement, watch_hotplug_devices,
};
use beacn_lib::types::RGBA;
use beacn_lib::version::VersionNumber;
use beacn_lib::{BeacnError, UsbError};
use futures::FutureExt;
use futures::StreamExt;
use log::{debug, error};
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::thread;
use std::time::{Duration, Instant};
use strum_macros::Display;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tokio_stream::{Stream, StreamMap};

pub async fn spawn_device_manager(
    self_rx: Receiver<ManagerMessages>,
    self_tx: Sender<ToMainMessages>,
    event_tx: Sender<DeviceMessage>,
) {
    let (plug_tx, plug_rx) = unbounded();
    let (manage_tx, manage_rx) = unbounded();
    let (login_tx, login_rx) = bounded(5);
    let (login_stop_tx, login_stop_rx) = tokio::sync::mpsc::channel(1);

    // Device state, keyed by location. The actual request receivers live in
    // `device_streams` below (StreamMap needs to own them to poll them alongside
    // everything else); this map holds what we need to act once a request arrives.
    let mut devices: HashMap<DeviceLocation, DeviceEntry> = HashMap::new();

    // A dynamically-sized set of streams we can await together, one per attached device,
    // keyed the same way as `devices`. This is the async equivalent of the old
    // `flume::Selector` built fresh from `receiver_map` every loop iteration -- except
    // StreamMap handles insertion/removal of individual streams at runtime for us instead
    // of us needing to rebuild the whole selector each pass.
    let mut device_streams: StreamMap<DeviceLocation, DeviceRequestStream> = StreamMap::new();

    // watch_hotplug_devices is beacn-lib's async-native hotplug watcher -- spawn it as a
    // task instead of spawn_hotplug_handler, which would give us a dedicated OS thread we
    // don't need now that we're on a runtime.
    tokio::spawn(watch_hotplug_devices(plug_tx, manage_rx));
    thread::spawn(|| spawn_login_handler(login_tx, login_stop_rx));

    let mut suspended = false;
    let mut pending_attachments: Vec<(DeviceLocation, DeviceType, Sender<()>)> = vec![];

    loop {
        tokio::select! {
            msg = self_rx.recv_async() => {
                match msg {
                    Ok(ManagerMessages::Quit) | Err(_) => break,
                }
            }

            msg = login_rx.recv_async() => {
                let Ok(msg) = msg else { break };
                debug!("Received Login State Message: {msg:?}");

                match msg {
                    LoginEventTriggers::Sleep(tx) => {
                        suspended = true;
                        set_pipeweaver_draw_suspended(&devices, true);
                        enable_devices(&devices, false);
                        let _ = tx.send(());
                    }

                    LoginEventTriggers::Wake(tx) => {
                        suspended = false;

                        for (location, device_type, health_tx) in pending_attachments.drain(..) {
                            handle_device_attached(
                                location,
                                device_type,
                                health_tx,
                                &mut devices,
                                &mut device_streams,
                                &event_tx,
                                &self_tx,
                            )
                            .await;
                        }

                        set_pipeweaver_draw_suspended(&devices, false);
                        enable_devices(&devices, true);
                        let _ = tx.send(());
                    }

                    LoginEventTriggers::Lock => {
                        set_pipeweaver_draw_suspended(&devices, true);
                        enable_devices(&devices, false);
                    }

                    LoginEventTriggers::Unlock => {
                        set_pipeweaver_draw_suspended(&devices, false);
                        enable_devices(&devices, true);
                    }
                }
            }

            msg = plug_rx.recv_async() => {
                let Ok(msg) = msg else { break };

                match msg {
                    HotPlugMessage::DeviceAttached(location, device_type, health_tx) => {
                        if suspended {
                            pending_attachments.push((location, device_type, health_tx));
                        } else {
                            handle_device_attached(
                                location,
                                device_type,
                                health_tx,
                                &mut devices,
                                &mut device_streams,
                                &event_tx,
                                &self_tx,
                            )
                            .await;
                        }
                    }

                    HotPlugMessage::DeviceRemoved(location) => {
                        pending_attachments.retain(|(loc, _, _)| *loc != location);

                        let _ = event_tx.send(DeviceMessage::DeviceRemoved(location.clone()));

                        devices.remove(&location);
                        device_streams.remove(&location);

                        let _ = self_tx.send(ToMainMessages::RequestRedraw);
                    }

                    HotPlugMessage::ThreadStopped => break,
                }
            }

            Some((location, req)) = device_streams.next() => {
                match req {
                    DeviceRequest::Audio(msg) => {
                        if let Some(DeviceEntry::Audio(dev, _)) = devices.get(&location) {
                            match msg {
                                AudioMessage::Handle(msg, resp) => {
                                    let response = AssertUnwindSafe(dev.handle_message(msg))
                                        .catch_unwind()
                                        .await;

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
                                        let _ = tx.send(dev.get_linked_app_list().await);
                                    }

                                    LinkedCommands::SetLinked(app, tx) => {
                                        let _ = tx.send(dev.set_linked_app(app).await);
                                    }
                                },
                            }
                        }
                    }

                    DeviceRequest::Control(msg) => {
                        if let Some(DeviceEntry::Control(dev, ..)) = devices.get(&location) {
                            match msg {
                                ControlMessage::SendImage(img, x, y, tx) => {
                                    let result = dev.set_image(x, y, &img);
                                    let _ = tx.send(result);
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

    // Stop the dbus login handler
    let _ = login_stop_tx.send(()).await;

    // Stop any control devices which may be active
    for device in devices.values() {
        if let DeviceEntry::Control(_, _, stop, _, _) = device {
            let _ = stop.send(());
        }
    }

    // Drain the devices until they're finished. No more `runtime().block_on(...)` needed
    // here -- we're already running on the runtime, this is just the tail of the same
    // async fn.
    loop {
        let all_done = devices.values().all(|d| match d {
            DeviceEntry::Control(_, _, _, _, task) => task.is_finished(),
            _ => true,
        });
        if all_done {
            break;
        }

        tokio::select! {
            Some((location, req)) = device_streams.next() => {
                if let DeviceRequest::Control(msg) = req {
                    if let Some(DeviceEntry::Control(dev, ..)) = devices.get(&location) {
                        match msg {
                            ControlMessage::SendImage(img, x, y, tx) => {
                                let _ = tx.send(dev.set_image(x, y, &img));
                            }
                            ControlMessage::ButtonColour(button, colour, tx) => {
                                let _ = tx.send(dev.set_button_colour(button, colour));
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(10)) => {}
        }
    }

    // For some reason, we're stopping. If the manager channel is still open, tell it to stop.
    let _ = manage_tx.send(HotPlugThreadManagement::Quit);
    debug!("Device Manager Stopped");
}

async fn handle_device_attached(
    location: DeviceLocation,
    device_type: DeviceType,
    health_tx: Sender<()>,
    devices: &mut HashMap<DeviceLocation, DeviceEntry>,
    device_streams: &mut StreamMap<DeviceLocation, DeviceRequestStream>,
    event_tx: &Sender<DeviceMessage>,
    self_tx: &Sender<ToMainMessages>,
) {
    match device_type {
        DeviceType::BeacnMic | DeviceType::BeacnStudio => {
            let (device, state) = match open_audio_device(location.clone()).await {
                Ok(d) => (Some(d), DefinitionState::Running),
                Err(e) => {
                    error!("Failed to open audio device: {e}");
                    (
                        None,
                        DefinitionState::Error(match e {
                            BeacnError::Usb(UsbError::PermissionDenied) => {
                                ErrorType::PermissionDenied
                            }
                            BeacnError::Usb(UsbError::Busy) => ErrorType::ResourceBusy,
                            BeacnError::Usb(e) => ErrorType::Other(format!("{:?}", e)),
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
                location: location.clone(),
                device_type,
                device_info: DeviceInfo { serial, version },
            };

            // Create a Message Bus for it
            let (tx, rx) = unbounded();

            // Add this into our device map + the stream map we select over
            if let Some(device) = device {
                devices.insert(location.clone(), DeviceEntry::Audio(device, data.clone()));
                device_streams.insert(location, box_audio_stream(rx));
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

            let (device, state) =
                match open_control_device(location.clone(), Some(input_tx), health_tx).await {
                    Ok(d) => (Some(d), DefinitionState::Running),
                    Err(e) => {
                        error!("Failed to open control device: {e}");

                        (
                            None,
                            DefinitionState::Error(match e {
                                BeacnError::Usb(UsbError::PermissionDenied) => {
                                    ErrorType::PermissionDenied
                                }
                                BeacnError::Usb(UsbError::Busy) => ErrorType::ResourceBusy,
                                BeacnError::Usb(e) => ErrorType::Other(format!("{:?}", e)),
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
                location: location.clone(),
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
                devices.insert(
                    location.clone(),
                    DeviceEntry::Control(device, data.clone(), stop_tx, suspended_tx, task),
                );
                device_streams.insert(location, box_control_stream(rx));
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

/// Turn a device's request receiver into a stream of the shared `DeviceRequest` enum and
/// box it, so audio and control device streams -- despite carrying different message
/// types -- can live side by side in the same `StreamMap`.
fn box_audio_stream(rx: Receiver<AudioMessage>) -> DeviceRequestStream {
    Box::pin(rx.into_stream().map(DeviceRequest::Audio))
}

fn box_control_stream(rx: Receiver<ControlMessage>) -> DeviceRequestStream {
    Box::pin(rx.into_stream().map(DeviceRequest::Control))
}

type DeviceRequestStream = Pin<Box<dyn Stream<Item = DeviceRequest> + Send>>;

enum DeviceRequest {
    Audio(AudioMessage),
    Control(ControlMessage),
}

#[allow(unused)]
fn enable_devices(devices: &HashMap<DeviceLocation, DeviceEntry>, enabled: bool) {
    for device in devices.values() {
        if let DeviceEntry::Control(dev, ..) = device {
            let _ = dev.set_enabled(enabled);
        }
    }
}

fn set_pipeweaver_draw_suspended(devices: &HashMap<DeviceLocation, DeviceEntry>, suspended: bool) {
    for device in devices.values() {
        if let DeviceEntry::Control(_, _, _, draw_suspend, _) = device {
            let _ = draw_suspend.send(suspended);
        }
    }
}

enum DeviceEntry {
    Audio(Box<dyn BeacnAudioDevice>, DeviceDefinition),
    Control(
        Box<dyn BeacnControlDevice>,
        DeviceDefinition,
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

#[derive(Debug)]
pub enum AudioMessage {
    Handle(Message, oneshot::Sender<Result<Message, BeacnError>>),
    Linked(LinkedCommands),
}

#[derive(Debug)]
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
