#[cfg(target_arch = "wasm32")]
use tokio_with_wasm as tokio;

use crate::devices::manager::ControlMessage;
pub use crate::integrations::pipeweaver::channel::MeterSettings;
use crate::integrations::pipeweaver::channel::{ChannelChangedProperty, ChannelRenderer};
use crate::integrations::pipeweaver::helpers::{Mix, MuteTarget, OrderGroup};

use crate::integrations::pipeweaver::layout::{
    DrawingUtils, FONT_BOLD, JPEG_QUALITY, TEXT_COLOUR, TextAlign,
};
use crate::integrations::pipeweaver::pieces::{
    HEADER_STRIP, PieceCache, PieceKey, Plate, ScreenModel, SlotShown, dial_origin, plate_jpeg,
    slot_origin,
};
use anyhow::{Result, anyhow, bail};
use beacn_lib::controller::messages::Message as BeacnMessage;
use beacn_lib::controller::{ButtonLighting, ButtonState, Buttons, Dials, Interactions};
use beacn_lib::flume::{Receiver, Sender, TryRecvError};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::RGBA;
use enum_map::{EnumMap, enum_map};
use iced::futures::SinkExt;
use iced::futures::StreamExt;
use image::{Rgba, RgbaImage};
use json_patch::Patch;
use log::{debug, info, warn};
use serde::Deserialize;
use serde_json::{Value, from_value, json};
use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::sync::{Arc, LazyLock};
use strum::IntoEnumIterator;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::{select, task, time};
use tokio_tungstenite_wasm::{Message, Utf8Bytes, WebSocketStream, connect};
use web_time::{Duration, Instant};

const HELD_TIME: Duration = Duration::from_millis(500);

// How long we wait for the daemon to echo back a volume we sent before giving up and resyncing
// from the daemon's real state.
const PENDING_TIMEOUT: Duration = Duration::from_millis(150);

// If a frame arrives very late (stalled device, suspended machine) don't let the meters lurch
// forward by the whole gap, just carry on from where they were.
const MAX_FRAME_DT: f32 = 0.1;

static PW_SPLASH: LazyLock<Arc<Vec<u8>>> = LazyLock::new(|| {
    let bytes = include_bytes!("../../../resources/screens/beacn-pipeweaver.jpg");
    Arc::new(bytes.to_vec())
});

// Simple method that checks whether pipeweaver is running, and if so, launches the UI
pub fn launch_pipeweaver_ui() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return {
        use crate::integrations::pipeweaver::helpers::{
            get_pipeweaver_socket_path, read_json, send_json,
        };
        use interprocess::local_socket::tokio::prelude::LocalSocketStream;
        use interprocess::local_socket::traits::tokio::Stream;
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        use tokio::runtime::Handle;
        if let Ok(path) = get_pipeweaver_socket_path()
            && let Ok(file_name) = path.to_fs_name::<GenericFilePath>()
        {
            return task::block_in_place(|| {
                Handle::current().block_on(async move {
                    if let Ok(mut stream) = LocalSocketStream::connect(file_name).await {
                        let command = json!( {
                            "Daemon": "OpenInterface",
                        });
                        if let Err(e) = send_json(&mut stream, &command).await {
                            warn!("Failed to send command to Pipeweaver: {}", e);
                            return false;
                        }

                        let Ok(response) = read_json(&mut stream).await else {
                            warn!("Failed to read response from Pipeweaver");
                            return false;
                        };

                        let Some(response) = response.as_str() else {
                            warn!("Failed to parse response from Pipeweaver");
                            return false;
                        };

                        return match response {
                            "Ok" => true,
                            _ => {
                                warn!("Unexpected response from Pipeweaver: {}", response);
                                false
                            }
                        };
                    }
                    warn!("Failed to connect to Pipeweaver");
                    false
                })
            });
        }
        warn!("Cannot locate Pipeweaver Socket");
        false
    };

    #[cfg(target_arch = "wasm32")]
    false
}

mod channel;
mod helpers;
mod layout;
mod pieces;

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

// This is a mapping for the meter messages
#[derive(Debug, Deserialize)]
struct MeterMessage {
    id: String,
    percent: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct ButtonHoldState {
    pub(crate) press_time: Option<Instant>,

    pub(crate) skip_hold: bool,
    pub(crate) skip_release: bool,
    pub(crate) hold_handled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelType {
    Source,
    Target,
}

type WebSocket = WebSocketStream;
type Renderers = HashMap<String, ChannelRenderer>;

struct PipeweaverHandler {
    device_type: DeviceType,
    sender: Sender<ControlMessage>,
    input_rx: Receiver<Interactions>,
    stop_rx: watch::Receiver<bool>,
    suspended_rx: watch::Receiver<bool>,
    temporary_active: bool,

    has_connected: bool,
    displaying_error: bool,

    command_index: u64,
    raw_status: Value,
    //status: DaemonStatus,
    active_page: u8,
    channel_type: ChannelType,
    active_mix: Mix,
    devices_shown: Vec<String>,
    renderers: Renderers,
    button_down_states: EnumMap<Buttons, Option<ButtonHoldState>>,

    // In-flight dial changes per channel
    pending_volumes: HashMap<String, (u8, Instant)>,

    // JPEG Cache and shadow model
    pieces: PieceCache,
    screen: ScreenModel,

    meter_settings: MeterSettings,
}

impl PipeweaverHandler {
    pub fn new(
        device_type: DeviceType,
        sender: Sender<ControlMessage>,
        input_rx: Receiver<Interactions>,
        stop_rx: watch::Receiver<bool>,
        suspended_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            device_type,
            sender,
            input_rx,
            stop_rx,
            suspended_rx,
            temporary_active: false,

            has_connected: false,
            displaying_error: false,

            command_index: 0,
            raw_status: Value::Null,
            //status: DaemonStatus::default(),
            active_page: 0,
            channel_type: ChannelType::Source,
            active_mix: Mix::A,
            devices_shown: Vec::with_capacity(4),
            renderers: HashMap::new(),
            button_down_states: EnumMap::default(),
            pending_volumes: HashMap::new(),
            pieces: PieceCache::default(),
            screen: ScreenModel::default(),
            meter_settings: MeterSettings::default(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(unreachable_code))]
    pub async fn run_handler(&mut self) {
        info!("Starting Pipeweaver Manager");

        // Send the Pipeweaver Splash
        self.draw_splash().await;

        #[cfg(target_arch = "wasm32")]
        {
            let msg = "Pipeweaver is not supported on the Web";
            self.draw_status(msg).await;
            return;
        }

        let mut clean_stop = true;
        let url = "ws://localhost:14565/api/websocket";
        let meter = "ws://localhost:14565/api/websocket/meter";

        self.draw_status("Loading...").await;

        sleep(Duration::from_millis(250)).await;

        self.disable_buttons().await;

        // We need to handle this in a loop, if something goes bad just make sure we're disconnencted
        // and try again after 5 seconds,
        'connect: while let Err(e) = self.handle_connection(url, meter).await {
            // It doesn't matter if we lose an input here, we're not handling them anyway.
            if matches!(self.input_rx.try_recv(), Err(TryRecvError::Disconnected)) {
                warn!("Interaction Handler Terminated, Bailing!");
                clean_stop = false;
                break;
            }

            if !self.displaying_error {
                if !self.has_connected {
                    self.draw_status("Failed to connect to Pipeweaver").await;
                    self.disable_buttons().await;
                } else {
                    self.draw_splash().await;
                    self.draw_status("Connection to Pipeweaver lost").await;
                    self.disable_buttons().await;
                }
            }
            self.displaying_error = true;

            // We only suppress 'Connection Refused' errors, as they're expected to happen
            let is_connection_refused = e
                .downcast_ref::<tokio_tungstenite_wasm::Error>()
                .and_then(|e| {
                    if let tokio_tungstenite_wasm::Error::Io(io) = e {
                        Some(io)
                    } else {
                        None
                    }
                })
                .map(|io| io.kind() == ErrorKind::ConnectionRefused)
                .unwrap_or(false);

            if !is_connection_refused {
                warn!("Pipeweaver Error: {}", e);
            }

            if *self.stop_rx.borrow() {
                info!("Shutdown Requested, terminating");
                break 'connect;
            }

            // Drain (and ignore) incoming interactions while disconnected
            loop {
                select! {
                    Ok(_) = self.input_rx.recv_async() => {
                        // We need to NOOP this, drain the channel so messages don't queue.
                    }
                    Ok(_) = self.stop_rx.changed() => {
                        break 'connect;
                    }
                    _ = sleep(Duration::from_secs(5)) => {
                        // 5 Seconds have elapsed, break this loop to reconnect
                        continue 'connect;
                    }
                }
            }
        }

        info!("Pipeweaver Manager Terminated");
        if clean_stop {
            self.draw_splash().await;
            self.draw_status("Beacn Utility Stopped").await;
            self.disable_buttons().await;
        }
    }

    async fn draw_splash(&mut self) {
        let message = BeacnMessage::Image(0, 0, PW_SPLASH.clone());
        let _ = self.send_message(message).await;

        // The splash covers everything, invalidate our model
        self.screen.invalidate();
    }

    async fn draw_status(&self, text: &str) {
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
            let message = BeacnMessage::Image(0, 330, Arc::new(img));
            let _ = self.send_message(message).await;
        }
    }

    async fn disable_buttons(&self) {
        for button in ButtonLighting::iter() {
            let msg = BeacnMessage::ButtonColour(button, COLOUR_BLACK);
            let _ = self.send_message(msg).await;
        }
    }

    async fn handle_connection(&mut self, url: &str, meter: &str) -> Result<()> {
        let mut stream = self.connect_with_stop(url).await?;
        let mut meter = self.connect_with_stop(meter).await?;
        info!("Successfully connected to Pipeweaver");

        self.has_connected = true;
        self.displaying_error = false;

        self.load_status(&mut stream).await?;
        self.load_initial_state().await?;
        self.run_message_loop(&mut stream, &mut meter).await?;

        Ok(())
    }

    async fn connect_with_stop(&mut self, url: &str) -> Result<WebSocket> {
        select! {
            result = connect(url) => {
                Ok(result?)
            }
            Ok(_) = self.stop_rx.changed() => {
                bail!("Shutdown requested")
            }
        }
    }

    async fn load_status(&mut self, stream: &mut WebSocket) -> Result<()> {
        // Perform the Initial Status Fetch
        let status_id = self.get_command_index();

        let status_request = json!({
            "id": status_id,
            "data": "GetStatus",
        });
        let status_request = serde_json::to_string(&status_request)?;

        let message = Message::Text(Utf8Bytes::from(status_request));
        if let Err(e) = stream.send(message).await {
            bail!("Failed to fetch Status: {}", e)
        }

        // There are occasionally patch messages which could occur before the status response,
        // so we'll loop here until we get the answer we're looking for
        loop {
            select! {
                message = stream.next() => match message {
                    Some(Ok(Message::Text(msg))) => {
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

                            break;
                        }
                    }

                    Some(Ok(Message::Close(frame))) => {
                        bail!("Pipeweaver closed websocket: {:?}", frame);
                    }

                    Some(Ok(other)) => {
                        debug!("Ignoring websocket message during status load: {:?}", other);
                    }

                    Some(Err(e)) => {
                        return Err(e.into());
                    }

                    None => {
                        bail!("Pipeweaver websocket closed while loading status");
                    }
                },

                Ok(_) = self.stop_rx.changed() => {
                    bail!("Shutdown Requested");
                }
            }
        }
        Ok(())
    }

    async fn load_initial_state(&mut self) -> Result<()> {
        self.pending_volumes.clear();

        // Start clean, we may be reconnecting, or switching between Sources and Targets
        self.renderers.clear();

        let devices_shown = self.get_channels_on_page()?;
        self.devices_shown = devices_shown;

        // Update the Rendering Nodes
        self.update_renderers()?;

        // Warm the pieces cache before we start
        self.warm_pieces();

        // Perform the initial screen render
        self.perform_full_refresh().await?;

        Ok(())
    }

    async fn run_message_loop(
        &mut self,
        stream: &mut WebSocket,
        meter: &mut WebSocket,
    ) -> Result<()> {
        let mut keep_alive = time::interval(Duration::from_secs(10));
        self.send_message(BeacnMessage::Enabled(true)).await?;

        let mut last_channel_count = 0;

        // Meter values only arrive every so often, so while any visible meter is still moving
        // we run our own frame timer, and animate every channel together on each frame.
        let fps = self.meter_settings.fps.clamp(1, 120);
        let frame_period = Duration::from_secs_f32(1.0 / fps as f32);
        let mut last_frame = Instant::now();
        let mut next_frame = time::Instant::now();
        let mut was_animating = false;

        let frame_sleep = tokio::time::sleep(Duration::MAX);
        tokio::pin!(frame_sleep);

        let suspend_sleep = tokio::time::sleep(Duration::MAX);
        tokio::pin!(suspend_sleep);

        let mut ticker = time::interval(Duration::from_millis(20));

        debug!("Starting Pipeweaver Message Loop");
        loop {
            let is_suspended = self.is_suspended();

            // When meters start moving, the frame clock starts from now rather than from whenever
            // it last ran, otherwise the first frame would see a huge gap
            let animating = self.is_animating();
            if animating && !was_animating {
                last_frame = Instant::now();
                next_frame = time::Instant::now() + frame_period;
                frame_sleep.as_mut().reset(next_frame);
            }
            was_animating = animating;

            select! {
                Ok(_) = self.stop_rx.changed() => {
                    // Trigger a safe exit
                    return Ok(());
                }

                Ok(_) = self.suspended_rx.changed() => {
                    // We've woken up from a suspension, so redraw everything
                    if !self.is_suspended() {
                        self.screen.invalidate();
                        self.refresh_page().await?;
                    }

                    // Restart the loop, just in case there are other redraws needed
                    continue;
               }

                message = stream.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            let result: Value = serde_json::from_str(&text)?;

                            if !result["data"]["Patch"].is_null() {
                                // Update the raw status for the change
                                let patch: Patch = from_value(result["data"]["Patch"].clone())?;
                                json_patch::patch(&mut self.raw_status, &patch)?;

                                // Count all channels that aren't hidden
                                let count = {
                                    let order = self.get_channel_order()?;
                                    order
                                        .iter()
                                        .filter(|(group, _)| *group != OrderGroup::Hidden)
                                        .map(|(_, v)| v.len())
                                        .sum::<usize>()
                                };

                                if count != last_channel_count {
                                    last_channel_count = count;
                                    self.load_page_button().await?;
                                }

                                self.handle_status_change().await?;
                            }
                        }
                        Some(Ok(Message::Close(frame))) => {
                            bail!("Server closed websocket: {:?}", frame);
                        }
                        Some(Ok(other)) => {
                            debug!("Ignoring websocket message: {:?}", other);
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => bail!("Websocket Closed"),
                    }
                }
                message = meter.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            let result = serde_json::from_str::<MeterMessage>(&text)?;
                            if let Some(renderer) = self.renderers.get_mut(&result.id) {
                                renderer.meter_target = f32::from(result.percent);
                            }
                        }
                        Some(Ok(Message::Close(frame))) => {
                            bail!("Server closed websocket: {:?}", frame);
                        }
                        Some(Ok(other)) => {
                            debug!("Ignoring websocket message: {:?}", other);
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => bail!("Websocket Closed"),
                    }
                }
                _ = &mut frame_sleep, if animating => {
                    let now = Instant::now();
                    let dt = now.duration_since(last_frame).as_secs_f32().min(MAX_FRAME_DT);
                    last_frame = now;

                    self.animate_meters(dt).await?;

                    // Keep frame time steady
                    next_frame = (next_frame + frame_period).max(time::Instant::now());
                    frame_sleep.as_mut().reset(next_frame);
                }

                _ = &mut suspend_sleep, if self.is_suspended() => {
                    // We should be sleeping, and something woke us up, so put us back to sleep
                    self.send_message(BeacnMessage::Enabled(false)).await?;
                    self.temporary_active = false;
                }

                maybe_msg = self.input_rx.recv_async() => {
                    match maybe_msg {
                        Ok(msg) => {
                            if is_suspended {
                                // Reset the timer in all cases
                                suspend_sleep.as_mut().reset(time::Instant::now() + Duration::from_secs(5));

                                if !self.temporary_active {
                                    // Wake the device up, and flag as temporarily active
                                    self.send_message(BeacnMessage::Enabled(true)).await?;
                                    self.temporary_active = true;
                                    self.sync_display().await?;
                                }
                            }

                            match self.device_type {
                                DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                                    match msg {
                                        Interactions::ButtonPress(button, state) => {
                                            match state {
                                                ButtonState::Press => self.on_button_down(button, stream).await?,
                                                ButtonState::Release => self.on_button_up(button, stream).await?,
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
                        Err(_) => bail!("Receive Handler Closed!")
                    }
                }
                _instant = keep_alive.tick() => {
                    self.send_message(BeacnMessage::KeepAlive).await?;
                }

                _ = ticker.tick() => {
                    self.check_held().await?;
                    self.expire_pending_volumes().await?;
                }
            }
        }
    }

    async fn perform_full_refresh(&mut self) -> Result<()> {
        self.sync_display_now().await?;
        self.load_all_dial_button_colours().await?;
        self.load_page_button().await?;
        self.load_mix_button_colours().await?;

        Ok(())
    }

    fn update_renderers(&mut self) -> Result<()> {
        let ids = self.displayable_channels(self.channel_type)?;
        for id in &ids {
            if !self.renderers.contains_key(id) {
                let render = self.get_channel_renderer_for(self.channel_type, id)?;
                self.renderers.insert(id.clone(), render);
            }
        }

        // Anything which no longer exists goes, along with any dial change still in flight for it
        let live: HashSet<&String> = ids.iter().collect();
        self.renderers.retain(|id, _| live.contains(id));
        self.pending_volumes.retain(|id, _| live.contains(id));
        Ok(())
    }

    // Brings every renderer up to date with the daemon, if there's a colour change a more
    // significant redrawing will be needed, so return it in preparation
    fn refresh_renderers(&mut self) -> Result<HashSet<String>> {
        let mut colour_changed = HashSet::new();
        let active_mix = self.active_mix;
        let channel_type = self.channel_type;

        let ids: Vec<String> = self.renderers.keys().cloned().collect();
        for id in ids {
            let dev = find_device(&self.raw_status, channel_type, &id)?;
            let render = self
                .renderers
                .get_mut(&id)
                .ok_or_else(|| anyhow!("Failed to get renderer"))?;

            let updates = match channel_type {
                ChannelType::Source => render.update_from_source_device_value(dev)?,
                ChannelType::Target => render.update_from_target_device_value(dev)?,
            };

            if updates.contains(&ChannelChangedProperty::Colour) {
                colour_changed.insert(id.clone());
            }

            if let Some(&(target, at)) = self.pending_volumes.get(&id) {
                if render.volumes[active_mix] == target || at.elapsed() > PENDING_TIMEOUT {
                    // Caught up, or gave up waiting: the daemon's value stands
                    self.pending_volumes.remove(&id);
                } else {
                    render.volumes[active_mix] = target;
                }
            }
        }
        Ok(colour_changed)
    }

    // Called after every change to the daemon's status
    async fn handle_status_change(&mut self) -> Result<()> {
        // Channels may have been added or removed, so make sure we track exactly what exists
        self.update_renderers()?;

        // Update everything we track, including channels which aren't on this page
        let colour_changed = self.refresh_renderers()?;

        let devices = self.get_channels_on_page()?;
        let page_changed = devices != self.devices_shown;
        self.devices_shown = devices;

        if page_changed {
            self.load_all_dial_button_colours().await?;
        } else {
            for index in 0..self.devices_shown.len() {
                if colour_changed.contains(&self.devices_shown[index]) {
                    self.load_dial_button_colour(index).await?;
                }
            }
        }

        // Send whatever is different from what the device is showing (which may be nothing)
        self.sync_display().await?;

        // Anything which changed may have created new pieces, or made old ones obsolete
        self.warm_pieces();
        Ok(())
    }

    // Whether any channel we're showing still has a meter on the move
    fn is_animating(&self) -> bool {
        self.can_draw()
            && self
                .devices_shown
                .iter()
                .filter_map(|id| self.renderers.get(id))
                .any(|renderer| !renderer.meter_settled())
    }

    // Advances every visible meter by one frame and redraws whichever dials actually changed
    async fn animate_meters(&mut self, dt: f32) -> Result<()> {
        let settings = self.meter_settings;
        for id in &self.devices_shown {
            if let Some(renderer) = self.renderers.get_mut(id) {
                renderer.tick_meter(dt, &settings);
            }
        }

        // Drawing skips dials that look the same as what's already there
        for index in 0..self.devices_shown.len() {
            self.sync_dial(index).await?;
        }
        Ok(())
    }

    fn can_draw(&self) -> bool {
        !self.is_suspended() || self.temporary_active
    }

    // Brings the device in line with what we want it to show, if we're currently allowed to draw
    async fn sync_display(&mut self) -> Result<()> {
        if !self.can_draw() {
            return Ok(());
        }
        self.sync_display_now().await
    }

    async fn sync_display_now(&mut self) -> Result<()> {
        let mut messages = Vec::new();
        for index in 0..self.screen.slots.len() {
            messages.extend(self.plan_slot(index, false)?);
        }

        if !self.screen.header_drawn {
            messages.insert(0, BeacnMessage::Image(0, 0, HEADER_STRIP.clone()));
            self.screen.header_drawn = true;
        }

        for message in messages {
            self.send_message(message).await?;
        }
        Ok(())
    }

    // Just the dial, which is what meters and dial turns change
    async fn sync_dial(&mut self, index: usize) -> Result<()> {
        if !self.can_draw() {
            return Ok(());
        }
        for message in self.plan_slot(index, true)? {
            self.send_message(message).await?;
        }
        Ok(())
    }

    // Works out what a slot needs sending to match its renderer, and records that we sent it.
    // Sending a plate wipes the slot, so everything on top of it has to be sent again after it.
    fn plan_slot(&mut self, index: usize, dial_only: bool) -> Result<Vec<BeacnMessage>> {
        let mut out = Vec::new();
        if index >= self.screen.slots.len() {
            return Ok(out);
        }

        let (slot_x, slot_y) = slot_origin(index);
        let mix = self.active_mix;

        // These are all different fields, so they can be borrowed side by side
        let renderer = self
            .devices_shown
            .get(index)
            .and_then(|id| self.renderers.get(id));
        let shown = &mut self.screen.slots[index];

        let Some(renderer) = renderer else {
            // Nothing belongs in this slot, so it needs to be blank
            if !dial_only && shown.plate != Some(Plate::Blank) {
                out.push(BeacnMessage::Image(
                    slot_x,
                    slot_y,
                    plate_jpeg(Plate::Blank),
                ));
                *shown = SlotShown {
                    plate: Some(Plate::Blank),
                    ..Default::default()
                };
            }
            return Ok(out);
        };

        if !dial_only {
            let plate = renderer.plate();
            if shown.plate != Some(plate) {
                out.push(BeacnMessage::Image(slot_x, slot_y, plate_jpeg(plate)));
                *shown = SlotShown {
                    plate: Some(plate),
                    ..Default::default()
                };
            }

            let top = renderer.top_key();
            if shown.top.as_ref() != Some(&top) {
                let (x, y) = top.origin();
                let jpeg = self.pieces.get(&top)?;
                out.push(BeacnMessage::Image(slot_x + x, slot_y + y, jpeg));
                shown.top = Some(top);
            }
        }

        let dial = renderer.dial_key(mix);
        if shown.dial != Some(dial) {
            let (x, y) = dial_origin();
            let jpeg = renderer.get_volume(mix)?;
            out.push(BeacnMessage::Image(slot_x + x, slot_y + y, jpeg));
            shown.dial = Some(dial);
        }

        if !dial_only {
            let mute = renderer.mute_key();
            if shown.mute.as_ref() != Some(&mute) {
                let (x, y) = mute.origin();
                let jpeg = self.pieces.get(&mute)?;
                out.push(BeacnMessage::Image(slot_x + x, slot_y + y, jpeg));
                shown.mute = Some(mute);
            }
        }

        Ok(out)
    }

    // Renders every piece any channel could show (on any page, Source or Target), then drops
    // anything nothing can produce any more. Anything already rendered is skipped.
    fn warm_pieces(&mut self) {
        let mut keys: Vec<PieceKey> = self
            .renderers
            .values()
            .flat_map(|renderer| renderer.all_keys())
            .collect();

        // The other channel type is only a button hold away, so keep that ready too
        let other = match self.channel_type {
            ChannelType::Source => ChannelType::Target,
            ChannelType::Target => ChannelType::Source,
        };
        match self.build_renderers(other) {
            Ok(renderers) => {
                keys.extend(renderers.iter().flat_map(|renderer| renderer.all_keys()));
            }
            Err(e) => debug!("Unable to prepare {:?} channels: {}", other, e),
        }

        let mut live = HashSet::with_capacity(keys.len());
        for key in keys {
            if let Err(e) = self.pieces.get(&key) {
                warn!("Failed to render {:?}: {}", key, e);
            }
            live.insert(key);
        }
        self.pieces.retain_live(&live);
    }

    // Re-reads a single channel's state from the daemon's status, discarding any optimistic values
    fn resync_renderer(&mut self, device: &str) -> Result<()> {
        let dev = find_device(&self.raw_status, self.channel_type, device)?;
        let render = self
            .renderers
            .get_mut(device)
            .ok_or_else(|| anyhow!("Failed to get renderer"))?;

        let _ = match self.channel_type {
            ChannelType::Source => render.update_from_source_device_value(dev)?,
            ChannelType::Target => render.update_from_target_device_value(dev)?,
        };
        Ok(())
    }

    // If the daemon never confirmed a value we sent (clamped, rejected, etc), fall back to its
    // real state so the display doesn't keep showing our optimistic number.
    async fn expire_pending_volumes(&mut self) -> Result<()> {
        let expired: Vec<String> = self
            .pending_volumes
            .iter()
            .filter(|(_, (_, at))| at.elapsed() > PENDING_TIMEOUT)
            .map(|(id, _)| id.clone())
            .collect();

        if expired.is_empty() {
            return Ok(());
        }

        for id in &expired {
            self.pending_volumes.remove(id);
            if self.renderers.contains_key(id) {
                self.resync_renderer(id)?;
            }
        }

        self.sync_display().await
    }

    async fn load_all_dial_button_colours(&self) -> Result<()> {
        for index in 0..self.devices_shown.len() {
            self.load_dial_button_colour(index).await?;
        }
        Ok(())
    }

    async fn load_page_button(&mut self) -> Result<()> {
        let pages = self.get_page_count()?;
        if self.active_page >= pages {
            self.active_page = pages - 1;
        }

        let left_colour = match self.active_page == 0 {
            true => COLOUR_BLACK,
            false => COLOUR_WHITE,
        };

        // Map the Previous / Next Button colours
        let right_colour = match self.get_page_count()? {
            1 => COLOUR_BLACK,
            c => match self.active_page == c - 1 {
                true => COLOUR_BLACK,
                false => COLOUR_WHITE,
            },
        };

        // Send the page colours
        self.set_button_colour(ButtonLighting::Left, left_colour)
            .await?;
        self.set_button_colour(ButtonLighting::Right, right_colour)
            .await?;

        Ok(())
    }

    async fn load_mix_button_colours(&self) -> Result<()> {
        let colour = match self.channel_type {
            ChannelType::Source => match self.active_mix {
                Mix::A => COLOUR_MIX_B,
                Mix::B => COLOUR_MIX_A,
            },

            ChannelType::Target => COLOUR_BLACK,
        };

        self.set_button_colour(ButtonLighting::Mix, colour).await?;
        Ok(())
    }

    async fn load_dial_button_colour(&self, index: usize) -> Result<()> {
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

        self.set_button_colour(dial_button, beacn_colour).await?;
        Ok(())
    }

    fn get_command_index(&mut self) -> u64 {
        let result = self.command_index;
        self.command_index += 1;
        result
    }

    fn get_channel_renderer_for(
        &self,
        channel_type: ChannelType,
        device: &str,
    ) -> Result<ChannelRenderer> {
        let dev = find_device(&self.raw_status, channel_type, device)?;

        let mut renderer = match channel_type {
            ChannelType::Source => ChannelRenderer::from_source_device_value(dev)?,
            ChannelType::Target => ChannelRenderer::from_target_device_value(dev)?,
        };

        renderer.set_beacn_device(self.device_type);
        Ok(renderer)
    }

    fn build_renderers(&self, channel_type: ChannelType) -> Result<Vec<ChannelRenderer>> {
        self.displayable_channels(channel_type)?
            .iter()
            .map(|id| self.get_channel_renderer_for(channel_type, id))
            .collect()
    }

    async fn refresh_page(&mut self) -> Result<()> {
        self.pending_volumes.clear();

        self.devices_shown = self.get_channels_on_page()?;
        self.update_renderers()?;
        self.perform_full_refresh().await?;
        Ok(())
    }

    async fn send_message(&self, message: BeacnMessage) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let msg = ControlMessage::Handle(message, tx);
        self.sender.send_async(msg).await?;

        rx.await??;
        Ok(())
    }

    fn get_page_count(&self) -> Result<u8> {
        let order = self.get_channel_order()?;

        // If we can't display any other channels because we're populated with pins, send 1 page.
        if order[OrderGroup::Pinned].len() >= 4 || order[OrderGroup::Default].is_empty() {
            return Ok(1);
        }

        let channels_per_page = 4 - order[OrderGroup::Pinned].len() as u8;
        let channel_count = order[OrderGroup::Default].len() as u8;
        Ok((channels_per_page + channel_count - 1) / channels_per_page)
    }

    fn get_channels_on_page(&self) -> Result<Vec<String>> {
        let order = self.get_channel_order()?;
        let mut channels = Vec::with_capacity(4);

        // This is a little complicated, we need to check the pinned channels and add them first
        let pinned = &order[OrderGroup::Pinned];
        let others = &order[OrderGroup::Default];

        if pinned.is_empty() && others.is_empty() {
            warn!("No channels are defined!");
            return Ok(channels);
        }

        // The pinned options should appear on all the pages
        for channel in pinned.iter().take(channels.capacity() - channels.len()) {
            channels.push(channel.clone());
        }

        // If the user has 4 pinned channels, we really can't do paging
        if channels.len() == channels.capacity() {
            return Ok(channels);
        }

        // Ok, now we need to work out how many non-pinned channels per page we can have
        let channels_per_page = 4 - pinned.len() as u8;

        if others.len() < channels_per_page as usize {
            for other in others {
                channels.push(other.clone());
            }
            return Ok(channels);
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
                channels.push(channel.clone());
            }
        }

        Ok(channels)
    }

    fn get_channel_order(&self) -> Result<EnumMap<OrderGroup, Vec<String>>> {
        self.channel_order_for(self.channel_type)
    }

    fn channel_order_for(
        &self,
        channel_type: ChannelType,
    ) -> Result<EnumMap<OrderGroup, Vec<String>>> {
        let base = &self.raw_status["audio"]["profile"]["devices"];

        let order = match channel_type {
            ChannelType::Source => &base["sources"]["device_order"],
            ChannelType::Target => &base["targets"]["device_order"],
        };

        let parse = |group: &str| -> Result<Vec<String>> {
            let values = match order[group].as_array() {
                Some(values) => values,
                None => bail!("missing or invalid {group} device_order"),
            };

            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| anyhow::anyhow!("expected string in {group} device_order"))
                })
                .collect()
        };

        Ok(enum_map! {
            OrderGroup::Pinned => parse("Pinned")?,
            OrderGroup::Default => parse("Default")?,
            OrderGroup::Hidden => parse("Hidden")?,
        })
    }

    // Every channel which can appear on some page (Hidden channels never do)
    fn displayable_channels(&self, channel_type: ChannelType) -> Result<Vec<String>> {
        let order = self.channel_order_for(channel_type)?;
        Ok(order[OrderGroup::Pinned]
            .iter()
            .chain(order[OrderGroup::Default].iter())
            .cloned()
            .collect())
    }

    async fn set_button_colour(&self, button: ButtonLighting, colour: RGBA) -> Result<()> {
        let message = BeacnMessage::ButtonColour(button, colour);
        self.send_message(message).await?;
        Ok(())
    }

    async fn on_button_down(&mut self, button: Buttons, stream: &mut WebSocket) -> Result<()> {
        debug!("Button Down: {:?}", button);

        // Only the dial buttons have a hold behaviour, so they have to wait for the release
        // to know whether this was a press or a hold. Everything else can fire immediately.
        let has_hold = matches!(
            button,
            Buttons::Dial1 | Buttons::Dial2 | Buttons::Dial3 | Buttons::Dial4
        );

        self.button_down_states[button].replace(ButtonHoldState {
            press_time: Some(Instant::now()),
            skip_hold: !has_hold,
            skip_release: !has_hold,
            hold_handled: false,
        });

        if !has_hold {
            self.handle_button(button, stream).await?;
        }

        Ok(())
    }

    async fn on_button_up(&mut self, button: Buttons, stream: &mut WebSocket) -> Result<()> {
        debug!("Button Up: {:?}", button);

        // Have we been instructed to skip release behaviour for this button?
        if let Some(state) = self.button_down_states[button]
            && state.skip_release
        {
            debug!("State: {:?}", state);
            debug!("Skipping Release Behaviour for Button: {:?}", button);

            // Take the handler, and return.
            self.button_down_states[button].take();
            return Ok(());
        }

        debug!("Button Up handling normally: {}..", button);

        // Handle the button up normally
        self.handle_button(button, stream).await?;
        if self.button_down_states[button].is_some() {
            self.button_down_states[button].take();
        }

        Ok(())
    }

    async fn on_button_held(&mut self, button: Buttons) -> Result<()> {
        debug!("Button Held: {:?}", button);

        // Handle holding should only occur once per button press, so regardless of what happens
        // below, flag this as handled so we don't try to handle it again, and if it's already
        // been handled, we can just return.
        if let Some(state) = &mut self.button_down_states[button] {
            if state.hold_handled {
                return Ok(());
            } else {
                state.hold_handled = true;
            }
        }

        // Button has been held, handle hold behaviour here.
        match button {
            Buttons::Dial1 | Buttons::Dial2 | Buttons::Dial3 | Buttons::Dial4 => {
                // Switch from Sources to Targets
                self.channel_type = match self.channel_type {
                    ChannelType::Source => ChannelType::Target,
                    ChannelType::Target => ChannelType::Source,
                };

                // We need to reload from scratch, so load the new initial state
                self.active_page = 0;
                self.active_mix = Mix::A;

                let _ = self.load_initial_state().await;

                // Don't handle the release for this button, it's already handled.
                if let Some(state) = &mut self.button_down_states[button] {
                    state.skip_release = true;
                }
            }
            _ => {}
        }

        // If this isn't handled, do nothing.
        Ok(())
    }

    async fn check_held(&mut self) -> Result<()> {
        for button in Buttons::iter() {
            if let Some(state) = self.button_down_states[button] {
                // If we don't have a press time, there's nothing to handle.
                if let Some(time) = state.press_time
                    && !state.hold_handled
                    && !state.skip_hold
                    && time.elapsed() > HELD_TIME
                {
                    self.on_button_held(button).await?;
                }
            }
        }
        Ok(())
    }

    // Handle Button Presses
    async fn handle_button(&mut self, button: Buttons, stream: &mut WebSocket) -> Result<()> {
        match button {
            Buttons::AudienceMix => {
                // If we're set to target mode, we shouldn't handle this.
                if self.channel_type == ChannelType::Target {
                    return Ok(());
                }

                // This one is now stupidly simple
                self.active_mix = match self.active_mix {
                    Mix::A => Mix::B,
                    Mix::B => Mix::A,
                };
                self.pending_volumes.clear();
                self.sync_display().await?;
                self.load_mix_button_colours().await?;
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
                if self.active_page == self.get_page_count()? - 1 && change == 1 {
                    return Ok(());
                }

                self.active_page = self.active_page.wrapping_add_signed(change);

                if !self.is_suspended() || self.temporary_active {
                    self.refresh_page().await?;
                }
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

                    let message = match current.channel_type {
                        ChannelType::Source => {
                            if current.mute_states[target].is_active {
                                json!({
                                    "DelSourceMuteTarget": [device, target]
                                })
                            } else {
                                json!({
                                    "AddSourceMuteTarget": [device, target]
                                })
                            }
                        }
                        ChannelType::Target => {
                            let muted = current.mute_states[MuteTarget::TargetA].is_active;
                            let state = match muted {
                                true => "Unmuted",
                                false => "Muted",
                            };
                            json!({
                                "SetTargetMuteState": [device, state]
                            })
                        }
                    };

                    let command = json!({
                        "id": self.get_command_index(),
                        "data": { "Pipewire": message },
                    });
                    let command = serde_json::to_string(&command)?;
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

        if let Some(device) = self.devices_shown.get(device_index).cloned() {
            let mix = self.active_mix;
            let error = anyhow!("Failed to get Renderer");
            let renderer = self.renderers.get_mut(&device).ok_or(error)?;

            // The renderer's volume is kept optimistic while a change is in flight, so it's
            // always the correct baseline, and rapid turns accumulate.
            let volume = renderer.volumes[mix];
            let new_volume = (volume as i16 + change as i16).clamp(0, 100) as u8;
            if new_volume == volume {
                // Clamped no-op, nothing to send (and no pending entry to get stuck)
                return Ok(());
            }

            // Optimistically update the renderer and remember what we're waiting for
            renderer.volumes[mix] = new_volume;
            self.pending_volumes
                .insert(device.clone(), (new_volume, Instant::now()));

            let message = match self.channel_type {
                ChannelType::Source => json!({
                    "SetSourceVolume": [device, mix, new_volume]
                }),
                ChannelType::Target => json!({
                    "SetTargetVolume": [device, new_volume]
                }),
            };

            let command = json!({
                "id": self.get_command_index(),
                "data": { "Pipewire": message },
            });
            let command = serde_json::to_string(&command)?;

            stream.send(Message::Text(Utf8Bytes::from(command))).await?;

            // Give immediate feedback rather than waiting for the daemon's echo
            self.sync_dial(device_index).await?;
        }

        Ok(())
    }

    fn is_suspended(&self) -> bool {
        *self.suspended_rx.borrow()
    }
}

pub fn spawn_pipeweaver_handler(
    sender: Sender<ControlMessage>,
    device: DeviceType,
    input_rx: Receiver<Interactions>,
    stop_rx: watch::Receiver<bool>,
    suspended_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    let mut handler = PipeweaverHandler::new(device, sender, input_rx, stop_rx, suspended_rx);
    task::spawn(async move { handler.run_handler().await })
}

fn img_as_jpeg(image: RgbaImage, background: Rgba<u8>) -> Result<Vec<u8>> {
    DrawingUtils::image_as_jpeg(image, background, JPEG_QUALITY)
}

// Finds a channel's JSON in the daemon's status. This takes the status rather than the handler
// so callers can hold it while mutably borrowing other parts of the handler.
fn find_device<'a>(
    status: &'a Value,
    channel_type: ChannelType,
    device: &str,
) -> Result<&'a Value> {
    let devices = &status["audio"]["profile"]["devices"];
    let origin = match channel_type {
        ChannelType::Source => &devices["sources"],
        ChannelType::Target => &devices["targets"],
    };

    ["physical_devices", "virtual_devices"]
        .iter()
        .filter_map(|kind| origin[*kind].as_array())
        .flatten()
        .find(|value| value["description"]["id"].as_str() == Some(device))
        .ok_or_else(|| anyhow!("Failed to locate device by ID: {}", device))
}
