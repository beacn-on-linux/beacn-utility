use crate::device_manager::ControlMessage;
use crate::device_manager::ControlMessage::SendImage;
use crate::integrations::pipeweaver::channel::{
    BeacnImage, ChannelChangedProperty, ChannelRenderer, UpdateFrom,
};
use crate::integrations::pipeweaver::layout::{
    BG_COLOUR, CHANNEL_DIMENSIONS, CHANNEL_INNER_COLOUR, DISPLAY_DIMENSIONS, DrawingUtils,
    JPEG_QUALITY, POSITION_ROOT,
};
use crate::runtime;
use anyhow::{Context, Result, anyhow, bail};
use beacn_lib::controller::{ButtonLighting, ButtonState, Buttons, Dials, Interactions};
use beacn_lib::crossbeam::channel::{Receiver, RecvError, Sender};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::RGBA;
use enum_map::EnumMap;
use futures_util::{SinkExt, StreamExt};
use image::{ImageBuffer, Rgba, RgbaImage};
use log::{debug, info, warn};
use pipeweaver_ipc::commands::APICommand::SetSourceVolume;
use pipeweaver_ipc::commands::DaemonRequest::GetStatus;
use pipeweaver_ipc::commands::{
    DaemonRequest, DaemonResponse, DaemonStatus, WebsocketRequest, WebsocketResponse,
};
use pipeweaver_profile::{PhysicalSourceDevice, SourceDevices, VirtualSourceDevice};
use pipeweaver_shared::{Colour, Mix, OrderGroup};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::mpsc::channel;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use ulid::Ulid;

mod channel;
mod layout;

// This is so we can more cleanly Map Physical / Virtual devices, because the data we need from
// them is the same regardless, and ChannelRenderer has From<> for Both
#[derive(Debug)]
enum DeviceRef<'a> {
    Physical(&'a PhysicalSourceDevice),
    Virtual(&'a VirtualSourceDevice),
}

pub async fn spawn_pipeweaver_handler(
    sender: Sender<ControlMessage>,
    device: DeviceType,
    input_rx: Receiver<Interactions>,
) {
    info!("Starting Pipeweaver Manager");
    let url = "ws://localhost:14565/api/websocket";

    // We need to handle this in a loop, if something goes bad just make sure we're disconnencted
    // and try again after 5 seconds,
    loop {
        info!("Attempting Connection to Pipeweaver");

        // Attempt a connection to Pipeweaver
        let connection = connect_async(url).await;
        if let Err(e) = connection {
            warn!("Error Connecting to Pipeweaver: {}", e);
            sleep(Duration::from_secs(5)).await;
            continue;
        }
        let (mut stream, _) = connection.unwrap();
        info!("Successfully connected to Pipeweaver");

        let result =
            run_pipeweaver_socket(&mut stream, sender.clone(), device, input_rx.clone()).await;
        match result {
            Ok(_) => break,
            Err(e) => {
                warn!("Pipeweaver Error, closing socket: {}", e);
                let _ = stream.close(None).await;
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        }
    }
}

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Renderers = HashMap<Ulid, ChannelRenderer>;
async fn run_pipeweaver_socket(
    stream: &mut WebSocket,
    sender: Sender<ControlMessage>,
    device_type: DeviceType,
    input_rx: Receiver<Interactions>,
) -> Result<()> {
    // There are some types (for example, HashMaps) which don't guarantee ordering, so going from
    // a DaemonStatus -> JSON before applying a PATCH could result in the wrong data being altered.
    // So we'll maintain a raw state to apply patches to, then update the Status after changes
    let mut raw_status: Value = Value::Null;

    let mut status = DaemonStatus::default();
    let mut command_index = 0;

    // Perform the Initial Status Fetch
    let status_request = serde_json::to_string(&WebsocketRequest {
        id: command_index,
        data: GetStatus,
    })?;

    if let Err(e) = stream
        .send(Message::Text(Utf8Bytes::from(status_request)))
        .await
    {
        bail!("Failed to fetch Status: {}", e)
    }

    command_index += 1;

    // There are occasionally patch messages which could occur before the status response,
    // so we'll loop here until we get the answer we're looking for
    loop {
        let message = stream.next().await;
        if let Some(message) = message {
            let message = message?;
            if let Message::Text(msg) = message {
                let value = serde_json::from_str::<Value>(msg.as_str())?;

                // This should be a WebSocketResponse object
                let object = value.as_object().ok_or(anyhow!("Failed to Read Object"))?;

                // Check the ID (should always be present)
                let id = object.get("id").ok_or(anyhow!("Failed to Read ID"))?;

                // We can occasionally get patches before the Status response, so verify the ID...
                if id.as_u64().ok_or(anyhow!("Unable to Parse id"))? == 0u64 {
                    // This is our DaemonStatus response
                    let data = object
                        .get("data")
                        .ok_or(anyhow!("Failed to Read Data"))?
                        .clone();
                    raw_status = data
                        .get("Status")
                        .ok_or(anyhow!("Failed to Read Status"))?
                        .clone();
                    status = serde_json::from_value::<DaemonStatus>(raw_status.clone())?;
                    break;
                }
            }
        }
    }

    // Next, we need to find the channels, and create a renderer
    let mut renderers: Renderers = HashMap::new();

    let mut page = 0;
    let sources = &status.audio.profile.devices.sources;
    let mut devices_shown = get_page_channels(&sources.device_order, page);

    for device in &devices_shown {
        let render = get_channel_renderers(device, sources);
        renderers.insert(*device, render?);
    }

    let mut active_mix = Mix::A;

    // Send out a full device refresh
    let (tx, rx) = oneshot::channel();
    for (index, device) in devices_shown.iter().enumerate() {
        let render = renderers
            .get(device)
            .ok_or_else(|| anyhow!("Failed to get render"))?;
        let dial_button = match index {
            0 => ButtonLighting::Dial1,
            1 => ButtonLighting::Dial2,
            2 => ButtonLighting::Dial3,
            3 => ButtonLighting::Dial4,
            _ => bail!("Invalid Dial Index"),
        };

        let colour = render.colour;
        let beacn_colour = RGBA {
            red: colour[0],
            green: colour[1],
            blue: colour[2],
            alpha: colour[3],
        };

        let (tx, rx) = oneshot::channel();
        sender.send(ControlMessage::ButtonColour(dial_button, beacn_colour, tx))?;
        rx.recv()??;
    }
    let img = do_full_render(active_mix, &devices_shown, &renderers)?;
    let (x, y) = img.position;
    sender.send(SendImage(img_as_jpeg(img.image, BG_COLOUR)?, x, y, tx))?;
    rx.recv()??;

    // We create a pending volume send, and store the 'final' value, this allows us to better
    // manage intermediate updates.
    let (interaction_tx, mut interaction_rx) = channel(10);

    // Ok, we need to wrap the input handler into something async
    runtime().spawn_blocking(move || sync_to_async(input_rx, interaction_tx));

    // Ok, we can move into the websocket loop
    loop {
        select! {
            Some(message) = stream.next() => {
                let message = message?;
                if message.is_text() {
                    let result = serde_json::from_str::<WebsocketResponse>(message.to_text()?)?;
                    if let DaemonResponse::Patch(patch) = result.data {
                        // Update the raw status for the change
                        json_patch::patch(&mut raw_status, &patch)?;
                        status = serde_json::from_value::<DaemonStatus>(raw_status.clone())?;

                        // Check whether the channel list has changed
                        let sources = &status.audio.profile.devices.sources;
                        let devices = get_page_channels(&sources.device_order, page);

                        if devices != devices_shown {
                            devices_shown = devices.clone();
                            for device in devices {
                                if !renderers.contains_key(&device) {
                                    let render = get_channel_renderers(&device, sources)?;
                                    renderers.insert(device, render);
                                }
                            }
                            // Remove configs which aren't shown anymore
                            renderers.retain(|id, _| devices_shown.contains(id));

                            // Set the Button Colours
                            for (index, device) in devices_shown.iter().enumerate() {
                                let render = renderers.get(device).ok_or_else(|| anyhow!("Failed to get render"))?;
                                let dial_button = match index {
                                    0 => ButtonLighting::Dial1,
                                    1 => ButtonLighting::Dial2,
                                    2 => ButtonLighting::Dial3,
                                    3 => ButtonLighting::Dial4,
                                    _ => bail!("Invalid Dial Index"),
                                };

                                let colour = render.colour;
                                let beacn_colour = RGBA {
                                    red: colour[0],
                                    green: colour[1],
                                    blue: colour[2],
                                    alpha: colour[3],
                                };

                                let (tx,rx) = oneshot::channel();
                                sender.send(ControlMessage::ButtonColour(dial_button, beacn_colour, tx))?;
                                rx.recv()??;
                            }

                            // Perform a full redraw of the display
                            let (tx, rx) = oneshot::channel();
                            let img = do_full_render(active_mix, &devices_shown, &renderers)?;
                            let (x, y) = img.position;
                            sender.send(SendImage(img_as_jpeg(img.image, BG_COLOUR)?, x, y, tx))?;
                            rx.recv()??;
                        } else {
                            // Check whether any existing devices have changed
                            for (index, device) in devices_shown.iter().enumerate() {
                                let dev_ref = get_device_ref(device, sources)?;
                                let render = renderers.get_mut(&device).ok_or_else(|| anyhow!("Failed to get renderer"))?;

                                let update = match dev_ref {
                                    DeviceRef::Physical(d) => render.update_from(d.clone()),
                                    DeviceRef::Virtual(d) => render.update_from(d.clone()),
                                };

                                for part in update {
                                    let (img, x, y) = match part {
                                        ChannelChangedProperty::Title => {
                                            let img = render.draw_header();

                                            let (x, y) = img.position;
                                            let img = img_as_jpeg(img.image, BG_COLOUR)?;

                                            (img, x, y)
                                        }
                                        ChannelChangedProperty::Colour => {
                                            // Firstly, set the button colour
                                            let dial_button = match index {
                                                0 => ButtonLighting::Dial1,
                                                1 => ButtonLighting::Dial2,
                                                2 => ButtonLighting::Dial3,
                                                3 => ButtonLighting::Dial4,
                                                _ => bail!("Invalid Dial Index"),
                                            };

                                            let colour = render.colour;
                                            let beacn_colour = RGBA {
                                                red: colour[0],
                                                green: colour[1],
                                                blue: colour[2],
                                                alpha: colour[3],
                                            };

                                            let (tx,rx) = oneshot::channel();
                                            sender.send(ControlMessage::ButtonColour(dial_button, beacn_colour, tx))?;
                                            rx.recv()??;

                                            // We need to redraw the entire channel
                                            let img = render.full_render(active_mix);

                                            let (x, y) = img.position;
                                            let img = img_as_jpeg(img.image, BG_COLOUR)?;

                                            (img, x, y)
                                        }
                                        ChannelChangedProperty::Volumes(mix) => {
                                            if mix != active_mix {
                                                continue
                                            }

                                            let img = render.get_volume(active_mix)?;
                                            let (x, y) = img.position;

                                            (img.image, x, y)
                                        }
                                        ChannelChangedProperty::MuteState(target) => {
                                            let img = render.draw_mute_box(target);

                                            let (x, y) = img.position;
                                            let img = img_as_jpeg(img.image, BG_COLOUR)?;

                                            (img, x, y)
                                        }
                                    };

                                    let (ch_w, _) = CHANNEL_DIMENSIONS;
                                    let base_x = ch_w * index as u32;

                                    let (root_x, root_y) = POSITION_ROOT;
                                    let x = base_x + x + root_x;
                                    let y = y + root_y;

                                    let (tx,rx) = oneshot::channel();
                                    sender.send(SendImage(img, x, y, tx))?;
                                    rx.recv()??;
                                };
                            }
                        }
                    }
                } else {
                    bail!("Received non-text message from Websocket!")
                }
            }
            Some(msg) = interaction_rx.recv() => {
                match device_type {
                    DeviceType::BeacnMix => {
                        match msg {
                            Interactions::ButtonPress(_,_) => {}
                            Interactions::DialChanged(dial, change) => {
                                let device_index = match dial {
                                    Dials::Dial1 => 0,
                                    Dials::Dial2 => 1,
                                    Dials::Dial3 => 2,
                                    Dials::Dial4 => 3,
                                };
                                if let Some(device) = devices_shown.get(device_index) {
                                    let current = renderers.get_mut(device).ok_or_else(|| anyhow!("Failed to get renderer"))?;
                                    let volume = current.volumes[active_mix];

                                    let new_volume = (volume as i8 + change).clamp(0, 100);

                                    let command = serde_json::to_string(&WebsocketRequest {
                                        id: command_index,
                                        data: DaemonRequest::Pipewire(SetSourceVolume(*device, active_mix, new_volume as u8)),
                                    })?;
                                    command_index += 1;
                                    stream.send(Message::Text(Utf8Bytes::from(command))).await?;
                                }
                            }
                        }

                    }
                    DeviceType::BeacnMixCreate => {
                        match msg {
                            Interactions::ButtonPress(button, state) => {
                                if state == ButtonState::Release {
                                    match button {
                                        Buttons::AudienceMix => {

                                        }
                                        Buttons::PageLeft => {

                                        }
                                        Buttons::PageRight => {

                                        }
                                        Buttons::Dial1 => {

                                        }
                                        Buttons::Dial2 => {

                                        }
                                        Buttons::Dial3 => {

                                        }
                                        Buttons::Dial4 => {

                                        }
                                        Buttons::Audience1 => {

                                        }
                                        Buttons::Audience2 => {

                                        }
                                        Buttons::Audience3 => {

                                        }
                                        Buttons::Audience4 => {

                                        }
                                    }
                                }
                            }
                            Interactions::DialChanged(dial, change) => {
                                let device_index = match dial {
                                    Dials::Dial1 => 0,
                                    Dials::Dial2 => 1,
                                    Dials::Dial3 => 2,
                                    Dials::Dial4 => 3,
                                };
                                if let Some(device) = devices_shown.get(device_index) {
                                    let current = renderers.get_mut(device).ok_or_else(|| anyhow!("Failed to get renderer"))?;
                                    let volume = current.volumes[active_mix];

                                    let new_volume = (volume as i8 + change).clamp(0, 100);

                                    let command = serde_json::to_string(&WebsocketRequest {
                                        id: command_index,
                                        data: DaemonRequest::Pipewire(SetSourceVolume(*device, active_mix, new_volume as u8)),
                                    })?;
                                    command_index += 1;

                                    stream.send(Message::Text(Utf8Bytes::from(command))).await?;
                                }
                            }
                        }
                    }
                    _ => bail!("Received interaction from invalid device!"),
                }
                debug!("PW: Received: {}", msg);
            }
        }
    }

    Ok(())
}

fn img_as_jpeg(image: RgbaImage, background: Rgba<u8>) -> Result<Vec<u8>> {
    DrawingUtils::image_as_jpeg(image, background, JPEG_QUALITY)
}

fn do_full_render(mix: Mix, list: &Vec<Ulid>, map: &Renderers) -> Result<BeacnImage> {
    let (width, height) = DISPLAY_DIMENSIONS;
    let mut base = ImageBuffer::from_pixel(width, height, BG_COLOUR);

    for (index, item) in list.iter().enumerate() {
        let renderer = map
            .get(item)
            .ok_or_else(|| anyhow!("No such render object"))?;
        let drawing = renderer.full_render(mix);
        let (_, y) = drawing.position;
        let (width, _) = CHANNEL_DIMENSIONS;
        let x = width * index as u32;
        DrawingUtils::composite_from_pos(&mut base, &drawing.image, (x, y));
    }

    Ok(BeacnImage {
        position: (0, 0),
        image: base,
    })
}

fn get_device_ref<'a>(device: &Ulid, sources: &'a SourceDevices) -> Result<DeviceRef<'a>> {
    sources
        .physical_devices
        .iter()
        .map(DeviceRef::Physical)
        .chain(sources.virtual_devices.iter().map(DeviceRef::Virtual))
        .find(|dev| match dev {
            DeviceRef::Physical(d) => d.description.id == *device,
            DeviceRef::Virtual(d) => d.description.id == *device,
        })
        .with_context(|| format!("Attempted to Display Non-existing Device: {}", device))
}

fn get_channel_renderers(device: &Ulid, sources: &SourceDevices) -> Result<ChannelRenderer> {
    let dev = get_device_ref(device, sources)?;

    let renderer = match dev {
        DeviceRef::Physical(d) => ChannelRenderer::from(d.clone()),
        DeviceRef::Virtual(d) => ChannelRenderer::from(d.clone()),
    };
    Ok(renderer)
}

fn get_page_channels(order: &EnumMap<OrderGroup, Vec<Ulid>>, page: u8) -> Vec<Ulid> {
    let mut channels = Vec::with_capacity(4);

    // This is a little complicated, we need to check the pinned channels and add them first
    let pinned = &order[OrderGroup::Pinned];
    let others = &order[OrderGroup::Default];

    if pinned.is_empty() && others.is_empty() {
        warn!("No channels are defined!");
        return channels;
    }

    // The pinned options should appear on all the pages
    for channel in pinned.iter().take(channels.capacity() - channels.len()) {
        channels.push(*channel);
    }

    // If the user has 4 pinned channels, we really can't do paging
    if channels.len() == channels.capacity() {
        return channels;
    }

    // Ok, now we need to work out how many non-pinned channels per page we can have
    let channels_per_page = 4 - pinned.len() as u8;

    if others.len() < channels_per_page as usize {
        for other in others {
            channels.push(*other);
        }
        return channels;
    }

    let start = if ((channels_per_page * page) + channels_per_page) as usize > others.len() {
        // Clamp to the Last item in the list if this overflows
        others.len().saturating_sub(channels_per_page as usize)
    } else {
        (channels_per_page * page) as usize
    };

    for channel in others.iter().skip(start) {
        if channels.len() != channels.capacity() {
            channels.push(*channel);
        }
    }

    channels
}

fn sync_to_async(
    rx: Receiver<Interactions>,
    tx: tokio::sync::mpsc::Sender<Interactions>,
) -> Result<()> {
    debug!("Running Up Receiver..");
    loop {
        match rx.recv() {
            Ok(val) => {
                tx.blocking_send(val)?;
            }
            Err(_) => {
                debug!("Error Occurred, stopping sync wrapper");
                break;
            }
        }
    }
    Ok(())
}
