use crate::device_manager::ControlMessage;
use crate::device_manager::ControlMessage::{ButtonColour, SendImage};
use crate::integrations::pipeweaver::channel::{
    ChannelChangedProperty, ChannelRenderer, UpdateFrom,
};
use crate::integrations::pipeweaver::layout::{
    BG_COLOUR, CHANNEL_DIMENSIONS, DISPLAY_DIMENSIONS, DrawingUtils, FONT_BOLD, HEADER,
    JPEG_QUALITY, POSITION_ROOT, TEXT_COLOUR, TextAlign,
};
use crate::runtime;
use anyhow::{Context, Result, anyhow, bail};
use beacn_lib::controller::{ButtonLighting, ButtonState, Buttons, Dials, Interactions};
use beacn_lib::crossbeam::channel::{Receiver, Sender, TryRecvError};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::RGBA;
use futures_util::{SinkExt, StreamExt};
use image::{ImageBuffer, Rgba, RgbaImage, load_from_memory};
use log::{debug, info, warn};
use pipeweaver_ipc::commands::APICommand::SetSourceVolume;
use pipeweaver_ipc::commands::DaemonRequest::GetStatus;
use pipeweaver_ipc::commands::{
    APICommand, DaemonRequest, DaemonResponse, DaemonStatus, WebsocketRequest, WebsocketResponse,
};
use pipeweaver_profile::{PhysicalSourceDevice, SourceDevices, VirtualSourceDevice};
use pipeweaver_shared::{Mix, MuteTarget, OrderGroup};
use serde_json::Value;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::time::Duration;
use strum::IntoEnumIterator;
use tokio::net::TcpStream;
use tokio::sync::mpsc::channel;
use tokio::time::sleep;
use tokio::{select, time};
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite};
use ulid::Ulid;

const PW_SPLASH: &[u8] = include_bytes!("../../../resources/screens/beacn-pipeweaver.jpg");

mod channel;
mod layout;

const COLOUR_MIX_A: RGBA = RGBA {
    red: 89,
    green: 177,
    blue: 182,
    alpha: 255,
};
const COLOUR_MIX_B: RGBA = RGBA {
    red: 244,
    green: 124,
    blue: 36,
    alpha: 255,
};

const COLOUR_WHITE: RGBA = RGBA {
    red: 255,
    green: 255,
    blue: 255,
    alpha: 255,
};

const COLOUR_BLACK: RGBA = RGBA {
    red: 0,
    green: 0,
    blue: 0,
    alpha: 0,
};

// This is so we can more cleanly Map Physical / Virtual devices, because the data we need from
// them is the same regardless, and ChannelRenderer has From<> for Both
#[derive(Debug)]
enum DeviceRef<'a> {
    Physical(&'a PhysicalSourceDevice),
    Virtual(&'a VirtualSourceDevice),
}

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Renderers = HashMap<Ulid, ChannelRenderer>;

struct PipeweaverHandler {
    device_type: DeviceType,
    sender: Sender<ControlMessage>,
    input_rx: Receiver<Interactions>,

    has_connected: bool,
    displaying_error: bool,

    command_index: u64,
    raw_status: Value,
    status: DaemonStatus,

    active_page: u8,
    active_mix: Mix,
    devices_shown: Vec<Ulid>,
    renderers: Renderers,
}

impl PipeweaverHandler {
    pub fn new(
        device_type: DeviceType,
        sender: Sender<ControlMessage>,
        input_rx: Receiver<Interactions>,
    ) -> Self {
        Self {
            device_type,
            sender,
            input_rx,

            has_connected: false,
            displaying_error: false,

            command_index: 0,
            raw_status: Value::Null,
            status: DaemonStatus::default(),

            active_page: 0,
            active_mix: Mix::A,
            devices_shown: Vec::with_capacity(4),
            renderers: HashMap::new(),
        }
    }

    pub async fn run_handler(&mut self) {
        info!("Starting Pipeweaver Manager");
        let url = "ws://localhost:14565/api/websocket";

        // Send the Pipeweaver Splash
        self.draw_splash();
        self.draw_status("Loading...");
        self.disable_buttons();

        // We need to handle this in a loop, if something goes bad just make sure we're disconnencted
        // and try again after 5 seconds,
        while let Err(e) = self.handle_connection(url).await {
            // It doesn't matter if we lose an input here, we're not handling them anyway.
            if matches!(self.input_rx.try_recv(), Err(TryRecvError::Disconnected)) {
                warn!("Interaction Handler Terminated, Bailing!");
                break;
            }

            if !self.displaying_error {
                if !self.has_connected {
                    self.draw_status("Failed to connect to Pipeweaver");
                    self.disable_buttons();
                } else {
                    self.draw_splash();
                    self.draw_status("Connection to Pipeweaver lost");
                    self.disable_buttons();
                }
            }
            self.displaying_error = true;

            if let Some(tungstenite::Error::Io(e)) = e.downcast_ref()
                && e.kind() == ErrorKind::ConnectionRefused
            {
                // No point dumping a warning here, Pipeweaver isn't running.
                sleep(Duration::from_secs(5)).await;
                continue;
            }

            warn!("Pipeweaver Error: {}", e);
            sleep(Duration::from_secs(5)).await;
            continue;
        }
    }

    fn draw_splash(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.sender.send(SendImage(Vec::from(PW_SPLASH), 0, 0, tx));
        let _ = rx.recv();
    }

    fn draw_status(&self, text: &str) {
        let text = DrawingUtils::draw_text(
            text.into(),
            800,
            30,
            FONT_BOLD,
            28.,
            TEXT_COLOUR,
            TextAlign::Center,
        );

        if let Ok(img) = img_as_jpeg(text, Rgba([0, 0, 0, 255])) {
            let (tx, rx) = oneshot::channel();
            let _ = self.sender.send(SendImage(img, 0, 330, tx));
            let _ = rx.recv();
        }
    }

    fn disable_buttons(&self) {
        for button in ButtonLighting::iter() {
            let (tx, rx) = oneshot::channel();
            let _ = self.sender.send(ButtonColour(button, COLOUR_BLACK, tx));
            let _ = rx.recv();
        }
    }

    async fn handle_connection(&mut self, url: &str) -> Result<()> {
        let (mut stream, _) = connect_async(url).await?;
        info!("Successfully connected to Pipeweaver");

        self.has_connected = true;
        self.displaying_error = false;

        self.load_status(&mut stream).await?;
        self.load_initial_state().await?;
        self.run_message_loop(&mut stream).await?;

        Ok(())
    }

    async fn load_status(&mut self, stream: &mut WebSocket) -> Result<()> {
        // Perform the Initial Status Fetch
        let status_id = self.get_command_index();

        let status_request = serde_json::to_string(&WebsocketRequest {
            id: status_id,
            data: GetStatus,
        })?;

        let message = Message::Text(Utf8Bytes::from(status_request));
        if let Err(e) = stream.send(message).await {
            bail!("Failed to fetch Status: {}", e)
        }

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
                    if id.as_u64().ok_or(anyhow!("Unable to Parse id"))? == status_id {
                        // This is our DaemonStatus response
                        let error = anyhow!("Failed to Read Data");
                        let data = object.get("data").ok_or(error)?.clone();

                        let error = anyhow!("Failed to Read Status");
                        self.raw_status = data.get("Status").ok_or(error)?.clone();

                        let raw = self.raw_status.clone();
                        self.status = serde_json::from_value::<DaemonStatus>(raw)?;
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    async fn load_initial_state(&mut self) -> Result<()> {
        let devices_shown = self.get_channels_on_page();
        self.devices_shown = devices_shown;

        // Update the Rendering Nodes
        self.update_renderers()?;

        // Perform the initial screen render
        self.perform_full_refresh()?;

        Ok(())
    }

    async fn run_message_loop(&mut self, stream: &mut WebSocket) -> Result<()> {
        debug!("Spawning Sync <-> Async Loop");

        let sync_receiver = self.input_rx.clone();
        let (interaction_tx, mut interaction_rx) = channel(10);
        runtime().spawn_blocking(move || sync_to_async(sync_receiver, interaction_tx));

        let mut keep_alive = time::interval(Duration::from_secs(10));

        let (tx, rx) = oneshot::channel();
        self.sender.send(ControlMessage::Enabled(true, tx))?;
        rx.recv()??;

        debug!("Starting Pipeweaver Message Loop");
        loop {
            select! {
                Some(message) = stream.next() => {
                    let message = message?;
                    if message.is_text() {
                        let result = serde_json::from_str::<WebsocketResponse>(message.to_text()?)?;
                        if let DaemonResponse::Patch(patch) = result.data {
                            // Update the raw status for the change
                            json_patch::patch(&mut self.raw_status, &patch)?;
                            self.status = serde_json::from_value::<DaemonStatus>(self.raw_status.clone())?;

                            // Check whether the channel list has changed
                            let sources = &self.status.audio.profile.devices.sources;
                            let devices = self.get_channels_on_page();

                            if devices != self.devices_shown {
                                self.devices_shown = devices.clone();

                                self.update_renderers()?;

                                // Set the Button Colours
                                self.load_all_dial_button_colours()?;
                                self.perform_full_redraw()?;
                            } else {
                                // Check whether any existing devices have changed
                                for (index, device) in self.devices_shown.iter().enumerate() {
                                    let mut refresh_button_colour = false;

                                    let dev_ref = self.get_device_ref(device, sources)?;
                                    let render = self.renderers.get_mut(device).ok_or_else(|| anyhow!("Failed to get renderer"))?;

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
                                                // Set the Button Colour to Refresh
                                                refresh_button_colour = true;

                                                // We need to redraw the entire channel
                                                let img = render.full_render(self.active_mix);

                                                let (x, y) = img.position;
                                                let img = img_as_jpeg(img.image, BG_COLOUR)?;

                                                (img, x, y)
                                            }
                                            ChannelChangedProperty::Volumes(mix) => {
                                                if mix != self.active_mix {
                                                    continue
                                                }

                                                let img = render.get_volume(self.active_mix)?;
                                                let (x, y) = img.position;

                                                (img.image, x, y)
                                            }
                                            ChannelChangedProperty::MuteState(target) => {
                                                // Don't draw MixB Mute updates on the Beacn Mix
                                                if target == MuteTarget::TargetB && self.device_type == DeviceType::BeacnMix {
                                                    continue;
                                                }

                                                let img = render.draw_mute_box(target);

                                                let (x, y) = img.position;
                                                let img = img_as_jpeg(img.image, BG_COLOUR)?;

                                                (img, x, y)
                                            }
                                        };

                                        // Determine the 'start' position of this channel
                                        let (ch_w, _) = CHANNEL_DIMENSIONS;
                                        let base_x = ch_w * index as u32;

                                        // Get the position relative to the main image root
                                        let (root_x, root_y) = POSITION_ROOT;
                                        let x = base_x + x + root_x;
                                        let y = y + root_y;

                                        // Send it
                                        let (tx,rx) = oneshot::channel();
                                        self.sender.send(SendImage(img, x, y, tx))?;
                                        rx.recv()??;
                                    };

                                    // We split this out because there's a lot of borrowing going on
                                    // inside the loops regards the renderer, which makes executing
                                    // earlier more difficult :D
                                    if refresh_button_colour {
                                        self.load_dial_button_colour(index)?;
                                    }
                                }
                            }
                        }
                    } else {
                        bail!("Received non-text message from Websocket!")
                    }
                }
                maybe_msg = interaction_rx.recv() => {
                    match maybe_msg {
                        Some(msg) => {
                            match self.device_type {
                                DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                                    match msg {
                                        Interactions::ButtonPress(button, state) => {
                                            if state == ButtonState::Release {
                                                self.handle_button(button, stream).await?;
                                            }
                                        }
                                        Interactions::DialChanged(dial, change) => {
                                            self.handle_dial(dial, change, stream).await?;
                                        }
                                    }
                                }
                                t => bail!("WTF is this doing here?! {:?}", t)
                            }
                        },
                        None => bail!("Receive Handler Closed!")
                    }
                }
                _instant = keep_alive.tick() => {
                    let (tx,rx) = oneshot::channel();
                    self.sender.send(ControlMessage::KeepAlive(tx))?;
                    rx.recv()??;
                }
            }
        }
    }

    fn perform_full_refresh(&self) -> Result<()> {
        self.perform_full_redraw()?;
        self.load_all_dial_button_colours()?;
        self.load_page_button_colours()?;
        self.load_mix_button_colours()?;

        Ok(())
    }

    fn update_renderers(&mut self) -> Result<()> {
        for device in &self.devices_shown {
            if !self.renderers.contains_key(device) {
                let render = self.get_channel_renderer(device)?;
                self.renderers.insert(*device, render);
            }
        }
        // Remove configs which aren't shown anymore
        self.renderers
            .retain(|id, _| self.devices_shown.contains(id));
        Ok(())
    }

    fn perform_full_redraw(&self) -> Result<()> {
        let (width, height) = DISPLAY_DIMENSIONS;
        let mut base = ImageBuffer::from_pixel(width, height, BG_COLOUR);

        DrawingUtils::composite_from_pos(&mut base, &jpeg_as_img(HEADER)?, (0, 0));

        for (index, item) in self.devices_shown.iter().enumerate() {
            let error = anyhow!("No Such Render Object");
            let renderer = self.renderers.get(item).ok_or(error)?;
            let drawing = renderer.full_render(self.active_mix);
            let (width, _) = CHANNEL_DIMENSIONS;
            let x = width * index as u32;
            let y = POSITION_ROOT.1;
            DrawingUtils::composite_from_pos(&mut base, &drawing.image, (x, y));
        }

        let (tx, rx) = oneshot::channel();
        let img = img_as_jpeg(base, BG_COLOUR)?;
        self.sender.send(SendImage(img, 0, 0, tx))?;
        rx.recv()??;

        Ok(())
    }

    fn redraw_volumes(&self) -> Result<()> {
        for (index, item) in self.devices_shown.iter().enumerate() {
            let error = anyhow!("No Such Render Object");
            let renderer = self.renderers.get(item).ok_or(error)?;
            let drawing = renderer.get_volume(self.active_mix)?;
            let (x, y) = drawing.position;

            // Determine the 'start' position of this channel
            let (ch_w, _) = CHANNEL_DIMENSIONS;
            let base_x = ch_w * index as u32;

            // Get the position relative to the main image root
            let (root_x, root_y) = POSITION_ROOT;
            let x = base_x + x + root_x;
            let y = y + root_y;

            // Send it
            let (tx, rx) = oneshot::channel();
            self.sender.send(SendImage(drawing.image, x, y, tx))?;
            rx.recv()??;
        }

        Ok(())
    }

    fn load_all_dial_button_colours(&self) -> Result<()> {
        for index in 0..self.devices_shown.len() {
            self.load_dial_button_colour(index)?;
        }
        Ok(())
    }

    fn load_page_button_colours(&self) -> Result<()> {
        let left_colour = match self.active_page == 0 {
            true => COLOUR_BLACK,
            false => COLOUR_WHITE,
        };

        // Map the Previous / Next Button colours
        let right_colour = match self.get_page_count() {
            1 => COLOUR_BLACK,
            c => match self.active_page == c - 1 {
                true => COLOUR_BLACK,
                false => COLOUR_WHITE,
            },
        };

        // Send the page colours
        self.set_button_colour(ButtonLighting::Left, left_colour)?;
        self.set_button_colour(ButtonLighting::Right, right_colour)?;

        Ok(())
    }

    fn load_mix_button_colours(&self) -> Result<()> {
        let colour = match self.active_mix {
            Mix::A => COLOUR_MIX_B,
            Mix::B => COLOUR_MIX_A,
        };
        self.set_button_colour(ButtonLighting::Mix, colour)?;
        Ok(())
    }

    fn load_dial_button_colour(&self, index: usize) -> Result<()> {
        let error = anyhow!("No Such Index");
        let device_id = self.devices_shown.get(index).ok_or(error)?;

        let error = anyhow!("Failed to Fetch Renderer");
        let render = self.renderers.get(device_id).ok_or(error)?;

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

        self.set_button_colour(dial_button, beacn_colour)?;
        Ok(())
    }

    fn get_command_index(&mut self) -> u64 {
        let result = self.command_index;
        self.command_index += 1;
        result
    }

    fn get_channel_renderer(&self, device: &Ulid) -> Result<ChannelRenderer> {
        let sources = &self.status.audio.profile.devices.sources;
        let dev = self.get_device_ref(device, sources)?;

        let mut renderer = match dev {
            DeviceRef::Physical(d) => ChannelRenderer::from(d.clone()),
            DeviceRef::Virtual(d) => ChannelRenderer::from(d.clone()),
        };
        renderer.set_beacn_device(self.device_type);
        Ok(renderer)
    }

    fn refresh_page(&mut self) -> Result<()> {
        self.devices_shown = self.get_channels_on_page();
        self.update_renderers()?;
        self.perform_full_refresh()?;
        Ok(())
    }

    fn get_page_count(&self) -> u8 {
        let order = &self.status.audio.profile.devices.sources.device_order;

        // If we can't display any other channels because we're populated with pins, send 1 page.
        if order[OrderGroup::Pinned].len() >= 4 || order[OrderGroup::Default].is_empty() {
            return 1;
        }

        let channels_per_page = 4 - order[OrderGroup::Pinned].len() as u8;
        let channel_count = order[OrderGroup::Default].len() as u8;
        (channels_per_page + channel_count - 1) / channels_per_page
    }

    fn get_channels_on_page(&self) -> Vec<Ulid> {
        let order = &self.status.audio.profile.devices.sources.device_order;
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

        let channel_start = (channels_per_page * self.active_page) + channels_per_page;
        let start = if channel_start as usize > others.len() {
            // Clamp to the Last item in the list if this overflows
            others.len().saturating_sub(channels_per_page as usize)
        } else {
            (channels_per_page * self.active_page) as usize
        };

        for channel in others.iter().skip(start) {
            if channels.len() != channels.capacity() {
                channels.push(*channel);
            }
        }

        channels
    }
    fn get_device_ref<'a>(
        &self,
        device: &Ulid,
        sources: &'a SourceDevices,
    ) -> Result<DeviceRef<'a>> {
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

    fn set_button_colour(&self, button: ButtonLighting, colour: RGBA) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let message = ButtonColour(button, colour, tx);
        self.sender.send(message)?;
        rx.recv()??;
        Ok(())
    }

    // Handle Button Presses
    async fn handle_button(&mut self, button: Buttons, stream: &mut WebSocket) -> Result<()> {
        match button {
            Buttons::AudienceMix => {
                // This one is now stupidly simple
                self.active_mix = match self.active_mix {
                    Mix::A => Mix::B,
                    Mix::B => Mix::A,
                };
                self.redraw_volumes()?;
                self.load_mix_button_colours()?;
            }
            Buttons::PageLeft | Buttons::PageRight => {
                let change: i8 = match button {
                    Buttons::PageLeft => -1,
                    Buttons::PageRight => 1,
                    _ => bail!("Invalid button"),
                };

                if self.active_page == 0 && change == -1 {
                    return Ok(());
                }
                if self.active_page == self.get_page_count() - 1 && change == 1 {
                    return Ok(());
                }

                self.active_page = self.active_page.wrapping_add_signed(change);
                self.refresh_page()?;
            }

            // The general behaviour for all the main buttons is the same, just with minor tweaks
            // depending on which was pressed
            Buttons::Dial1
            | Buttons::Dial2
            | Buttons::Dial3
            | Buttons::Dial4
            | Buttons::Audience1
            | Buttons::Audience2
            | Buttons::Audience3
            | Buttons::Audience4 => {
                // Get our refined information from the button
                let (index, target) = match button {
                    Buttons::Dial1 => (0, MuteTarget::TargetA),
                    Buttons::Dial2 => (1, MuteTarget::TargetA),
                    Buttons::Dial3 => (2, MuteTarget::TargetA),
                    Buttons::Dial4 => (3, MuteTarget::TargetA),
                    Buttons::Audience1 => (0, MuteTarget::TargetB),
                    Buttons::Audience2 => (1, MuteTarget::TargetB),
                    Buttons::Audience3 => (2, MuteTarget::TargetB),
                    Buttons::Audience4 => (3, MuteTarget::TargetB),
                    _ => bail!("This shouldn't happen."),
                };

                if let Some(device) = self.devices_shown.get(index) {
                    let error = anyhow!("Failed to get Renderer");
                    let current = self.renderers.get_mut(device).ok_or(error)?;
                    let message = if current.mute_states[target].is_active {
                        APICommand::DelSourceMuteTarget(*device, target)
                    } else {
                        APICommand::AddSourceMuteTarget(*device, target)
                    };
                    let command = serde_json::to_string(&WebsocketRequest {
                        id: self.get_command_index(),
                        data: DaemonRequest::Pipewire(message),
                    })?;
                    stream.send(Message::Text(Utf8Bytes::from(command))).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_dial(&mut self, dial: Dials, change: i8, stream: &mut WebSocket) -> Result<()> {
        let device_index = match dial {
            Dials::Dial1 => 0,
            Dials::Dial2 => 1,
            Dials::Dial3 => 2,
            Dials::Dial4 => 3,
        };

        let command_index = self.get_command_index();
        if let Some(device) = self.devices_shown.get(device_index) {
            let error = anyhow!("Failed to get Renderer");
            let current = self.renderers.get(device).ok_or(error)?;

            let volume = current.volumes[self.active_mix] as i16;
            let new_volume = (volume + change as i16).clamp(0, 100) as u8;

            let command = serde_json::to_string(&WebsocketRequest {
                id: command_index,
                data: DaemonRequest::Pipewire(SetSourceVolume(
                    *device,
                    self.active_mix,
                    new_volume as u8,
                )),
            })?;

            stream.send(Message::Text(Utf8Bytes::from(command))).await?;
        }

        Ok(())
    }
}

pub fn spawn_pipeweaver_handler(
    sender: Sender<ControlMessage>,
    device: DeviceType,
    input_rx: Receiver<Interactions>,
) {
    let mut handler = PipeweaverHandler::new(device, sender, input_rx);
    runtime().spawn(async move { handler.run_handler().await });
}

fn img_as_jpeg(image: RgbaImage, background: Rgba<u8>) -> Result<Vec<u8>> {
    DrawingUtils::image_as_jpeg(image, background, JPEG_QUALITY)
}

fn jpeg_as_img(image: &[u8]) -> Result<RgbaImage> {
    if let Ok(img) = load_from_memory(image) {
        return Ok(img.into_rgba8());
    }
    bail!("Failed to load image");
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
            Err(e) => {
                debug!("Error Occurred, stopping sync wrapper: {}", e);
                break;
            }
        }
    }
    Ok(())
}
