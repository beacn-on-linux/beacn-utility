use crate::devices::states::audio::{AudioState, EqualiserBand, EqualiserBandType};
use crate::ui::pages::audio::hp_equaliser::HPEQMessage::*;
use crate::ui::pages::audio::hp_equaliser::HPEQValue::*;

use crate::ui::pages::page::{AudioPage, PageMessage};
use crate::ui::widgets::equaliser::eq_common::{
    Bands, EqGeometry, MAX_FREQUENCY, MAX_GAIN, MIN_FREQUENCY, MIN_GAIN, band_type_has_gain,
    get_q_delta,
};
use crate::ui::widgets::equaliser::eq_drawer::{EQDrawView, EQMouseEvent};
use crate::ui::widgets::helpers::buttons::padded_button;
use crate::ui::widgets::helpers::drag_value::styled_drag_value;
use crate::ui::widgets::helpers::slider::{slider_theme, themed_slider};
use crate::ui::widgets::helpers::svg::{svg_button, svg_button_style, svg_button_unstyled};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::equaliser::{EQFrequency, EQGain, EQQ};
use beacn_lib::audio::messages::headphones::{HPLevel, HPMicMonitorLevel, Headphones};
use beacn_lib::audio::messages::subwoofer::{Subwoofer, SubwooferAmount};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::HasRange;
use enum_map::{Enum, EnumMap};
use iced::border::Radius;
use iced::font::Weight;
use iced::mouse::ScrollDelta;
use iced::widget::{Canvas, Space, checkbox, column, container, row, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Point, Task};
use log::warn;
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

// Stolen from mic_equaliser
const DRAG_DELAY: Duration = Duration::from_millis(80);

#[derive(Copy, Clone, Eq, PartialEq, Debug, Enum, EnumIter)]
pub(crate) enum Channel {
    Left,
    Right,
}
impl Channel {
    fn other(self) -> Self {
        match self {
            Channel::Left => Channel::Right,
            Channel::Right => Channel::Left,
        }
    }
}

#[derive(Debug, Clone)]
pub enum HPEQMessage {
    Equaliser(Channel, EQMouseEvent),

    // EQ UI Elements
    EqualiserValue(HPEQValue),

    AddBand,
    RemoveBand,

    Balance(i32),
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
    Type(EqualiserBandType),
    Frequency(u32),
    Gain(f32),
    Q(f32),
}

pub struct HPEqualiser {
    // Just one equaliser, although we need two..
    view: EnumMap<Channel, EQDrawView>,

    // Temporary data so we can test interactions. The state will eventually feed this
    temp: EnumMap<Channel, Bands>,

    // These are internal to this page
    active_channel: Channel,
    active_band: Option<EqualiserBand>,

    // Time in which a drag was initiated, if any.
    drag_start: Option<Instant>,

    // Needs to be fed from the state, currently unavailable.
    balance: i32,
    is_stereo: bool,
    is_linked: bool,
}

impl HPEqualiser {
    pub fn new() -> Self {
        Self {
            view: Default::default(),
            temp: Default::default(),

            active_channel: Channel::Left,
            active_band: None,
            drag_start: None,

            balance: 0,
            is_stereo: true,
            is_linked: false,
        }
    }

    fn load_temp_data(&mut self, state: &AudioState) {
        for channel in Channel::iter() {
            self.temp[channel] = state.equaliser.bands[state.equaliser.mode];
            self.view[channel].set_bands(self.temp[channel]);
        }
        self.switch_active_channel_force(Channel::Left, true);
    }

    fn update(&mut self, state: &mut AudioState, msg: HPEQMessage) -> Task<HPEQMessage> {
        match msg {
            Equaliser(channel, event) => {
                self.handle_eq_event(state, channel, event);
            }

            EqualiserValue(value) => {
                let channel = self.active_channel;
                let other = self.active_channel.other();

                // Let the commands fill this vec with messages, we'll send them to the handler,
                // then apply changes to the view at the end.
                let messages = vec![];
                if let Some(band) = self.active_band {
                    match value {
                        Type(band_type) => {
                            if let Some(band) = self.active_band {
                                // TODO - MESSAGE
                                self.temp[channel][band].band_type = band_type;

                                if self.is_linked {
                                    self.temp[other][band].band_type = band_type;
                                }
                            }
                        }
                        Frequency(frequency) => {
                            if let Some(band) = self.active_band {
                                // TODO - MESSAGE
                                self.temp[channel][band].frequency = frequency;

                                if self.is_linked {
                                    self.temp[other][band].frequency = frequency;
                                }
                            }
                        }
                        Gain(gain) => {
                            if let Some(band) = self.active_band {
                                // TODO - MESSAGE
                                self.temp[channel][band].gain = gain;

                                if self.is_linked {
                                    self.temp[other][band].gain = gain;
                                }
                            }
                        }
                        Q(q) => {
                            if let Some(band) = self.active_band {
                                // TODO - MESSAGE
                                self.temp[channel][band].q = q;
                                if self.is_linked {
                                    self.temp[other][band].q = q;
                                }
                            }
                        }
                    }
                    for message in messages {
                        let _ = state.handle_message(message);
                    }

                    self.view[channel].set_band(band, self.temp[channel][band]);
                    if self.is_linked {
                        self.view[other].set_band(band, self.temp[other][band]);
                    }
                }
            }

            AddBand => {
                // This is kinda awkward, but here we go :)
                let channel = self.active_channel;
                let other = channel.other();

                // We just need to find the first disabled band..
                let found = self.temp[channel]
                    .iter()
                    .find(|(_, b)| !b.enabled)
                    .map(|(band, _)| band);

                // Update and Send it, this will probably be easier with a real state!
                if let Some(band) = found {
                    // There could be up to 4 message state changes, so collect and send later.
                    let messages = vec![];
                    if self.temp[channel][band].band_type == EqualiserBandType::NotSet {
                        warn!("EQ Band doesn't have type set, defaulting to BellBand");

                        // TODO - MESSAGE
                        self.temp[channel][band].band_type = EqualiserBandType::BellBand;
                        if self.is_linked {
                            self.temp[other][band].band_type = EqualiserBandType::BellBand;
                        }
                    }

                    // Enable the band
                    // TODO - MESSAGE
                    self.temp[channel][band].enabled = true;
                    if self.is_linked {
                        self.temp[other][band].enabled = true;
                    }

                    // Send and update the state
                    for message in messages {
                        let _ = state.handle_message(message);
                    }

                    // Update the views
                    self.active_band = Some(band);
                    self.view[channel].set_active(Some(band));
                    self.view[channel].set_band(band, self.temp[channel][band]);
                    if self.is_linked {
                        self.view[other].set_active(Some(band));
                        self.view[other].set_band(band, self.temp[other][band]);
                    }
                }
            }
            RemoveBand => {
                let channel = self.active_channel;
                let other = channel.other();

                if let Some(active) = self.active_band {
                    // TODO - MESSAGE
                    self.temp[channel][active].enabled = false;
                    self.view[channel].set_band(active, self.temp[channel][active]);
                    if self.is_linked {
                        self.temp[other][active].enabled = false;
                        self.view[other].set_band(active, self.temp[other][active]);
                    }

                    // Try and find a new active band on this channel.
                    self.active_band = None;
                    for band in EqualiserBand::iter() {
                        if self.temp[channel][band].enabled {
                            self.active_band = Some(band);
                            break;
                        }
                    }

                    let active_band = self.active_band;
                    self.view[channel].set_active(active_band);
                    if self.is_linked {
                        self.view[other].set_active(active_band);
                    }
                }
            }

            Balance(amount) => {
                self.balance = amount;
            }

            SubWoofer(amount) => {
                // Subwoofer is fun, get various messages based on amount
                let messages = Subwoofer::get_amount_messages(amount);
                for message in messages {
                    let _ = state.handle_message(message);
                }
            }

            Stereo(enabled) => {
                self.is_stereo = enabled;
            }

            ToggleLinked => {
                self.set_linked(!self.is_linked);
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

    fn handle_eq_press(&mut self, _state: &mut AudioState, channel: Channel, point: Point) {
        // Do we have a hit on a band?
        if self.check_band_hit(channel, point).is_some() {
            // Prepare for a drag, will be taken care of during the handle_eq_move callback
            self.view[channel].set_track_motion(true);
            self.drag_start = Some(Instant::now());
        }
    }
    fn handle_eq_moved(&mut self, _state: &mut AudioState, channel: Channel, point: Point) {
        if self.drag_start.is_some_and(|t| t.elapsed() >= DRAG_DELAY) {
            // We're dragging the active band, so should be updating its state
            let Some(band) = self.active_band else {
                return;
            };

            let plot = self.view[channel].plot_rect();

            // Grab the Frequency..
            let frequency = EqGeometry::x_to_freq(point.x, plot);
            let frequency = frequency.clamp(MIN_FREQUENCY as f32, MAX_FREQUENCY as f32);

            // TODO - MESSAGE
            // Logically, this makes sense to queue messages, process them all at once, then apply the changes.
            self.temp[channel][band].frequency = frequency as u32;
            if self.is_linked {
                self.temp[channel.other()][band].frequency = frequency as u32;
            }

            // Set the Gain..
            if band_type_has_gain(self.temp[channel][band].band_type) {
                let gain = EqGeometry::y_to_db(point.y, plot).clamp(MIN_GAIN, MAX_GAIN);
                let gain = (gain * 10.0).round() / 10.0;

                // TODO - MESSAGE
                self.temp[channel][band].gain = gain;
                if self.is_linked {
                    self.temp[channel.other()][band].gain = gain;
                }
            }

            self.view[channel].set_band(band, self.temp[channel][band]);
            if self.is_linked {
                self.view[channel.other()].set_band(band, self.temp[channel.other()][band]);
            }
        }
    }
    fn handle_eq_released(&mut self, _state: &AudioState, channel: Channel) {
        self.view[channel].set_track_motion(false);
        self.drag_start = None;
    }
    fn handle_eq_scrolled(&mut self, _state: &AudioState, c: Channel, p: Point, d: ScrollDelta) {
        let (channel, point, delta) = (c, p, d);

        if let Some(band) = self.check_band_hit(channel, point) {
            let delta = get_q_delta(delta);

            let q = self.temp[channel][band].q;
            let adjusted = (q + delta).clamp(0.1, 10.0);
            let adjusted = (adjusted * 10.0).round() / 10.0;

            // TODO - MESSAGE
            self.temp[channel][band].q = adjusted;
            self.view[channel].set_band(band, self.temp[channel][band]);

            if self.is_linked {
                self.temp[channel.other()][band].q = adjusted;
                self.view[channel.other()].set_band(band, self.temp[channel.other()][band]);
            }
        }
    }

    // Checks for a band hit, if found, updates the channel and active band, then returns the band.
    fn check_band_hit(&mut self, channel: Channel, point: Point) -> Option<EqualiserBand> {
        let bands = &self.temp[channel];
        let rect = self.view[channel].plot_rect();
        if let Some(selected_band) = EqGeometry::hit_test(rect, point, bands) {
            self.switch_active_channel(channel);

            self.active_band = Some(selected_band);
            self.view[channel].set_active(Some(selected_band));
            if self.is_linked {
                // If we're linked, update the other channel as well.
                self.view[channel.other()].set_active(Some(selected_band));
            }

            return Some(selected_band);
        }
        None
    }

    // When we become linked, outside of syncing both channels, we also need a couple of display
    // tweaks. Firstly, we disable the green border around the active channel, then we enable
    // a matching active band on the other channel.
    fn set_linked(&mut self, is_linked: bool) {
        self.is_linked = is_linked;
        if is_linked {
            self.view[self.active_channel].set_border_colour(None);
            self.view[self.active_channel.other()].set_active(self.active_band);
        } else {
            self.view[self.active_channel].set_border_colour(Some(Color::from_rgb8(0, 255, 0)));
            self.view[self.active_channel.other()].set_active(None);
        }
    }

    fn switch_active_channel(&mut self, channel: Channel) {
        self.switch_active_channel_force(channel, false);
    }

    // The force param will result in the border being cleared then reloaded, along with the
    // active band.
    fn switch_active_channel_force(&mut self, channel: Channel, force: bool) {
        if self.active_channel == channel && !force {
            return;
        }

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
            Some(band) => self.temp[channel][band].enabled,
            None => true,
        };

        // If it is, set the new active band, otherwise use the current active band
        let active_band = match find_new_band {
            true => EqualiserBand::iter().find(|&b| self.temp[channel][b].enabled),
            false => self.active_band,
        };

        self.active_channel = channel;
        self.active_band = active_band;
        self.view[channel].set_active(active_band);

        // Only update the border if we're not linked
        if !self.is_linked {
            self.view[channel].set_border_colour(Some(Color::from_rgb8(0, 255, 0)));
        }
    }

    fn view(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        column![
            self.add_eq_canvas(Channel::Left),
            container(self.add_controls(state)).height(Length::Fixed(80.0)),
            self.add_eq_canvas(Channel::Right),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(6.0)
        .padding(10.0)
        .into()
    }

    // Canvas
    fn add_eq_canvas(&self, active: Channel) -> Element<'_, HPEQMessage> {
        // Lets grab the base canvas from the view..
        let canvas: Element<'_, EQMouseEvent> = Element::from(
            Canvas::new(&self.view[active])
                .width(Length::Fill)
                .height(Length::Fill),
        );

        let canvas = match active {
            Channel::Left => canvas.map(|m| Equaliser(Channel::Left, m)),
            Channel::Right => canvas.map(|m| Equaliser(Channel::Right, m)),
        };

        let label = match active {
            Channel::Left => "LEFT",
            Channel::Right => "RIGHT",
        };

        // Should depend on whether self.active = active
        let label_colour = match self.active_channel == active && !self.is_linked {
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
        let balance_at_zero = self.balance == 0;
        let balance_slider = themed_slider(-100..=100, self.balance, Balance)
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

    fn mono_stereo_controls(&self, _state: &AudioState) -> Element<'_, HPEQMessage> {
        // TODO: Need a Stereo / Mono Icon!

        column![
            text("Stereo"),
            Space::new().height(14),
            checkbox(self.is_stereo).on_toggle(Stereo),
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(80.0))
        .into()
    }

    fn equaliser_controls(&self, _state: &AudioState) -> Element<'_, HPEQMessage> {
        let channel = self.active_channel;
        let bands = &self.temp[channel];

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

        let type_grid = self.eq_type_grid().map(EqualiserValue);
        let values = self.eq_values().map(EqualiserValue);

        row![buttons, type_grid, values]
            .spacing(10.0)
            .align_y(Alignment::Center)
            .into()
    }

    fn eq_type_grid(&self) -> Element<'_, HPEQValue> {
        let Some(active) = self.active_band else {
            return row![].into();
        };

        let current_type = self.temp[self.active_channel][active].band_type;
        let types: [(&'static str, EqualiserBandType); 6] = [
            ("eq_low_pass", EqualiserBandType::LowPassFilter),
            ("eq_high_pass", EqualiserBandType::HighPassFilter),
            ("eq_notch", EqualiserBandType::NotchFilter),
            ("eq_bell", EqualiserBandType::BellBand),
            ("eq_low_shelf", EqualiserBandType::LowShelf),
            ("eq_high_shelf", EqualiserBandType::HighShelf),
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

    fn eq_values(&self) -> Element<'_, HPEQValue> {
        let Some(active) = self.active_band else {
            return row![].into();
        };

        // Clone the band so we can extract values
        let band = self.temp[self.active_channel][active];

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

    fn link_control(&self, _state: &AudioState) -> Element<'_, HPEQMessage> {
        let name = if self.is_linked { "unlink" } else { "link" };
        svg_button_unstyled(name)
            .on_press(ToggleLinked)
            .width(Length::Fixed(30.0))
            .style(move |t, s| svg_button_style(t, s, self.is_linked))
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

    fn on_open(&mut self, state: &AudioState) {
        self.load_temp_data(state);
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
