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
use crate::ManagerMessages;
use crate::devices::states::audio::AudioState;
use crate::devices::states::control::ControlState;
use crate::integrations::pipeweaver::spawn_pipeweaver_handler;
use crate::managers::LoginEventTriggers;
use anyhow::anyhow;
use beacn_lib::audio::messages::Message as AMessage;
use beacn_lib::audio::{BeacnAudioDevice, LinkedApp, open_audio_device};
use beacn_lib::controller::messages::Message as CMessage;
use beacn_lib::controller::{BeacnControlDevice, open_control_device};
use beacn_lib::flume::{Receiver, Sender, bounded, unbounded};
use beacn_lib::manager::{
    DeviceLocation, DeviceType, HotPlugMessage, HotPlugThreadManagement, watch_hotplug_devices,
};
use beacn_lib::version::VersionNumber;
use beacn_lib::{BeacnError, UsbError};
//use futures::FutureExt;
use iced::futures::FutureExt;
use log::{debug, error};
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;
use strum_macros::Display;
use tokio::sync::watch;
use tokio::task::JoinHandle;

pub(crate) async fn spawn_device_manager(
    self_rx: Receiver<ManagerMessages>,
    event_tx: Sender<DeviceMessage>,
) {
    let (plug_tx, plug_rx) = unbounded();
    let (manage_tx, manage_rx) = unbounded();

    #[cfg_attr(not(unix), allow(unused))]
    let (login_tx, login_rx) = bounded(5);

    #[cfg_attr(not(unix), allow(unused))]
    let (login_stop_tx, login_stop_rx) = tokio::sync::mpsc::channel(1);

    // Device state, keyed by location.
    let mut devices: HashMap<DeviceLocation, DeviceEntry> = HashMap::new();

    // Small List of forwarding tasks
    let (device_event_tx, device_event_rx) = unbounded();
    let mut forwarders: HashMap<DeviceLocation, JoinHandle<()>> = HashMap::new();

    // This is basically a FIFO channel for device arrivals
    let (order_tx, order_rx) = unbounded();
    {
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            while let Ok(rx) = order_rx.recv_async().await {
                if let Ok(msg) = rx.await {
                    let _ = event_tx.send(msg);
                }
            }
        });
    }

    // watch_hotplug_devices is beacn-lib's async-native hotplug watcher -- spawn it as a
    // task instead of spawn_hotplug_handler, which would give us a dedicated OS thread we
    // don't need now that we're on a runtime.
    tokio::spawn(watch_hotplug_devices(plug_tx, manage_rx));

    #[cfg(unix)]
    {
        use crate::managers::login::spawn_login_handler;
        tokio::spawn(spawn_login_handler(login_tx, login_stop_rx));
    }

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
                        enable_devices(&devices, false).await;
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
                                &mut forwarders,
                                &device_event_tx,
                                &order_tx,
                            )
                            .await;
                        }

                        set_pipeweaver_draw_suspended(&devices, false);
                        enable_devices(&devices, true).await;
                        let _ = tx.send(());
                    }

                    LoginEventTriggers::Lock => {
                        set_pipeweaver_draw_suspended(&devices, true);
                        enable_devices(&devices, false).await;
                    }

                    LoginEventTriggers::Unlock => {
                        set_pipeweaver_draw_suspended(&devices, false);
                        enable_devices(&devices, true).await;
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
                                &mut forwarders,
                                &device_event_tx,
                                &order_tx,
                            )
                            .await;
                        }
                    }

                    HotPlugMessage::DeviceRemoved(location) => {
                        pending_attachments.retain(|(loc, _, _)| *loc != location);

                        let _ = event_tx.send(DeviceMessage::DeviceRemoved(location.clone()));

                        devices.remove(&location);
                        if let Some(forwarder) = forwarders.remove(&location) {
                            forwarder.abort();
                        }

                        //let _ = self_tx.send(ToMainMessages::RequestRedraw);
                    }

                    HotPlugMessage::ThreadStopped => break,
                }
            }

            Ok((location, req)) = device_event_rx.recv_async() => {
                match req {
                    DeviceRequest::Audio(msg) => {
                        if let Some(DeviceEntry::Audio(dev)) = devices.get(&location) {
                            match msg {
                                AudioMessage::Handle(msg, resp) => {
                                    let response = AssertUnwindSafe(dev.handle_message(msg)).catch_unwind().await;

                                    match response {
                                        Ok(result) => {
                                            let _ = resp.send(result);
                                        }

                                        Err(panic) => {
                                            let error = panic.downcast_ref::<String>().cloned().unwrap_or_else(|| "Unknown Error".to_string());
                                            let _ = resp.send(Err(anyhow!(error).into()));
                                        }
                                    }
                                }

                                AudioMessage::Linked(command) => match command {
                                    LinkedCommands::GetLinked(tx) => {
                                        let _ = tx.send(dev.get_linked_apps().await);
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
                                ControlMessage::Handle(msg, resp) => {
                                    let response = AssertUnwindSafe(dev.handle_message(msg)).catch_unwind().await;

                                    match response {
                                        Ok(result) => {
                                            let _ = resp.send(result);
                                        }

                                        Err(panic) => {
                                            let error = panic.downcast_ref::<String>().cloned().unwrap_or_else(|| "Unknown Error".to_string());
                                            let _ = resp.send(Err(anyhow!(error).into()));
                                        }
                                    }
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
        if let DeviceEntry::Control(_, stop, _, _) = device {
            let _ = stop.send(true);
        }
    }

    // Drain the devices until they're finished. No more `runtime().block_on(...)` needed
    // here -- we're already running on the runtime, this is just the tail of the same
    // async fn.
    loop {
        let all_done = devices.values().all(|d| match d {
            DeviceEntry::Control(_, _, _, task) => task.is_finished(),
            _ => true,
        });
        if all_done {
            break;
        }

        tokio::select! {
            Ok((location, req)) = device_event_rx.recv_async() => {
                if let DeviceRequest::Control(msg) = req
                    && let Some(DeviceEntry::Control(dev, ..)) = devices.get(&location) {
                        match msg {
                            ControlMessage::Handle(msg, resp) => {
                                let _ = resp.send(dev.handle_message(msg).await);
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

#[allow(clippy::too_many_arguments)]
async fn handle_device_attached(
    location: DeviceLocation,
    device_type: DeviceType,
    health_tx: Sender<()>,
    devices: &mut HashMap<DeviceLocation, DeviceEntry>,
    forwarders: &mut HashMap<DeviceLocation, JoinHandle<()>>,
    device_event_tx: &Sender<(DeviceLocation, DeviceRequest)>,
    order_tx: &Sender<oneshot::Receiver<DeviceMessage>>,
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

            // Add this into our device map, and spawn a task to forward its requests
            if let Some(device) = device {
                devices.insert(location.clone(), DeviceEntry::Audio(device));
                forwarders.insert(
                    location.clone(),
                    spawn_forwarder(location, rx, device_event_tx.clone(), DeviceRequest::Audio),
                );
            }

            // Reserve a Slot in the Queue
            let (arrive_tx, arrive_rx) = oneshot::channel();
            let _ = order_tx.send(arrive_rx);

            // Complete against the queue
            tokio::spawn(async move {
                let state = AudioState::load_settings_async(data, tx).await;
                let arrived = DeviceArriveMessage::Audio(state);
                let message = DeviceMessage::DeviceArrived(arrived);
                let _ = arrive_tx.send(message);
            });
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
                None => ("Unknown".to_string(), VersionNumber(0, 0, 0, 0)),
            };

            let data = DeviceDefinition {
                state,
                location: location.clone(),
                device_type,
                device_info: DeviceInfo { serial, version },
            };

            let (tx, rx) = unbounded();
            let (stop_tx, stop_rx) = watch::channel(false);
            let (suspended_tx, suspended_rx) = watch::channel(false);
            let img_tx = tx.clone();
            let task =
                spawn_pipeweaver_handler(img_tx, device_type, input_rx, stop_rx, suspended_rx);

            if let Some(device) = device {
                devices.insert(
                    location.clone(),
                    DeviceEntry::Control(device, stop_tx, suspended_tx, task),
                );
                forwarders.insert(
                    location.clone(),
                    spawn_forwarder(
                        location,
                        rx,
                        device_event_tx.clone(),
                        DeviceRequest::Control,
                    ),
                );
            }

            // Reserve a Slot in the Queue
            let (arrive_tx, arrive_rx) = oneshot::channel();
            let _ = order_tx.send(arrive_rx);
            tokio::spawn(async move {
                let state = ControlState::load_settings_async(data, tx).await;
                let arrived = DeviceArriveMessage::Control(state);
                let message = DeviceMessage::DeviceArrived(arrived);
                let _ = arrive_tx.send(message);
            });
        }
    }
    //let _ = self_tx.send(ToMainMessages::RequestRedraw);
}

/// Spawn a small task that just loops on `rx` and forwards everything it receives into
/// the shared `device_event_tx`, tagged with `location` and wrapped into the common
/// `DeviceRequest` enum via `wrap` (`DeviceRequest::Audio` / `DeviceRequest::Control`).
/// Exits on its own once `rx`'s channel closes; also explicitly `.abort()`ed on device
/// removal so it doesn't linger.
fn spawn_forwarder<M: Send + 'static>(
    location: DeviceLocation,
    rx: Receiver<M>,
    device_event_tx: Sender<(DeviceLocation, DeviceRequest)>,
    wrap: impl Fn(M) -> DeviceRequest + Send + 'static,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv_async().await {
            if device_event_tx.send((location.clone(), wrap(msg))).is_err() {
                break;
            }
        }
    })
}

enum DeviceRequest {
    Audio(AudioMessage),
    Control(ControlMessage),
}

#[allow(unused)]
async fn enable_devices(devices: &HashMap<DeviceLocation, DeviceEntry>, enabled: bool) {
    for device in devices.values() {
        if let DeviceEntry::Control(dev, ..) = device {
            let message = CMessage::Enabled(enabled);
            let _ = dev.handle_message(message).await;
        }
    }
}

fn set_pipeweaver_draw_suspended(devices: &HashMap<DeviceLocation, DeviceEntry>, suspended: bool) {
    for device in devices.values() {
        if let DeviceEntry::Control(_, _, draw_suspend, _) = device {
            let _ = draw_suspend.send(suspended);
        }
    }
}

enum DeviceEntry {
    Audio(Box<dyn BeacnAudioDevice>),
    Control(
        Arc<Box<dyn BeacnControlDevice>>,
        watch::Sender<bool>,
        watch::Sender<bool>,
        JoinHandle<()>,
    ),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum DeviceMessage {
    DeviceArrived(DeviceArriveMessage),
    DeviceRemoved(DeviceLocation),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum DeviceArriveMessage {
    Audio(AudioState),
    Control(ControlState),
}

#[derive(Debug)]
pub enum AudioMessage {
    Handle(AMessage, oneshot::Sender<Result<AMessage, BeacnError>>),
    Linked(LinkedCommands),
}

#[derive(Debug)]
pub enum LinkedCommands {
    GetLinked(oneshot::Sender<Result<Option<Vec<LinkedApp>>, BeacnError>>),
    SetLinked(LinkedApp, oneshot::Sender<Result<(), BeacnError>>),
}

#[allow(unused)]
pub enum ControlMessage {
    Handle(CMessage, oneshot::Sender<Result<CMessage, BeacnError>>),
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
