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
use crate::device_manager::DeviceMessage::DeviceRemoved;
use anyhow::{Result, anyhow};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::{BeacnAudioDevice, open_audio_device};
use beacn_lib::controller::{BeacnControlDevice, ButtonLighting, open_control_device};
use beacn_lib::crossbeam::channel;
use beacn_lib::crossbeam::channel::internal::SelectHandle;
use beacn_lib::crossbeam::channel::{Receiver, RecvError, Select, Sender, tick};
use beacn_lib::manager::{
    DeviceLocation, DeviceType, HotPlugMessage, HotPlugThreadManagement, spawn_hotplug_handler,
};
use beacn_lib::types::RGBA;
use beacn_lib::version::VersionNumber;
use log::{debug, error};
use std::collections::HashMap;
use std::panic;
use std::panic::catch_unwind;
use std::time::{Duration, Instant};

pub fn spawn_device_manager(self_rx: Receiver<ManagerMessages>, event_tx: Sender<DeviceMessage>) {
    let (plug_tx, plug_rx) = channel::unbounded();
    let (manage_tx, manage_rx) = channel::unbounded();

    // We need a hashmap that'll map a receiver to an object
    let mut receiver_map: Vec<DeviceMap> = vec![];
    let mut context = None;

    let controller_keepalive = tick(Duration::from_secs(10));

    spawn_hotplug_handler(plug_tx, manage_rx).expect("Failed to Spawn HotPlug Handler");
    loop {
        let mut selector = Select::new();
        // Ok, so when you add a receiver to a selector, it gets an index. This index lets us
        // know which receiver has triggered a message.

        // First, we'll add our own handler
        let self_index = selector.recv(&self_rx);

        // Next, the hotplug receiver
        let hotplug_index = selector.recv(&plug_rx);

        // Now the Keepalive ticker
        let keepalive_index = selector.recv(&controller_keepalive);

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
                        ManagerMessages::SetContext(ctx) => context = ctx,
                        ManagerMessages::Quit => break,
                    }
                }
            }
            i if i == hotplug_index => match operation.recv(&plug_rx) {
                Ok(m) => match m {
                    HotPlugMessage::DeviceAttached(location, device_type) => {
                        match device_type {
                            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                                let device = match open_audio_device(location) {
                                    Ok(d) => d,

                                    // TODO: We should have some kinda 'Failed' state
                                    // It's important to let the App know that we are aware that a
                                    // device exists, but isn't functional.
                                    Err(e) => panic!("Failed to Open Device: {}", e),
                                };

                                // Firstly, build the device definition
                                let data = DeviceDefinition {
                                    location,
                                    device_type,
                                    device_info: DeviceInfo {
                                        serial: device.get_serial(),
                                        version: VersionNumber::from(device.get_version()),
                                    },
                                };

                                // Create a Message Bus for it
                                let (tx, rx) = channel::unbounded();

                                // Add this into our receiver array
                                receiver_map.push(DeviceMap::Audio(device, data.clone(), rx));
                                let arrived = DeviceArriveMessage::Audio(data, tx);
                                let message = DeviceMessage::DeviceArrived(arrived);
                                let _ = event_tx.send(message);
                            }
                            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                                //continue;
                                // This is relatively similar, but the code paths are different. In
                                // the future, we'd be setting up button handlers, a pipeweaver
                                // connection and management.

                                let device = match open_control_device(location, None) {
                                    Ok(d) => d,
                                    Err(e) => panic!("Failed to Open Device: {}", e),
                                };

                                let data = DeviceDefinition {
                                    location,
                                    device_type,
                                    device_info: DeviceInfo {
                                        serial: device.get_serial(),
                                        version: VersionNumber::from(device.get_version()),
                                    },
                                };

                                let (tx, rx) = channel::unbounded();
                                receiver_map.push(DeviceMap::Control(device, data.clone(), rx));

                                let arrived = DeviceArriveMessage::Control(data, tx);
                                let message = DeviceMessage::DeviceArrived(arrived);
                                let _ = event_tx.send(message);
                            }
                        }
                        if let Some(context) = &context {
                            context.request_repaint();
                        }
                    }
                    HotPlugMessage::DeviceRemoved(location) => {
                        let _ = event_tx.send(DeviceRemoved(location));
                        receiver_map.retain(|e| match e {
                            DeviceMap::Audio(_, d, _) => d.location != location,
                            DeviceMap::Control(_, d, _) => d.location != location,
                        });

                        if let Some(context) = &context {
                            context.request_repaint();
                        }
                    }
                    HotPlugMessage::ThreadStopped => break,
                },
                Err(_) => break,
            },
            i if i == keepalive_index => match controller_keepalive.recv() {
                Ok(_) => {
                    debug!("Sending KeepAlive");
                    for device in &receiver_map {
                        if let DeviceMap::Control(device, _, _) = device {
                            let _ = device.send_keepalive();
                        }
                    }
                }
                Err(e) => {
                    error!("KeepAlive Poller Failed, {}", e);
                    break;
                }
            },
            i => {
                // Find the specific device for this index
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
                                                let _ = resp.send(Err(anyhow!(error)));
                                            } else {
                                                // Send back the original response
                                                let _ = resp.send(response.unwrap());
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

    debug!("Device Manager Stopped");
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

pub enum DeviceMessage {
    DeviceArrived(DeviceArriveMessage),
    DeviceRemoved(DeviceLocation),
}

pub enum DeviceArriveMessage {
    Audio(DeviceDefinition, Sender<AudioMessage>),
    Control(DeviceDefinition, Sender<ControlMessage>),
}

pub enum AudioMessage {
    Handle(Message, oneshot::Sender<Result<Message>>),
}

pub enum ControlMessage {
    SendImage(Vec<u8>, u32, u32, Sender<Result<()>>),
    DisplayBrightness(u8, Sender<Result<()>>),
    ButtonBrightness(u8, Sender<Result<()>>),
    DimTimeout(Duration, Sender<Result<()>>),
    ButtonColour(ButtonLighting, RGBA, Sender<Result<()>>),
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct DeviceDefinition {
    pub location: DeviceLocation,
    pub device_type: DeviceType,
    pub device_info: DeviceInfo,
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: String,
    pub version: VersionNumber,
}
