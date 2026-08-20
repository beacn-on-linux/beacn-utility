use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::hp_equaliser::HPEQMessage::*;
use crate::ui::pages::audio::hp_equaliser::HPEQValue::*;
use crate::ui::pages::page::{AudioPage, PageMessage};
use crate::ui::utility::pipewire::{
    PipeWireNodeType, SpectrumHandle, find_pipewire_nodes_for_usb, start_spectrum_analyser,
};
use crate::ui::widgets::equaliser::eq_common::{
    EqGeometry, MAX_FREQUENCY, MAX_GAIN, MIN_FREQUENCY, MIN_GAIN, band_type_has_gain, get_q_delta,
};
use crate::ui::widgets::equaliser::eq_drawer::{EQDrawView, EQMouseEvent};
use crate::ui::widgets::helpers::buttons::padded_button;
use crate::ui::widgets::helpers::drag_value::styled_drag_value;
use crate::ui::widgets::helpers::slider::{slider_theme, themed_slider};
use crate::ui::widgets::helpers::svg::{svg_button, svg_button_style, svg_button_unstyled};
use beacn_lib::EQ_HEADPHONES_VERSION;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::controls::Controls;
use beacn_lib::audio::messages::eq_common::{EQBand, EQBandType, EQFrequency, EQGain, EQQ};
use beacn_lib::audio::messages::eq_headphones::EQChannel as Channel;
use beacn_lib::audio::messages::eq_headphones::EQHeadphones;
use beacn_lib::audio::messages::headphones::{HPLevel, HPMicMonitorLevel, Headphones};
use beacn_lib::audio::messages::subwoofer::{Subwoofer, SubwooferAmount};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::HasRange;
use enum_map::{EnumMap, enum_map};
use iced::border::Radius;
use iced::font::Weight;
use iced::mouse::ScrollDelta;
use iced::widget::{Canvas, Space, checkbox, column, container, row, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Point, Task};
use log::warn;
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;

// Stolen from mic_equaliser
const DRAG_DELAY: Duration = Duration::from_millis(80);

#[derive(Debug, Clone)]
pub enum HPEQMessage {
    Equaliser(Channel, EQMouseEvent),

    // EQ UI Elements
    EqualiserValue(HPEQValue),

    AddBand,
    RemoveBand,

    Balance(i8),
    SubWoofer(u8),
    Stereo(bool),
    ToggleLinked,

    // Direct State Change
    State(Message),
}

// We isolate these because they all basically do the same thing but with a different field,
// so we can wrap up stuff for ease of consistency.
#[derive(Debug, Clone)]
pub enum HPEQValue {
    Type(EQBandType),
    Frequency(u32),
    Gain(f32),
    Q(f32),
}

pub struct HPEqualiser {
    // Just one equaliser, although we need two..
    view: EnumMap<Channel, EQDrawView>,
    spectrum_handler: Option<SpectrumHandle>,
    spectrum_data: EnumMap<Channel, Option<Arc<Mutex<Vec<f32>>>>>,

    // Temporary data so we can test interactions. The state will eventually feed this
    //temp: EnumMap<Channel, Bands>,

    // These are internal to this page
    active_channel: Channel,
    active_band: Option<EQBand>,

    // Time in which a drag was initiated, if any.
    drag_start: Option<Instant>,
}

impl HPEqualiser {
    pub fn new() -> Self {
        Self {
            view: Default::default(),
            //temp: Default::default(),
            spectrum_handler: None,
            spectrum_data: Default::default(),

            active_channel: Channel::Left,
            active_band: None,
            drag_start: None,
        }
    }

    fn load_temp_data(&mut self, state: &AudioState) {
        for channel in Channel::iter() {
            self.view[channel].set_bands(state.eq_headphones.bands[channel]);
        }
        self.switch_active_channel_force(Channel::Left, state, true);
    }

    fn update(&mut self, state: &mut AudioState, msg: HPEQMessage) -> Task<HPEQMessage> {
        let is_linked = state.eq_headphones.linked;
        match msg {
            Equaliser(channel, event) => {
                self.handle_eq_event(state, channel, event);
            }

            EqualiserValue(value) => {
                let ch = self.active_channel;
                let ot = self.active_channel.other();

                // Let the commands fill this vec with messages, we'll send them to the handler,
                // then apply changes to the view at the end.
                let mut messages = vec![];
                if let Some(band) = self.active_band {
                    match value {
                        Type(band_type) => {
                            if let Some(band) = self.active_band {
                                let msg = EQHeadphones::Type(ch, band, band_type);
                                let msg = Message::EQHeadphones(msg);
                                messages.push(msg);

                                if is_linked {
                                    let msg = EQHeadphones::Type(ot, band, band_type);
                                    let msg = Message::EQHeadphones(msg);
                                    messages.push(msg);
                                }
                            }
                        }
                        Frequency(frequency) => {
                            if let Some(band) = self.active_band {
                                let msg = EQHeadphones::Frequency(ch, band, frequency.into());
                                let msg = Message::EQHeadphones(msg);
                                messages.push(msg);

                                if is_linked {
                                    let msg = EQHeadphones::Frequency(ot, band, frequency.into());
                                    let msg = Message::EQHeadphones(msg);
                                    messages.push(msg);
                                }
                            }
                        }
                        Gain(gain) => {
                            if let Some(band) = self.active_band {
                                let msg = EQHeadphones::Gain(ch, band, gain.into());
                                let msg = Message::EQHeadphones(msg);
                                messages.push(msg);

                                if is_linked {
                                    let msg = EQHeadphones::Gain(ot, band, gain.into());
                                    let msg = Message::EQHeadphones(msg);
                                    messages.push(msg);
                                }
                            }
                        }
                        Q(q) => {
                            if let Some(band) = self.active_band {
                                let msg = EQHeadphones::Q(ch, band, q.into());
                                let msg = Message::EQHeadphones(msg);
                                messages.push(msg);

                                if is_linked {
                                    let msg = EQHeadphones::Q(ot, band, q.into());
                                    let msg = Message::EQHeadphones(msg);
                                    messages.push(msg);
                                }
                            }
                        }
                    }
                    for message in messages {
                        let _ = state.handle_message(message);
                    }

                    self.view[ch].set_band(band, state.eq_headphones.bands[ch][band]);
                    if is_linked {
                        self.view[ot].set_band(band, state.eq_headphones.bands[ot][band]);
                    }
                }
            }

            AddBand => {
                // This is kinda awkward, but here we go :)
                let ch = self.active_channel;
                let ot = ch.other();

                // We just need to find the first disabled band..
                let found = state.eq_headphones.bands[ch]
                    .iter()
                    .find(|(_, b)| !b.enabled)
                    .map(|(band, _)| band);

                // Update and Send it, this will probably be easier with a real state!
                if let Some(band) = found {
                    // There could be up to 4 message state changes, so collect and send later.
                    let mut messages = vec![];
                    if state.eq_headphones.bands[ch][band].band_type == EQBandType::NotSet {
                        warn!("EQ Band doesn't have type set, defaulting to BellBand");
                        let msg = EQHeadphones::Type(ch, band, EQBandType::BellBand);
                        let msg = Message::EQHeadphones(msg);
                        messages.push(msg);

                        if is_linked {
                            let msg = EQHeadphones::Type(ot, band, EQBandType::BellBand);
                            let msg = Message::EQHeadphones(msg);
                            messages.push(msg);
                        }
                    }

                    // Enable the band
                    let msg = EQHeadphones::Enabled(ch, band, true);
                    let msg = Message::EQHeadphones(msg);
                    messages.push(msg);

                    if is_linked {
                        let msg = EQHeadphones::Enabled(ot, band, true);
                        let msg = Message::EQHeadphones(msg);
                        messages.push(msg);
                    }

                    // Send and update the state
                    for message in messages {
                        let _ = state.handle_message(message);
                    }

                    // Update the views
                    self.active_band = Some(band);
                    self.view[ch].set_active(Some(band));
                    self.view[ch].set_band(band, state.eq_headphones.bands[ch][band]);
                    if is_linked {
                        self.view[ot].set_active(Some(band));
                        self.view[ot].set_band(band, state.eq_headphones.bands[ot][band]);
                    }
                }
            }
            RemoveBand => {
                let ch = self.active_channel;
                let ot = ch.other();

                if let Some(band) = self.active_band {
                    let msg = EQHeadphones::Enabled(ch, band, false);
                    let msg = Message::EQHeadphones(msg);
                    let _ = state.handle_message(msg);

                    self.view[ch].set_band(band, state.eq_headphones.bands[ch][band]);
                    if is_linked {
                        let msg = EQHeadphones::Enabled(ot, band, false);
                        let msg = Message::EQHeadphones(msg);
                        let _ = state.handle_message(msg);

                        self.view[ot].set_band(band, state.eq_headphones.bands[ot][band]);
                    }

                    // Try and find a new active band on this channel.
                    self.active_band = None;
                    for band in EQBand::iter().rev() {
                        if state.eq_headphones.bands[ch][band].enabled {
                            self.active_band = Some(band);
                            break;
                        }
                    }

                    let active_band = self.active_band;
                    self.view[ch].set_active(active_band);
                    if is_linked {
                        self.view[ot].set_active(active_band);
                    }
                }
            }

            Balance(amount) => {
                let msg = Controls::Balance(amount.into());
                let msg = Message::Controls(msg);
                let _ = state.handle_message(msg);
            }

            SubWoofer(amount) => {
                let version = state.device_definition.device_info.version;

                // Subwoofer is fun, get various messages based on amount
                let messages = Subwoofer::get_amount_messages(amount, version);
                for message in messages {
                    let _ = state.handle_message(message);
                }
            }

            Stereo(enabled) => {
                // The checkbox is flipped, so we need to unflip it here
                let msg = Controls::Mono(!enabled);
                let msg = Message::Controls(msg);
                let _ = state.handle_message(msg);
            }

            ToggleLinked => {
                self.toggle_linked(state);
            }

            State(msg) => {
                let _ = state.handle_message(msg);
            }
        }

        Task::none()
    }

    fn handle_eq_event(&mut self, state: &mut AudioState, channel: Channel, event: EQMouseEvent) {
        match event {
            EQMouseEvent::Pressed(e) => self.handle_eq_press(state, channel, e),
            EQMouseEvent::Moved(e) => self.handle_eq_moved(state, channel, e),
            EQMouseEvent::Released => self.handle_eq_released(state, channel),
            EQMouseEvent::Scrolled(p, d) => self.handle_eq_scrolled(state, channel, p, d),
        }
    }

    fn handle_eq_press(&mut self, state: &mut AudioState, channel: Channel, point: Point) {
        // Do we have a hit on a band?
        if self.check_band_hit(channel, state, point).is_some() {
            // Prepare for a drag, will be taken care of during the handle_eq_move callback
            self.view[channel].set_track_motion(true);
            self.drag_start = Some(Instant::now());
        }
    }
    fn handle_eq_moved(&mut self, state: &mut AudioState, ch: Channel, point: Point) {
        let is_linked = state.eq_headphones.linked;
        let ot = ch.other();
        if self.drag_start.is_some_and(|t| t.elapsed() >= DRAG_DELAY) {
            // We're dragging the active band, so should be updating its state
            let Some(band) = self.active_band else {
                return;
            };

            let mut messages = vec![];
            let plot = self.view[ch].plot_rect();

            // Grab the Frequency..
            let frequency = EqGeometry::x_to_freq(point.x, plot);
            let frequency = frequency.clamp(MIN_FREQUENCY as f32, MAX_FREQUENCY as f32);

            let msg = EQHeadphones::Frequency(ch, band, frequency.into());
            let msg = Message::EQHeadphones(msg);
            messages.push(msg);

            if is_linked {
                let msg = EQHeadphones::Frequency(ch.other(), band, frequency.into());
                let msg = Message::EQHeadphones(msg);
                messages.push(msg);
            }

            // Set the Gain..
            if band_type_has_gain(state.eq_headphones.bands[ch][band].band_type) {
                let gain = EqGeometry::y_to_db(point.y, plot).clamp(MIN_GAIN, MAX_GAIN);
                let gain = (gain * 10.0).round() / 10.0;

                let msg = EQHeadphones::Gain(ch, band, gain.into());
                let msg = Message::EQHeadphones(msg);
                messages.push(msg);

                if is_linked {
                    let msg = EQHeadphones::Gain(ch.other(), band, gain.into());
                    let msg = Message::EQHeadphones(msg);
                    messages.push(msg);
                }
            }

            for message in messages {
                let _ = state.handle_message(message);
            }

            self.view[ch].set_band(band, state.eq_headphones.bands[ch][band]);
            if is_linked {
                self.view[ot].set_band(band, state.eq_headphones.bands[ot][band]);
            }
        }
    }
    fn handle_eq_released(&mut self, _state: &AudioState, channel: Channel) {
        self.view[channel].set_track_motion(false);
        self.drag_start = None;
    }
    fn handle_eq_scrolled(&mut self, state: &mut AudioState, c: Channel, p: Point, d: ScrollDelta) {
        let (ch, point, delta) = (c, p, d);
        let is_linked = state.eq_headphones.linked;
        let ot = ch.other();

        if let Some(band) = self.check_band_hit(ch, state, point) {
            let delta = get_q_delta(delta);

            let mut messages = vec![];

            let q = state.eq_headphones.bands[ch][band].q;
            let adjusted = (q + delta).clamp(0.1, 10.0);
            let adjusted = (adjusted * 10.0).round() / 10.0;

            let msg = EQHeadphones::Q(ch, band, adjusted.into());
            let msg = Message::EQHeadphones(msg);
            messages.push(msg);

            if is_linked {
                let msg = EQHeadphones::Q(ch.other(), band, adjusted.into());
                let msg = Message::EQHeadphones(msg);
                messages.push(msg);
            }

            for message in messages {
                let _ = state.handle_message(message);
            }

            self.view[ch].set_band(band, state.eq_headphones.bands[ch][band]);
            if is_linked {
                self.view[ot].set_band(band, state.eq_headphones.bands[ot][band]);
            }
        }
    }

    // Checks for a band hit, if found, updates the channel and active band, then returns the band.
    fn check_band_hit(&mut self, c: Channel, s: &AudioState, p: Point) -> Option<EQBand> {
        let (channel, state, point) = (c, s, p);
        let is_linked = state.eq_headphones.linked;
        let bands = &state.eq_headphones.bands[channel];

        let rect = self.view[c].plot_rect();
        if let Some(selected_band) = EqGeometry::hit_test(rect, point, bands) {
            self.switch_active_channel(c, state);

            self.active_band = Some(selected_band);
            self.view[c].set_active(Some(selected_band));
            if is_linked {
                // If we're linked, update the other channel as well.
                self.view[c.other()].set_active(Some(selected_band));
            }

            return Some(selected_band);
        }
        None
    }

    // When we become linked, outside of syncing both channels, we also need a couple of display
    // tweaks. Firstly, we disable the green border around the active channel, then we enable
    // a matching active band on the other channel.
    fn toggle_linked(&mut self, state: &mut AudioState) {
        let set_linked = !state.eq_headphones.linked;
        let msg = EQHeadphones::Linked(set_linked);
        let msg = Message::EQHeadphones(msg);
        let _ = state.handle_message(msg);

        let is_linked = state.eq_headphones.linked;
        if is_linked {
            self.view[self.active_channel].set_border_colour(None);
            self.view[self.active_channel.other()].set_active(self.active_band);
        } else {
            self.view[self.active_channel].set_border_colour(Some(Color::from_rgb8(0, 255, 0)));
            self.view[self.active_channel.other()].set_active(None);
        }
    }

    fn switch_active_channel(&mut self, channel: Channel, state: &AudioState) {
        self.switch_active_channel_force(channel, state, false);
    }

    // The force param will result in the border being cleared then reloaded, along with the
    // active band.
    fn switch_active_channel_force(&mut self, ch: Channel, state: &AudioState, force: bool) {
        if self.active_channel == ch && !force {
            return;
        }

        let is_linked = state.eq_headphones.linked;

        // Clear the Active Band and the Border Colour from the existing channel
        self.view[self.active_channel].set_active(None);
        self.view[self.active_channel].set_border_colour(None);

        // The active band is likely about to be replaced by the caller (normally a mouse down, or
        // scroll event are the things that cause a channel switch), but that'll happen after a hit
        // on a visible band point.
        //
        // With that said, safety first. Make sure a band is selected by default.

        // Check whether the current active band is available on the new channel
        let find_new_band = match self.active_band {
            Some(band) => state.eq_headphones.bands[ch][band].enabled,
            None => true,
        };

        // If it is, set the new active band, otherwise use the current active band
        let active_band = match find_new_band {
            true => EQBand::iter().find(|&b| state.eq_headphones.bands[ch][b].enabled),
            false => self.active_band,
        };

        self.active_channel = ch;
        self.active_band = active_band;
        self.view[ch].set_active(active_band);

        // Only update the border if we're not linked
        if !is_linked {
            self.view[ch].set_border_colour(Some(Color::from_rgb8(0, 255, 0)));
        }
    }

    fn view(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        column![
            self.add_eq_canvas(Channel::Left, state),
            container(self.add_controls(state)).height(Length::Fixed(80.0)),
            self.add_eq_canvas(Channel::Right, state),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(6.0)
        .padding(10.0)
        .into()
    }

    // Canvas
    fn add_eq_canvas(&self, channel: Channel, state: &AudioState) -> Element<'_, HPEQMessage> {
        // Lets grab the base canvas from the view..
        let canvas: Element<'_, EQMouseEvent> = Element::from(
            Canvas::new(&self.view[channel])
                .width(Length::Fill)
                .height(Length::Fill),
        );

        let canvas = match channel {
            Channel::Left => canvas.map(|m| Equaliser(Channel::Left, m)),
            Channel::Right => canvas.map(|m| Equaliser(Channel::Right, m)),
        };

        let label = match channel {
            Channel::Left => "LEFT",
            Channel::Right => "RIGHT",
        };

        // Should depend on whether self.active = active
        let is_linked = state.eq_headphones.linked;
        let label_colour = match self.active_channel == channel && !is_linked {
            true => Color::from_rgba8(0, 255, 0, 1.0),
            false => Color::from_rgba8(255, 255, 255, 0.5),
        };

        let label_font = Font {
            weight: Weight::Bold,
            ..Default::default()
        };
        let container_padding = Padding {
            top: 0.0,
            bottom: 10.0,
            left: 0.0,
            right: 5.0,
        };

        let overlay = container(text(label).size(20).color(label_colour).font(label_font))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(container_padding)
            .align_x(Alignment::End)
            .align_y(Alignment::End);

        // We should just need to create the EQ canvas, then stack![] them.
        stack![canvas, overlay].into()
    }

    // Controls
    fn add_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        row![
            Space::new().width(16),
            Self::panel(self.volume_controls(state)),
            Self::panel(self.balance_sub_controls(state)),
            Self::panel(self.mono_stereo_controls(state)),
            Self::panel(self.equaliser_controls(state)),
            Space::new().width(Length::Fill),
            self.link_control(state),
        ]
        .align_y(Alignment::Center)
        .spacing(6)
        .into()
    }

    fn volume_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        let device_type = state.device_definition.device_type;
        let value = state.headphones.mic_monitor;
        let range = HPMicMonitorLevel::range();
        let monitor = themed_slider(range, value, move |v| {
            let command = match device_type {
                DeviceType::BeacnMic => Headphones::MicMonitor(HPMicMonitorLevel(v)),
                DeviceType::BeacnStudio => Headphones::StudioMicMonitor(HPMicMonitorLevel(v)),
                _ => unreachable!(),
            };

            let msg = Message::Headphones(command);
            State(msg)
        });

        let value = state.headphones.level;
        let range = HPLevel::range();
        let level = themed_slider(range, value, |v| {
            let msg = Message::Headphones(Headphones::HeadphoneLevel(HPLevel(v)));
            State(msg)
        });

        column![
            text("Headphone Level").size(11.0),
            level,
            Space::new().height(7),
            text("Mic Monitor").size(11.0),
            monitor,
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(120.0))
        .into()
    }

    fn balance_sub_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        let balance = state.controls.balance;

        let balance_at_zero = balance == 0;
        let balance_slider = themed_slider(-100..=100, balance, Balance)
            .style(move |theme, status| {
                let mut style = slider_theme(theme, status);
                if balance_at_zero {
                    style.handle.border_color = Color::from_rgb(0.0, 1.0, 0.0);
                }
                style
            })
            .trail_start(0);

        let value = state.subwoofer.amount;
        let range = SubwooferAmount::range();
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let woofer = themed_slider(range, value, SubWoofer);
        column![
            text("Balance").size(11.0),
            balance_slider,
            Space::new().height(7),
            text("Subwoofer").size(11.0),
            woofer,
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(120.0))
        .into()
    }

    fn mono_stereo_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        // TODO: Need a Stereo / Mono Icon!

        column![
            text("Stereo"),
            Space::new().height(14),
            checkbox(!state.controls.mono).on_toggle(Stereo),
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(80.0))
        .into()
    }

    fn equaliser_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        let channel = self.active_channel;
        let bands = &state.eq_headphones.bands[channel];

        let add_band = bands.values().any(|b| !b.enabled).then_some(AddBand);
        let remove_band = self.active_band.map(|_| RemoveBand);

        let add_button = padded_button("Add Band", Alignment::Center)
            .width(Length::Fixed(120.0))
            .on_press_maybe(add_band);

        let remove_button = padded_button("Remove Band", Alignment::Center)
            .width(Length::Fixed(120.0))
            .on_press_maybe(remove_band);

        let buttons = column![
            add_button,
            container(text("")).height(Length::Fixed(12.0)),
            remove_button
        ]
        .align_x(Alignment::Center);

        let type_grid = self.eq_type_grid(state).map(EqualiserValue);
        let values = self.eq_values(state).map(EqualiserValue);

        row![buttons, type_grid, values]
            .spacing(10.0)
            .align_y(Alignment::Center)
            .into()
    }

    fn eq_type_grid(&self, state: &AudioState) -> Element<'_, HPEQValue> {
        let Some(active) = self.active_band else {
            return row![].into();
        };

        let current_type = state.eq_headphones.bands[self.active_channel][active].band_type;
        let types: [(&'static str, EQBandType); 6] = [
            ("eq_low_pass", EQBandType::LowPassFilter),
            ("eq_high_pass", EQBandType::HighPassFilter),
            ("eq_notch", EQBandType::NotchFilter),
            ("eq_bell", EQBandType::BellBand),
            ("eq_low_shelf", EQBandType::LowShelf),
            ("eq_high_shelf", EQBandType::HighShelf),
        ];

        let mut top = row![].spacing(1.0);
        let mut bottom = row![].spacing(1.0);

        for (index, (name, band_type)) in types.into_iter().enumerate() {
            let selected = current_type == band_type;
            let is_first_row = index < 3;
            let col = index % 3;

            let button = svg_button(name, selected)
                .width(Length::Fixed(45.0))
                .on_press(Type(band_type));

            let button = button.style(move |theme, status| {
                let mut style = svg_button_style(theme, status, selected);
                style.border.radius = Radius {
                    top_left: if is_first_row && col == 0 { 6.0 } else { 0.0 },
                    top_right: if is_first_row && col == 2 { 6.0 } else { 0.0 },
                    bottom_right: if !is_first_row && col == 2 { 6.0 } else { 0.0 },
                    bottom_left: if !is_first_row && col == 0 { 6.0 } else { 0.0 },
                };
                style
            });

            if is_first_row {
                top = top.push(button);
            } else {
                bottom = bottom.push(button);
            }
        }

        column![top, bottom].spacing(1.0).into()
    }

    fn eq_values(&self, state: &AudioState) -> Element<'_, HPEQValue> {
        let Some(active) = self.active_band else {
            return row![].into();
        };

        // Clone the band so we can extract values
        let band = state.eq_headphones.bands[self.active_channel][active];

        let freq_r = EQFrequency::range();
        let freq_r: RangeInclusive<u32> = (*freq_r.start() as u32)..=(*freq_r.end() as u32);
        let frequency = styled_drag_value(band.frequency, freq_r)
            .suffix("Hz")
            .on_change(Frequency)
            .width(Length::Fixed(100.0));

        let has_gain = band_type_has_gain(band.band_type);
        let gain_value = if has_gain { band.gain } else { 0.0 };
        let gain_range = EQGain::range();
        let gain = styled_drag_value(gain_value, gain_range)
            .suffix("dB")
            .on_change_maybe(has_gain.then_some(Gain))
            .width(Length::Fixed(100.0));

        let q_range = EQQ::range();
        let q = styled_drag_value(band.q, q_range)
            .on_change(Q)
            .width(Length::Fixed(100.0));

        column![
            row![text("Freq").width(Length::Fixed(34.0)), frequency]
                .spacing(4.0)
                .align_y(Alignment::Center),
            row![text("Gain").width(Length::Fixed(34.0)), gain]
                .spacing(4.0)
                .align_y(Alignment::Center),
            row![text("Q").width(Length::Fixed(34.0)), q]
                .spacing(4.0)
                .align_y(Alignment::Center),
        ]
        .spacing(4.0)
        .into()
    }

    fn link_control(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        let is_linked = state.eq_headphones.linked;

        let name = if is_linked { "unlink" } else { "link" };
        svg_button_unstyled(name)
            .on_press(ToggleLinked)
            .width(Length::Fixed(30.0))
            .style(move |t, s| svg_button_style(t, s, is_linked))
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(60.0))
            .into()
    }

    // Small Helper stuff
    fn panel<'a>(content: impl Into<Element<'a, HPEQMessage>>) -> Element<'a, HPEQMessage> {
        container(content)
            .width(Length::Shrink)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgb8(35, 35, 35))),
                border: Border {
                    radius: Radius::from(5),
                    ..Default::default()
                },
                ..Default::default()
            })
            .padding(6)
            .into()
    }
}

impl AudioPage for HPEqualiser {
    fn icon(&self) -> &'static str {
        "headphones"
    }

    fn should_show(&self, state: &AudioState) -> bool {
        let version = state.device_definition.device_info.version;
        version > EQ_HEADPHONES_VERSION
    }

    fn on_open(&mut self, state: &AudioState) {
        self.load_temp_data(state);

        // This will look familiar, except this time we're pulling the stereo output rather than
        // the mono input :)
        let location = state.location();
        let bus_addr = location.bus_id.parse::<u8>().unwrap_or(0);
        let dev_addr = location.device_address;
        let nodes = find_pipewire_nodes_for_usb(bus_addr, dev_addr);

        let mut use_port = None;
        if let Ok(nodes) = nodes {
            for node in nodes {
                // Immediately ignore UCM child nodes and Sink nodes, they'll never contain what we need.
                if node.is_split_child || node.node_type != PipeWireNodeType::Sink {
                    continue;
                }

                match state.device_definition.device_type {
                    DeviceType::BeacnMic => {
                        // If we have 2 channels, we're in compliancy mode, so these are FL,FR
                        if node.channels.len() == 2
                            && let Some(left) = node.channels.get("FL")
                            && let Some(right) = node.channels.get("FR")
                        {
                            use_port.replace(vec![*left, *right]);
                            break;
                        }

                        // Otherwise, we're in UCM / Pro mode, so these are AUX0, AUX1
                        if node.channels.len() == 3
                            && let Some(left) = node.channels.get("AUX0")
                            && let Some(right) = node.channels.get("AUX1")
                        {
                            use_port.replace(vec![*left, *right]);
                            break;
                        }
                    }

                    // Beacn Studio can only ever have 11 channels, so this is easy.
                    DeviceType::BeacnStudio => {
                        if node.channels.len() == 11
                            && let Some(left) = node.channels.get("AUX0")
                            && let Some(right) = node.channels.get("AUX1")
                        {
                            use_port.replace(vec![*left, *right]);
                            break;
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }

        if let Some(ports) = use_port {
            // Ok, we have a usable port list, let's fire up a listener..
            let handler = start_spectrum_analyser(ports, 48000);

            // Get the internal Spectrum Data. We only use a single port here, so grab the only entry.
            self.spectrum_data = enum_map! {
                Channel::Left => Some(handler.data[0].clone()),
                Channel::Right => Some(handler.data[1].clone()),
            };
            self.spectrum_handler = Some(handler);
        }
    }

    fn on_close(&mut self) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
        }

        for channel in Channel::iter() {
            self.view[channel].clear();
        }
    }

    fn on_tick(&mut self, _state: &mut AudioState) -> Task<PageMessage> {
        let Some(handler) = self.spectrum_handler.as_mut() else {
            return Task::none();
        };

        if handler.has_stopped() {
            self.spectrum_handler = None;
            self.spectrum_data = Default::default();

            for channel in Channel::iter() {
                self.view[channel].clear_spectrum();
            }
            return Task::none();
        }

        // This shouldn't happen if the handler is active, but just in case..
        for channel in Channel::iter() {
            let Some(data) = self.spectrum_data[channel].as_mut() else {
                // Something has gone wrong, nuke everything.
                self.spectrum_handler = None;
                self.spectrum_data = Default::default();

                for channel in Channel::iter() {
                    self.view[channel].clear_spectrum();
                }
                return Task::none();
            };

            if let Ok(guard) = data.lock() {
                let spectrum = guard.clone();
                self.view[channel].set_spectrum(spectrum);
            }
        }

        Task::none()
    }

    fn update(&mut self, state: &mut AudioState, msg: PageMessage) -> Task<PageMessage> {
        if let PageMessage::AudioHPEqualiser(msg) = msg {
            return self.update(state, msg).map(PageMessage::AudioHPEqualiser);
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, PageMessage> {
        self.view(state).map(PageMessage::AudioHPEqualiser)
    }
}
