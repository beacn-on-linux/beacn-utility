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
use crate::ui::utility::pipewire::{
    PipeWireNodeType, SpectrumHandle, find_pipewire_nodes_for_usb, start_spectrum_analyser,
};
use crate::ui::widgets::helpers::composite::draw_range;
use crate::ui::widgets::helpers::tabs::render_tab;
use beacn_lib::audio::data::BulkMessage;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphones::{HPMicOutputGain, Headphones};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::HasRange;
use iced::widget::canvas::{Frame, Geometry};
use iced::widget::{Canvas, button, canvas, column, container, row, rule, text};
use iced::{
    Alignment, Element, Length, Padding, Point, Rectangle, Renderer, Size, Task, Theme, mouse,
};
use log::debug;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone)]
pub(crate) enum ConfigMessage {
    Equaliser(MicEqualiserEvent),
    Child(ChildMessage),
    SelectTab(usize),

    OutputGainChanged(f32),
}

pub struct Configuration {
    equaliser: MicEqualiser,
    spectrum_handler: Option<SpectrumHandle>,
    spectrum_data: Option<Arc<Mutex<Vec<f32>>>>,

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

            meter_ballistics: MeterBallistics::new(-70.0),

            selected_tab: 0,
            tab_pages: vec![
                Box::new(MicrophoneSetup),
                Box::new(SuppressorPage),
                Box::new(ExpanderPage),
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

        let meter = MicMeter {
            db: self.meter_ballistics.db,
            peak_db: self.meter_ballistics.peak_db,
            range_db: (-70.0, 0.0),
        };
        let canvas = Canvas::new(meter).height(Length::Fill).width(Length::Fill);

        let canvas_container = container(canvas)
            .width(Length::Fixed(40.0))
            .height(Length::FillPortion(60))
            .align_x(Alignment::Center)
            .padding(8);

        let gain = container(gain)
            .width(Length::Fixed(95.0))
            .height(Length::FillPortion(40))
            .align_x(Alignment::Center)
            .padding(8);

        column![canvas_container, gain]
            .align_x(Alignment::Center)
            .into()
    }
}

impl AudioPage for Configuration {
    fn icon(&self) -> &'static str {
        "mic"
    }

    fn on_open(&mut self, state: &AudioState) {
        self.equaliser.load_device(state);

        if self.spectrum_handler.is_some() {
            return;
        }

        let location = state.location();
        let bus_addr = location.bus_id.parse::<u8>().unwrap_or(0);
        let dev_addr = location.device_address;
        let nodes = find_pipewire_nodes_for_usb(bus_addr, dev_addr);

        let expected_channels = match state.device_definition.device_type {
            DeviceType::BeacnMic => 4,
            DeviceType::BeacnStudio => 12,
            _ => unreachable!(),
        };

        let mut use_port = None;
        if let Ok(nodes) = nodes {
            // We found something, we need to find the mic node
            for node in nodes {
                // Immediately ignore UCM child nodes, they'll never contain what we need.
                if node.is_split_child || node.node_type != PipeWireNodeType::Source {
                    continue;
                }

                debug!("Found node: {:?}", node);
                // AUX3 is the Dry Mix for the Mic on the Mic / Studio. We can only get this
                // in UCM mode if we find the internal 4-port source.
                if node.channels.len() == expected_channels
                    && let Some(port) = node.channels.get("AUX3")
                {
                    use_port.replace(vec![*port]);
                }
            }
        }

        if let Some(ports) = use_port {
            // Ok, we have a usable port list, let's fire up a listener..
            let handler = start_spectrum_analyser(ports, 48000);

            // Get the internal Spectrum Data. We only use a single port here, so grab the only entry.
            self.spectrum_data = Some(handler.data[0].clone());
            self.spectrum_handler = Some(handler);
        }
    }

    fn on_close(&mut self) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
        }

        // Remove anything that may be cached, we should redraw later.
        self.equaliser.clear();
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
                    self.selected_tab = tab_index;
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
