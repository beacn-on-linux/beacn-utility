use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::compressor::CompressorPage;
use crate::ui::pages::audio::config_pages::expander::ExpanderPage;
use crate::ui::pages::audio::config_pages::headphones::HeadphonesPage;
use crate::ui::pages::audio::config_pages::mic_equaliser::{MicEqualiser, MicEqualiserEvent};
use crate::ui::pages::audio::config_pages::mic_setup::MicrophoneSetup;
use crate::ui::pages::audio::config_pages::suppressor::SuppressorPage;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::pages::page::{AudioPage, PageMessage};
use crate::ui::utility::pipewire::platform::{
    find_pipewire_nodes_for_usb, start_spectrum_analyser,
};
use crate::ui::utility::pipewire::{
    LoopbackHandler, LoopbackHandlerState, PipeWireNodeType, PipeWirePortType, SpectrumHandle,
};
use crate::ui::widgets::helpers::composite::draw_range;
use crate::ui::widgets::helpers::svg::{svg_button_style, svg_coloured_button_unstyled};
use crate::ui::widgets::helpers::tabs::render_tab;
use beacn_lib::audio::data::BulkMessage;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphones::{HPMicOutputGain, Headphones};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::HasRange;
use iced::widget::button::Status;
use iced::widget::canvas::{Frame, Geometry};
use iced::widget::{
    Canvas, Column, Float, Space, button, canvas, column, container, responsive, row, rule, stack,
    text,
};
use iced::{
    Alignment, Background, Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Task,
    Theme, Vector, mouse,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The meter's dB span, shared by the label overlay and the `MicMeter`
/// canvas so the two can never drift out of sync with each other.
const METER_RANGE_DB: (f32, f32) = (-70.0, 0.0);

#[derive(Debug, Clone)]
pub(crate) enum ConfigMessage {
    Equaliser(MicEqualiserEvent),
    Child(ChildMessage),
    SelectTab(usize),

    OutputGainChanged(f32),
    HandleRecording,
    HandlePlayback,
}

pub struct Configuration {
    equaliser: MicEqualiser,
    spectrum_handler: Option<SpectrumHandle>,
    spectrum_data: Option<Arc<Mutex<Vec<f32>>>>,

    loopback_handler: Option<LoopbackHandler>,

    meter_ballistics: MeterBallistics,

    selected_tab: usize,
    tab_pages: Vec<Box<dyn ConfigPage>>,
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            equaliser: MicEqualiser::new(),
            spectrum_handler: None,
            spectrum_data: None,

            loopback_handler: None,

            meter_ballistics: MeterBallistics::new(METER_RANGE_DB.0),

            selected_tab: 0,
            tab_pages: vec![
                Box::new(MicrophoneSetup),
                Box::new(SuppressorPage::new()),
                Box::new(ExpanderPage::new()),
                Box::new(CompressorPage::new()),
                Box::new(HeadphonesPage),
            ],
        }
    }

    fn bottom_view(&self, state: &AudioState) -> Element<'_, ConfigMessage> {
        let content = match self.tab_pages.get(self.selected_tab) {
            Some(page) => page.view(state).map(ConfigMessage::Child),
            None => container(text("No configuration page")).into(),
        };

        let tabs = self
            .tab_pages
            .iter()
            .enumerate()
            .fold(row![], |tabs, (index, page)| {
                let is_active = index == self.selected_tab;

                let mut btn = button(text(page.title()))
                    .style(move |t, s| render_tab(t, s, is_active))
                    .padding(6.0);

                if !is_active {
                    btn = btn.on_press(ConfigMessage::SelectTab(index));
                }
                let separator = container(rule::vertical(1)).center_y(Length::Fixed(28.0));
                tabs.push(btn).push(separator)
            });

        let tab_layout = column![column![tabs], rule::horizontal(1), content]
            .width(Length::Fill)
            .height(Length::Fill);

        tab_layout.into()
    }

    fn gain_view(&self, state: &AudioState) -> Element<'_, ConfigMessage> {
        let value = state.headphones.output_gain;
        let range = HPMicOutputGain::range();
        let gain = draw_range(
            "Output Gain",
            value,
            range,
            "dB",
            ConfigMessage::OutputGainChanged,
        );

        let title = text("Mic Output");
        let title_spacer = Space::new().height(8.0);

        let output_labels = responsive(|size| {
            let (min_db, max_db) = METER_RANGE_DB;
            let y_for_db = |db: f32| ((max_db - db) / (max_db - min_db)) * size.height;
            let label = |value: &'static str, db: f32| {
                let target_y = y_for_db(db);

                Float::new(
                    container(text(value).size(10))
                        .width(Length::Fixed(30.0))
                        .align_x(Alignment::End),
                )
                .translate(move |bounds, _viewport| {
                    Vector::new(-22.0, target_y - bounds.height / 2.0)
                })
            };

            stack![
                // Base layer establishes the exact 30px × full-height label area.
                container(Space::new())
                    .width(Length::Fixed(20.0))
                    .height(Length::Fill),
                label("0", 0.0),
                label("-5", -5.0),
                label("-10", -10.0),
                label("-20", -20.0),
                label("-30", -30.0),
                label("-40", -40.0),
                label("-50", -50.0),
                label("-60", -60.0),
                label("-70", -70.0),
            ]
            .width(Length::Fixed(20.0))
            .height(Length::Fill)
            .into()
        })
        .width(Length::Fixed(30.0))
        .height(Length::Fill);

        let meter = MicMeter {
            db: self.meter_ballistics.db,
            peak_db: self.meter_ballistics.peak_db,
            range_db: METER_RANGE_DB,
        };
        let canvas = Canvas::new(meter).height(Length::Fill).width(Length::Fill);
        let canvas = stack![canvas, output_labels]
            .width(Length::Fixed(40.0))
            .height(Length::Fill);

        let canvas = column![title, title_spacer, canvas]
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center);

        let canvas_container = container(canvas)
            .width(Length::Fill)
            .height(Length::FillPortion(65))
            .align_x(Alignment::Center)
            .padding(8);

        let gain = container(gain)
            .width(Length::Fill)
            .height(Length::FillPortion(35))
            .align_x(Alignment::Center)
            .padding(8);

        let mut children: Vec<Element<'_, ConfigMessage>> = vec![
            canvas_container.into(),
            rule::horizontal(2).into(),
            gain.into(),
        ];

        if let Some(handler) = &self.loopback_handler {
            let output = match handler.state() {
                LoopbackHandlerState::Recording => {
                    format!("{:.1} / 10.0", handler.current_len().as_secs_f32())
                }
                LoopbackHandlerState::Playing => {
                    let current = handler.current_pos();
                    let total = handler.current_len();
                    format!("{:.1} / {:.1}", current.as_secs_f32(), total.as_secs_f32())
                }
                LoopbackHandlerState::Stopped => {
                    let total = handler.current_len();
                    if total.as_secs_f32() == 0.0 {
                        "-".to_string()
                    } else {
                        format!("{:.1}", total.as_secs_f32())
                    }
                }
            };

            let record_icon = match handler.state() {
                LoopbackHandlerState::Recording => "stop",
                LoopbackHandlerState::Playing => "record",
                LoopbackHandlerState::Stopped => "record",
            };
            let record_color = match handler.state() {
                LoopbackHandlerState::Recording => Color::WHITE,
                LoopbackHandlerState::Playing => Color::from_rgb8(138, 138, 138),
                LoopbackHandlerState::Stopped => Color::WHITE,
            };
            let record_action = match handler.state() {
                LoopbackHandlerState::Recording => Some(ConfigMessage::HandleRecording),
                LoopbackHandlerState::Playing => None,
                LoopbackHandlerState::Stopped => Some(ConfigMessage::HandleRecording),
            };

            let play_icon = match handler.state() {
                LoopbackHandlerState::Recording => "play",
                LoopbackHandlerState::Playing => "stop",
                LoopbackHandlerState::Stopped => "play",
            };

            let play_color = match handler.state() {
                LoopbackHandlerState::Recording => Color::from_rgb8(138, 138, 138),
                LoopbackHandlerState::Playing => Color::WHITE,
                LoopbackHandlerState::Stopped => {
                    let total = handler.current_len();
                    if total.as_secs_f32() == 0.0 {
                        Color::from_rgb8(138, 138, 138)
                    } else {
                        Color::WHITE
                    }
                }
            };

            let play_action = match handler.state() {
                LoopbackHandlerState::Recording => None,
                LoopbackHandlerState::Playing => Some(ConfigMessage::HandlePlayback),
                LoopbackHandlerState::Stopped => {
                    let total = handler.current_len();
                    if total.as_secs_f32() == 0.0 {
                        None
                    } else {
                        Some(ConfigMessage::HandlePlayback)
                    }
                }
            };

            let t1 = text(output).size(10.0).width(Length::Fill);
            let a = svg_coloured_button_unstyled(record_icon, record_color)
                .style(move |t, s| {
                    let mut base = svg_button_style(t, s, true);
                    let mut button = t.extended_palette().danger.weak.color;
                    if s == Status::Disabled {
                        button.a = 0.5;
                    };

                    base.background = Some(Background::Color(button));
                    base
                })
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(20.0))
                .padding(2)
                .on_press_maybe(record_action);

            let b = svg_coloured_button_unstyled(play_icon, play_color)
                .style(move |t, s| {
                    let mut base = svg_button_style(t, s, true);
                    let mut button = t.extended_palette().success.weak.color;
                    if s == Status::Disabled {
                        button.a = 0.5;
                    };

                    base.background = Some(Background::Color(button));
                    base
                })
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(20.0))
                .padding(2)
                .on_press_maybe(play_action);

            let row = row![t1, a, b]
                .height(Length::Shrink)
                .spacing(3.0)
                .padding(2.0)
                .align_y(Alignment::Center);

            children.push(Element::new(column![rule::horizontal(2), row].spacing(2)))
        }

        Column::with_children(children)
            .width(Length::Fixed(95.0))
            .align_x(Alignment::Center)
            .spacing(5.0)
            .into()
    }
}

impl AudioPage for Configuration {
    fn icon(&self) -> &'static str {
        "mic"
    }

    fn on_open(&mut self, state: &mut AudioState) {
        self.equaliser.load_device(state);

        let location = state.location();
        let bus_addr = location.bus_id.parse::<u8>().unwrap_or(0);
        let dev_addr = location.device_address;
        let nodes = find_pipewire_nodes_for_usb(bus_addr, dev_addr);

        let expected_source_channels = match state.device_definition.device_type {
            DeviceType::BeacnMic => 4,
            DeviceType::BeacnStudio => 12,
            _ => unreachable!(),
        };

        let expected_sink_channels = match state.device_definition.device_type {
            DeviceType::BeacnMic => 3,
            DeviceType::BeacnStudio => 11,
            _ => unreachable!(),
        };

        let mut spectrum_port = None;
        let mut dry_mix_port = None;
        let mut loopback_port = None;

        if let Ok(nodes) = nodes {
            // We found something, we need to find the mic node
            for node in nodes {
                // Immediately ignore UCM child nodes, they'll never contain what we need.
                if node.is_split_child {
                    continue;
                }

                // It should be noted that prior to pipewire 1.4 none of these ports are visible
                // as it uses dsnoop in alsa to build the UCM profiles. Post 1.4 they're available
                // on an internal node, which we can grab, so this won't work on older versions.
                if node.node_type == PipeWireNodeType::Sink {
                    if node.channels.len() == expected_sink_channels {
                        for port in node.ports {
                            if port.name == "AUX2" && port.port_type == PipeWirePortType::Input {
                                loopback_port.replace(port.id);
                            }
                        }
                    }
                } else {
                    // AUX2 is the Dry Mix, and AUX3 is the Post-Expander Dry Mix. These ports are not
                    // available on a Beacn Mic in compliancy mode.
                    if node.channels.len() == expected_source_channels {
                        if let Some(port) = node.channels.get("AUX2") {
                            dry_mix_port.replace(*port);
                        }
                        if let Some(port) = node.channels.get("AUX3") {
                            spectrum_port.replace(vec![*port]);
                        }
                    }
                }
            }
        }

        if let Some(dry_mix_port) = dry_mix_port
            && let Some(loopback_port) = loopback_port
            && self.loopback_handler.is_none()
        {
            self.loopback_handler = Some(LoopbackHandler::new(dry_mix_port, loopback_port));
        }

        if let Some(spectrum_ports) = spectrum_port
            && self.spectrum_handler.is_none()
        {
            // Ok, we have a usable port list, let's fire up a listener..
            let handler = start_spectrum_analyser(spectrum_ports, 48000);

            // Get the internal Spectrum Data. We only use a single port here, so grab the only entry.
            self.spectrum_data = Some(handler.data[0].clone());
            self.spectrum_handler = Some(handler);
        }

        // Open the active tab
        self.tab_pages[self.selected_tab].on_open(state);
    }

    fn on_close(&mut self, state: &mut AudioState) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
        }

        if let Some(mut handler) = self.loopback_handler.take() {
            handler.stop();
            handler.clear_buffer();

            let msg = Message::Headphones(Headphones::MicFromLoopback(false));
            let _ = state.handle_message(msg);
        }

        // Remove anything that may be cached, we should redraw later.
        self.equaliser.clear();

        // Close the active tab
        self.tab_pages[self.selected_tab].on_close(state);
    }

    fn on_tick(&mut self, state: &mut AudioState) -> Task<PageMessage> {
        // Ok, let's try and feed compressor data :D
        let message = BulkMessage::GetMeters;
        if let Ok(meters) = state.handle_bulk_message(message)
            && let BulkMessage::Meters(response) = meters
        {
            // Send a message to the active config page, notifying it that we have some new
            // meter data. Not all pages use this, but we do, so we always need it.
            let msg = ChildMessage::Meters(response);
            let _ = self.tab_pages[self.selected_tab].update(state, msg);

            // Push the fresh raw level into the ballistics; it tracks its own timing
            // internally and produces a smoothed value + decaying peak line.
            self.meter_ballistics.advance(response.processed_mic);
        }

        // Send a frame tick to the child, in case it needs anything.
        let msg = ChildMessage::OnTick;
        let _ = self.tab_pages[self.selected_tab].update(state, msg);

        let Some(handler) = self.spectrum_handler.as_mut() else {
            return Task::none();
        };

        if handler.has_stopped() {
            self.spectrum_handler = None;
            self.spectrum_data = None;

            self.equaliser.clear_spectrum_data();
            return Task::none();
        }

        // This shouldn't happen if the handler is active, but just in case..
        let Some(data) = self.spectrum_data.as_mut() else {
            self.spectrum_handler = None;
            self.spectrum_data = None;

            self.equaliser.clear_spectrum_data();
            return Task::none();
        };

        if let Ok(guard) = data.lock() {
            self.equaliser.set_spectrum_data(guard.clone());
        }
        Task::none()
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        match message {
            PageMessage::AudioConfig(msg) => match msg {
                ConfigMessage::Equaliser(event) => self
                    .equaliser
                    .update(state, event)
                    .map(ConfigMessage::Equaliser)
                    .map(PageMessage::AudioConfig),

                ConfigMessage::SelectTab(tab_index) => {
                    self.tab_pages[self.selected_tab].on_close(state);
                    self.selected_tab = tab_index;
                    self.tab_pages[self.selected_tab].on_open(state);

                    Task::none()
                }

                ConfigMessage::OutputGainChanged(gain) => {
                    let msg = Headphones::MicOutputGain(HPMicOutputGain(gain));
                    let msg = Message::Headphones(msg);
                    let _ = state.handle_message(msg);

                    Task::none()
                }

                // These are messages intended to go to device
                ConfigMessage::Child(ChildMessage::State(msg)) => {
                    let _ = state.handle_message(msg);
                    Task::none()
                }

                ConfigMessage::Child(child_msg) => {
                    match self.tab_pages.get_mut(self.selected_tab) {
                        Some(page) => page
                            .update(state, child_msg)
                            .map(ConfigMessage::Child)
                            .map(PageMessage::AudioConfig),

                        None => Task::none(),
                    }
                }

                ConfigMessage::HandleRecording => {
                    // Pretty simple, this can't trigger unless we're either stopped or recording,
                    // so just do whatever the opposite of that is :)
                    if let Some(handler) = self.loopback_handler.as_mut() {
                        if handler.state() != LoopbackHandlerState::Stopped {
                            handler.stop();
                        } else {
                            handler.perform_record();
                        }
                    }
                    Task::none()
                }

                ConfigMessage::HandlePlayback => {
                    // Same as above, except with playback.
                    if let Some(handler) = self.loopback_handler.as_mut() {
                        if handler.state() != LoopbackHandlerState::Stopped {
                            handler.stop();

                            let msg = Message::Headphones(Headphones::MicFromLoopback(false));
                            let _ = state.handle_message(msg);
                        } else {
                            let msg = Message::Headphones(Headphones::MicFromLoopback(true));
                            let _ = state.handle_message(msg);

                            handler.perform_playback();
                        }
                    }
                    Task::none()
                }
            },

            _ => Task::none(),
        }
    }

    fn view(&self, state: &AudioState) -> Element<'_, PageMessage> {
        let equaliser = self
            .equaliser
            .view(state)
            .map(ConfigMessage::Equaliser)
            .map(PageMessage::AudioConfig);

        let controls = self
            .equaliser
            .eq_controls(state)
            .map(ConfigMessage::Equaliser)
            .map(PageMessage::AudioConfig);

        let bottom = self.bottom_view(state).map(PageMessage::AudioConfig);
        row![
            column![
                // Remaining space
                container(equaliser)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(Padding {
                        right: 6.0,
                        ..Default::default()
                    }),
                container(controls)
                    .width(Length::Fill)
                    .height(Length::Fixed(33.0))
                    .padding(Padding {
                        top: 0.0,
                        right: 0.0,
                        bottom: 5.0,
                        left: 0.0,
                    }),
                rule::horizontal(5),
                // Fixed bottom section
                container(bottom)
                    .width(Length::Fill)
                    .height(Length::Fixed(240.0)),
            ],
            rule::vertical(2),
            self.gain_view(state).map(PageMessage::AudioConfig)
        ]
        .into()
    }
}

// This will be called when the device goes away.
impl Drop for Configuration {
    fn drop(&mut self) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
        }

        if let Some(mut handler) = self.loopback_handler.take() {
            handler.stop();
        }
    }
}

/// Owns the meter's attack/release/peak-hold state and timing, entirely
/// self-contained. Feed it raw dB readings via `advance()` whenever they
/// arrive; it tracks real elapsed time internally and produces a smoothed
/// `db` value plus an independently-decaying `peak_db` value, suitable for
/// driving a VU-style meter draw without any "stabby" jumpiness.
struct MeterBallistics {
    db: f32,
    peak_db: f32,
    peak_hold_timer: f32,
    last_update: Instant,

    attack_tau: f32,
    release_rate: f32,
    peak_hold_time: f32,
    peak_release_rate: f32,
}

impl MeterBallistics {
    fn new(floor_db: f32) -> Self {
        Self {
            db: floor_db,
            peak_db: floor_db,
            peak_hold_timer: 0.0,
            last_update: Instant::now(),

            attack_tau: 0.03,
            release_rate: 90.0,      // dB/s fall rate
            peak_hold_time: 0.2,     // Seconds until the peak starts to fall
            peak_release_rate: 90.0, // dB/sec peak fall rate
        }
    }

    fn advance(&mut self, target_db: f32) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32().min(0.1);
        self.last_update = now;

        if target_db > self.db {
            let alpha = 1.0 - (-dt / self.attack_tau).exp();
            self.db += (target_db - self.db) * alpha;
        } else {
            self.db = (self.db - self.release_rate * dt).max(target_db);
        }

        if target_db >= self.peak_db {
            self.peak_db = target_db;
            self.peak_hold_timer = self.peak_hold_time;
        } else if self.peak_hold_timer > 0.0 {
            self.peak_hold_timer -= dt;
        } else {
            self.peak_db = (self.peak_db - self.peak_release_rate * dt).max(target_db);
        }
    }
}

struct MicMeter {
    db: f32,
    peak_db: f32,
    range_db: (f32, f32),
}

impl MicMeter {
    /// Fraction in [0,1] of the track height that a dB value covers.
    fn magnitude_fraction(&self, db: f32) -> f32 {
        let (min_db, max_db) = self.range_db;
        let span = (max_db - min_db).abs().max(f32::EPSILON);
        ((db - min_db) / span).clamp(0.0, 1.0)
    }

    /// Vertical pixel position for a specific DB value
    fn y_for_db(&self, db: f32, track_top: f32, track_height: f32) -> f32 {
        track_top + track_height * (1.0 - self.magnitude_fraction(db))
    }
}

impl<Message> canvas::Program<Message> for MicMeter {
    type State = ();

    fn draw(
        &self,
        _: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();

        // Track background
        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.size().width, bounds.size().height),
            palette.background.strong.color,
        );

        // 'Peak' Area
        let peak = Color {
            a: 0.4,
            ..palette.warning.weak.color
        };
        let peak_bottom = self.y_for_db(-5.0, 0.0, bounds.size().height) - 2.0;
        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.size().width, peak_bottom),
            peak,
        );

        // 'Good' Area
        let good = Color {
            a: 0.4,
            ..palette.success.weak.color
        };
        let good_bottom = self.y_for_db(-15.0, 0.0, bounds.size().height);
        frame.fill_rectangle(
            Point::new(0.0, peak_bottom + 2.0),
            Size::new(bounds.size().width, good_bottom - peak_bottom),
            good,
        );

        let anchor_y = bounds.size().height;
        let value_y = self.y_for_db(self.db, 0.0, bounds.size().height);
        let (fill_y, fill_height) = if value_y <= anchor_y {
            (value_y, anchor_y - value_y)
        } else {
            (anchor_y, value_y - anchor_y)
        };
        frame.fill_rectangle(
            Point::new(0.0, fill_y),
            Size::new(bounds.size().width, fill_height),
            palette.primary.weak.color,
        );

        // Peak-hold indicator line
        const PEAK_LINE_HEIGHT: f32 = 2.0;
        if self.peak_db > self.range_db.0 {
            let peak_y = self.y_for_db(self.peak_db, 0.0, bounds.size().height);
            frame.fill_rectangle(
                Point::new(0.0, (peak_y - PEAK_LINE_HEIGHT).max(0.0)),
                Size::new(bounds.size().width, PEAK_LINE_HEIGHT),
                palette.primary.strong.color,
            );
        }

        vec![frame.into_geometry()]
    }
}
