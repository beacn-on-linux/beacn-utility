use crate::devices::states::audio::EqualiserBandType::{BellBand, NotSet};
use crate::devices::states::audio::{AudioState, EqualiserBand, EqualiserBandType};
use crate::ui::pages::audio::config_pages::mic_equaliser::MicEqualiserEvent::{
    AddBand, LoadDefault, RemoveBand, SetAdvanced, SetFrequency, SetGain, SetQ, SetType,
};
use crate::ui::pages::audio::equaliser::eq_common::{
    EQ_MARGIN, EqGeometry, MAX_FREQUENCY, MAX_GAIN, MIN_FREQUENCY, MIN_GAIN, band_type_has_gain,
};
use crate::ui::pages::audio::equaliser::eq_drawer::{EQDrawView, EQMouseEvent};
use crate::ui::widgets::helpers::drag_value::styled_drag_value;
use crate::ui::widgets::helpers::svg::{svg_button, svg_button_style};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::equaliser::{
    EQBand, EQBandType, EQFrequency, EQGain, EQMode, EQQ, Equaliser,
};
use beacn_lib::types::HasRange;
use iced::border::Radius;
use iced::mouse::ScrollDelta;
use iced::widget::{Button, Canvas, button, checkbox, container, row, rule, text};
use iced::{Alignment, Element, Length, Padding, Point, Rectangle, Task};
use log::warn;
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;

const DRAG_DELAY: Duration = Duration::from_millis(80);

#[derive(Copy, Clone, Debug)]
pub enum MicEqualiserEvent {
    Equaliser(EQMouseEvent),
    SetAdvanced(bool),
    SetFrequency(u32),
    SetType(EqualiserBandType),
    SetGain(f32),
    SetQ(f32),

    LoadDefault,
    AddBand,
    RemoveBand,
}

pub struct MicEqualiser {
    eq_mode: EQMode,

    view: EQDrawView,

    active_band: Option<EqualiserBand>,
    active_band_drag: Option<EqualiserBand>,

    // Used to help drag detection
    pressed_at: Option<Instant>,
}

impl MicEqualiser {
    pub(crate) fn new() -> Self {
        Self {
            eq_mode: EQMode::Simple,
            view: EQDrawView::new(Default::default()),
            active_band: None,
            active_band_drag: None,

            pressed_at: None,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.eq_mode = EQMode::Simple;
        self.view.clear();
    }

    pub(crate) fn update(
        &mut self,
        state: &mut AudioState,
        message: MicEqualiserEvent,
    ) -> Task<MicEqualiserEvent> {
        match message {
            MicEqualiserEvent::Equaliser(msg) => match msg {
                EQMouseEvent::Pressed(position) => {
                    // Perform Hit Detection, and switch active point
                    let rect = self.view.plot_rect();
                    let bands = state.equaliser.bands[state.equaliser.mode];
                    if let Some(band) = EqGeometry::hit_test(rect, position, &bands) {
                        // Flag the time this was pressed, in case the user wants to drag..
                        self.pressed_at = Some(Instant::now());

                        // Flag as active, and that it might want dragging
                        self.active_band = Some(band);
                        self.active_band_drag = Some(band);

                        // Tell the view to highlight the band, and send movement events
                        self.view.set_track_motion(true);
                        self.view.set_active(self.active_band);
                    }
                }
                EQMouseEvent::Moved(position) => {
                    // Check to see whether we're dragging, and have passed the lockout
                    if self.active_band_drag.is_some()
                        && let Some(pressed_at) = self.pressed_at
                        && pressed_at.elapsed() >= DRAG_DELAY
                    {
                        // We're going to mutate the state, realistically we should be sending
                        // the message to the device.
                        self.handle_drag_event(self.view.plot_rect(), position, state);
                    }
                }
                EQMouseEvent::Released => {
                    // No drag left to handle, so clear up.
                    self.pressed_at = None;
                    self.active_band_drag = None;

                    // Tell the view to stop sending movement events
                    self.view.set_track_motion(false);
                }
                EQMouseEvent::Scrolled { delta, position } => {
                    let rect = self.view.plot_rect();
                    let bands = self.view.bands_mut();

                    if let Some(band) = EqGeometry::hit_test(rect, position, bands) {
                        // Might as well set this band active
                        self.active_band = Some(band);

                        // Ok, in iced, we don't just get a number, we get either a pixel value or a line value.
                        // We need to map these down into a Q value somehow, so lets start buy guessing.
                        const VALUE_PER_LINE: f32 = 0.2;
                        const PIXELS_PER_LINE: f32 = 20.0;

                        let delta = match delta {
                            ScrollDelta::Lines { y, .. } => y * VALUE_PER_LINE,
                            ScrollDelta::Pixels { y, .. } => y * VALUE_PER_LINE / PIXELS_PER_LINE,
                        };

                        let q = bands[band].q;
                        let adjusted = (q + delta).clamp(0.1, 10.0);
                        let adjusted = (adjusted * 10.0).round() / 10.0;
                        bands[band].q = adjusted;

                        let msg = Equaliser::Q(self.eq_mode, band.into(), EQQ(adjusted));
                        let _ = state.handle_message(Message::Equaliser(msg));

                        // Invalidate existing renders for this band
                        self.view.set_active(self.active_band);
                        self.view.invalidate_band(band);
                    }
                }
            },

            SetAdvanced(enabled) => {
                let new_mode = match enabled {
                    true => EQMode::Advanced,
                    false => EQMode::Simple,
                };
                let _ = state.handle_message(Message::Equaliser(Equaliser::Mode(new_mode)));

                self.eq_mode = new_mode;
                self.view.invalidate_all();
                self.view.set_bands(state.equaliser.bands[new_mode]);

                // Can we transition cleanly?
                if let Some(band) = self.active_band
                    && !state.equaliser.bands[new_mode][band].enabled
                {
                    // Current band isn't active in the new mode, clear it, then try and find a new one
                    self.active_band = None;

                    for band in EqualiserBand::iter() {
                        if state.equaliser.bands[new_mode][band].enabled {
                            self.active_band = Some(band);
                            break;
                        }
                    }
                }
            }
            SetFrequency(frequency) => {
                if let Some(active) = self.active_band {
                    let value = EQFrequency(frequency as f32);
                    let msg = Equaliser::Frequency(state.equaliser.mode, active.into(), value);
                    let _ = state.handle_message(Message::Equaliser(msg));

                    self.view.bands_mut()[active].frequency = frequency;
                    self.view.invalidate_band(active);
                }
            }
            SetType(band_type) => {
                if let Some(active) = self.active_band {
                    let mode = state.equaliser.mode;
                    let msg = Equaliser::Type(mode, active.into(), band_type.into());
                    let _ = state.handle_message(Message::Equaliser(msg));

                    self.view.bands_mut()[active].band_type = band_type;
                    self.view.invalidate_band(active);
                }
            }
            SetGain(gain) => {
                if let Some(active) = self.active_band {
                    let value = EQGain(gain);
                    let msg = Equaliser::Gain(state.equaliser.mode, active.into(), value);
                    let _ = state.handle_message(Message::Equaliser(msg));

                    self.view.bands_mut()[active].gain = gain;
                    self.view.invalidate_band(active);
                }
            }
            SetQ(q) => {
                if let Some(active) = self.active_band {
                    let msg = Equaliser::Q(state.equaliser.mode, active.into(), EQQ(q));
                    let _ = state.handle_message(Message::Equaliser(msg));

                    self.view.bands_mut()[active].q = q;
                    self.view.invalidate_band(active);
                }
            }
            LoadDefault => {
                let mode = state.equaliser.mode;
                self.load_default_state(state);

                self.view.invalidate_all();
                self.view.set_bands(state.equaliser.bands[mode]);

                self.active_band = Some(EqualiserBand::Band1);
                self.view.set_active(self.active_band);
            }
            AddBand => {
                // Simple process, find a band that's not enabled, and enable it
                let mode = state.equaliser.mode;
                let bands = self.view.bands_mut();

                if let Some((band, eq)) = bands.iter_mut().find(|(_, b)| !b.enabled) {
                    if eq.band_type == NotSet {
                        warn!("EQ Band doesn't have type set, defaulting to BellBand");

                        let msg = Equaliser::Type(mode, band.into(), BellBand.into());
                        let _ = state.handle_message(Message::Equaliser(msg));
                        eq.band_type = BellBand;
                    }

                    let msg = Equaliser::Enabled(mode, band.into(), true);
                    let _ = state.handle_message(Message::Equaliser(msg));
                    eq.enabled = true;

                    self.view.invalidate_band(band);

                    self.active_band = Some(band);
                    self.view.set_active(self.active_band);
                }
            }
            RemoveBand => {
                if let Some(active) = self.active_band {
                    let bands = self.view.bands_mut();
                    let mode = state.equaliser.mode;

                    let msg = Equaliser::Enabled(mode, active.into(), false);
                    let _ = state.handle_message(Message::Equaliser(msg));

                    bands[active].enabled = false;

                    // Try and find a new band to set active
                    self.active_band = None;

                    // Try and find an active band
                    for band in EqualiserBand::iter().rev() {
                        if bands[band].enabled {
                            self.active_band = Some(band);
                            self.view.set_active(self.active_band);
                            break;
                        }
                    }

                    // Invalidate the band we just disabled
                    self.view.invalidate_band(active);
                }
            }
        }

        Task::none()
    }

    fn load_default_state(&self, state: &mut AudioState) {
        // This can be used later as a 'Default' button
        let mode = state.equaliser.mode;
        if mode == EQMode::Simple {
            warn!("Should not be called in Simple Mode!");
        }

        let eq_freq_1 = EQFrequency(36.0);
        let eq_freq_2 = EQFrequency(500.0);
        let eq_freq_3 = EQFrequency(2000.0);

        let gain = EQGain(0.0);
        let q = EQQ(0.7);

        // This is basically the default setup for the 'Simple' Mode
        let messages = vec![
            Message::Equaliser(Equaliser::Enabled(mode, EQBand::Band1, true)),
            Message::Equaliser(Equaliser::Enabled(mode, EQBand::Band2, true)),
            Message::Equaliser(Equaliser::Enabled(mode, EQBand::Band3, true)),
            Message::Equaliser(Equaliser::Type(
                mode,
                EQBand::Band1,
                EQBandType::HighPassFilter,
            )),
            Message::Equaliser(Equaliser::Type(mode, EQBand::Band2, EQBandType::BellBand)),
            Message::Equaliser(Equaliser::Type(mode, EQBand::Band3, EQBandType::HighShelf)),
            Message::Equaliser(Equaliser::Frequency(mode, EQBand::Band1, eq_freq_1)),
            Message::Equaliser(Equaliser::Frequency(mode, EQBand::Band2, eq_freq_2)),
            Message::Equaliser(Equaliser::Frequency(mode, EQBand::Band3, eq_freq_3)),
            Message::Equaliser(Equaliser::Gain(mode, EQBand::Band1, gain)),
            Message::Equaliser(Equaliser::Gain(mode, EQBand::Band2, gain)),
            Message::Equaliser(Equaliser::Gain(mode, EQBand::Band3, gain)),
            Message::Equaliser(Equaliser::Q(mode, EQBand::Band1, q)),
            Message::Equaliser(Equaliser::Q(mode, EQBand::Band2, q)),
            Message::Equaliser(Equaliser::Q(mode, EQBand::Band3, q)),
        ];

        for message in messages {
            let _ = state.handle_message(message);
            state.set_local_value(message);
        }
    }

    fn handle_drag_event(&mut self, plot: Rectangle, pointer: Point, state: &mut AudioState) {
        let Some(active) = self.active_band_drag else {
            return;
        };

        let band = &mut self.view.bands_mut()[active];
        if self.eq_mode != EQMode::Simple {
            let frequency = EqGeometry::x_to_freq(pointer.x, plot)
                .clamp(MIN_FREQUENCY as f32, MAX_FREQUENCY as f32);

            let frequency = {
                band.frequency = frequency as u32;
                band.frequency
            };

            let value = EQFrequency(frequency as f32);
            let msg = Equaliser::Frequency(self.eq_mode, active.into(), value);
            let _ = state.handle_message(Message::Equaliser(msg));
        }

        let has_gain = {
            let band = &mut state.equaliser.bands[state.equaliser.mode][active];
            band_type_has_gain(band.band_type)
        };

        if has_gain {
            let gain = EqGeometry::y_to_db(pointer.y, plot).clamp(MIN_GAIN, MAX_GAIN);
            let gain = {
                band.gain = (gain * 10.0).round() / 10.0;
                band.gain
            };

            let value = EQGain(gain);
            let msg = Equaliser::Gain(self.eq_mode, active.into(), value);
            let _ = state.handle_message(Message::Equaliser(msg));
        }

        self.view.invalidate_band(active);
    }

    pub(crate) fn view(&self, _: &AudioState) -> Element<'_, MicEqualiserEvent> {
        let eq = Element::from(
            Canvas::new(&self.view)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .map(MicEqualiserEvent::Equaliser);

        container(eq)
            .padding(Padding {
                top: 10.0,
                bottom: 5.0,
                left: 10.0,
                right: 5.0,
            })
            .into()
    }

    pub(crate) fn eq_controls(&self, state: &AudioState) -> Element<'_, MicEqualiserEvent> {
        let is_advanced = state.equaliser.mode == EQMode::Advanced;

        // Ok, lets make some buttons :D
        let advanced_button = checkbox(is_advanced).on_toggle(SetAdvanced);
        let advanced_text = text("Advanced:");
        let advanced = row![advanced_text, advanced_button]
            .spacing(6.0)
            .align_y(Alignment::Center);

        let eq_buttons = if let Some(band) = self.active_band {
            // Ok, these are the EQ buttons
            let current_band_type = state.equaliser.bands[state.equaliser.mode][band].band_type;
            let eq_types = [
                (
                    "eq_low_pass",
                    current_band_type == EqualiserBandType::LowPassFilter,
                    SetType(EqualiserBandType::LowPassFilter),
                ),
                (
                    "eq_high_pass",
                    current_band_type == EqualiserBandType::HighPassFilter,
                    SetType(EqualiserBandType::HighPassFilter),
                ),
                (
                    "eq_notch",
                    current_band_type == EqualiserBandType::NotchFilter,
                    SetType(EqualiserBandType::NotchFilter),
                ),
                (
                    "eq_bell",
                    current_band_type == EqualiserBandType::BellBand,
                    SetType(EqualiserBandType::BellBand),
                ),
                (
                    "eq_low_shelf",
                    current_band_type == EqualiserBandType::LowShelf,
                    SetType(EqualiserBandType::LowShelf),
                ),
                (
                    "eq_high_shelf",
                    current_band_type == EqualiserBandType::HighShelf,
                    SetType(EqualiserBandType::HighShelf),
                ),
            ];

            eq_types
                .iter()
                .copied()
                .enumerate()
                .map(|(i, (name, selected, callback))| {
                    let button = svg_button(name, selected)
                        .width(Length::Fixed(45.0))
                        .on_press(callback);

                    if i == 0 {
                        // first
                        button.style(move |theme, status| {
                            let mut style = svg_button_style(theme, status, selected);

                            // For the first button, we need the border on the left size.
                            style.border.radius = Radius {
                                top_left: 6.0,
                                top_right: 0.0,
                                bottom_right: 0.0,
                                bottom_left: 6.0,
                            };

                            style
                        })
                    } else if i == eq_types.len() - 1 {
                        // Last one needs a radius on the right
                        button.style(move |theme, status| {
                            let mut style = svg_button_style(theme, status, selected);
                            style.border.radius = Radius {
                                top_left: 0.0,
                                top_right: 6.0,
                                bottom_right: 6.0,
                                bottom_left: 0.0,
                            };

                            style
                        })
                    } else {
                        button.style(move |theme, status| {
                            let mut style = svg_button_style(theme, status, selected);
                            style.border.radius = Radius {
                                top_left: 0.0,
                                top_right: 0.0,
                                bottom_right: 0.0,
                                bottom_left: 0.0,
                            };

                            style
                        })
                    }
                })
                .fold(row![], |row, button| row.push(button))
                .spacing(1.0)
        } else {
            row![]
        };

        let frequency = if let Some(band) = self.active_band {
            let value = state.equaliser.bands[state.equaliser.mode][band].frequency;
            let range = EQFrequency::range();
            let range: RangeInclusive<u32> = (*range.start() as u32)..=(*range.end() as u32);
            let frequency = styled_drag_value(value, range)
                .suffix("Hz")
                .on_change(SetFrequency)
                .width(Length::Fixed(75.0));

            let frequency_text = text("Frequency: ");
            row![frequency_text, frequency]
                .spacing(2.0)
                .align_y(Alignment::Center)
        } else {
            row![]
        };

        let gain = if let Some(band) = self.active_band {
            let band_type = state.equaliser.bands[state.equaliser.mode][band].band_type;
            let has_gain = band_type_has_gain(band_type);
            let enabled = match has_gain {
                true => Some(SetGain),
                false => None,
            };
            let value = match has_gain {
                true => state.equaliser.bands[state.equaliser.mode][band].gain,
                false => 0.0,
            };
            let range = EQGain::range();
            let gain = styled_drag_value(value, range)
                .suffix("dB")
                .on_change_maybe(enabled)
                .width(Length::Fixed(75.0));

            let gain_text = text("Gain: ");
            row![gain_text, gain]
                .spacing(2.0)
                .align_y(Alignment::Center)
        } else {
            row![]
        };

        let q = if let Some(band) = self.active_band {
            let value = state.equaliser.bands[state.equaliser.mode][band].q;
            let range = EQQ::range();
            let q = styled_drag_value(value, range)
                .width(Length::Fixed(75.0))
                .on_change(SetQ);
            let q_text = text("Q: ");
            row![q_text, q].spacing(2.0).align_y(Alignment::Center)
        } else {
            row![]
        };

        let add_band = self
            .view
            .bands()
            .values()
            .any(|b| !b.enabled)
            .then_some(AddBand);
        let remove_band = self
            .view
            .bands()
            .values()
            .any(|b| b.enabled)
            .then_some(RemoveBand);

        let add_band = padded_button("Add Band").on_press_maybe(add_band);
        let remove_band = padded_button("-").on_press_maybe(remove_band);
        let load_default = padded_button("Load Default").on_press(LoadDefault);

        let mut row = row![advanced, rule::vertical(1.0),]
            .align_y(Alignment::Center)
            .spacing(10.0)
            .padding(Padding {
                top: -4.0,
                bottom: 0.0,
                left: EQ_MARGIN.width + 13.0,
                right: 0.0,
            });

        if self.active_band.is_some() {
            if is_advanced {
                row = row.push(eq_buttons);
                row = row.push(rule::vertical(1.0));

                row = row.push(frequency);
                row = row.push(rule::vertical(1.0));
            }

            row = row.push(gain);
            row = row.push(rule::vertical(1.0));

            if is_advanced {
                row = row.push(q);
                row = row.push(rule::vertical(1.0));
            }
        }

        if is_advanced {
            row = row.push(add_band);

            if self.active_band.is_some() {
                row = row.push(remove_band);
            }
        }

        if self.active_band.is_none() {
            row = row.push(load_default);
        }

        row.into()
    }

    // Gives us an oppertunity to prepare for a new device
    pub(crate) fn load_device(&mut self, state: &AudioState) {
        let mode = state.equaliser.mode;
        let bands = state.equaliser.bands[state.equaliser.mode];

        if self.active_band.is_none() {
            for band in EqualiserBand::iter() {
                if bands[band].enabled {
                    self.active_band = Some(band);
                    self.view.set_active(self.active_band);
                    break;
                }
            }
        }

        self.view.set_bands(bands);
        self.eq_mode = mode;
    }
}

fn padded_button(text: &str) -> Button<'_, MicEqualiserEvent> {
    button(text).padding(Padding {
        top: 2.0,
        right: 4.0,
        bottom: 2.0,
        left: 4.0,
    })
}
