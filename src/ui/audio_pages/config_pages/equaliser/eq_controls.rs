use crate::ui::SVG;
use crate::ui::audio_pages::config_pages::equaliser::eq_common::{
    Bands, EqGeometry, MAX_FREQUENCY, MAX_GAIN, MIN_FREQUENCY, MIN_GAIN, band_type_has_gain,
};
use crate::ui::audio_pages::config_pages::equaliser::eq_drawer::EqDrawView;
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::widgets::draw_draggable;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::equaliser::EQBandType::{
    BellBand, HighPassFilter, HighShelf, LowPassFilter, LowShelf, NotSet, NotchFilter,
};
use beacn_lib::audio::messages::equaliser::{
    EQBand, EQBandType, EQFrequency, EQGain, EQMode, EQQ, Equaliser,
};
use egui::{Align, Button, Color32, CornerRadius, Image, Layout, Response, Ui, vec2};
use log::{debug, warn};
use strum::IntoEnumIterator;

// This is basically a replacement for the original drawer. Rather than handling everything,
// we just manage interactions with the View and draw the buttons.
pub struct ParametricEq {
    // The current serial that this EQ represents
    serial: Option<String>,

    // The current Equaliser Mode
    eq_mode: EQMode,

    // The pure render widget
    view: EqDrawView,

    // Active bands for interactions
    active_band: Option<EQBand>,
    active_band_drag: Option<EQBand>,
}

impl ParametricEq {
    pub(crate) fn new() -> Self {
        Self {
            serial: None,
            eq_mode: EQMode::Simple,
            view: EqDrawView::new(),
            active_band: None,
            active_band_drag: None,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.serial = None;
        self.eq_mode = EQMode::Simple;
        self.view.clear();
        self.active_band = None;
        self.active_band_drag = None;
    }

    fn load_default_state(&self, state: &mut BeacnAudioState) {
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
            Message::Equaliser(Equaliser::Type(mode, EQBand::Band1, HighPassFilter)),
            Message::Equaliser(Equaliser::Type(mode, EQBand::Band2, BellBand)),
            Message::Equaliser(Equaliser::Type(mode, EQBand::Band3, HighShelf)),
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

    /// Shows the parametric equalizer in the UI
    pub fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) -> Response {
        // Are we rendering this for the current serial?
        if let Some(serial) = &self.serial {
            let serial = serial.clone();
            if serial != state.device_definition.device_info.serial {
                debug!("Resetting EQ for New Device: {serial}");
                // If the serial doesn't match, we need to reset the widget
                self.clear();
                self.serial = Some(state.device_definition.device_info.serial.clone());
            }
        } else {
            debug!(
                "Loading EQ For: {}",
                state.device_definition.device_info.serial
            );
            self.serial = Some(state.device_definition.device_info.serial.clone());
        }
        let mode = state.equaliser.mode;

        // If the mode has changed since our last render, the band data is
        // wholesale different - the view's cached geometry is stale.
        if self.eq_mode != mode {
            self.eq_mode = mode;
            self.view.invalidate_all();
        }

        // Reborrow the bands, we may have made changes.
        let mut bands = state.equaliser.bands[state.equaliser.mode];

        // Look for an active band to select if we don't have one
        if self.active_band.is_none() {
            for band in EQBand::iter() {
                if bands[band].enabled {
                    self.active_band = Some(band);
                    break;
                }
            }
        }

        let desired_size = vec2(ui.available_width(), ui.available_height() - 20.0);
        let output = self.view.ui(ui, desired_size, &bands, self.active_band);
        let response = output.response;

        #[allow(clippy::collapsible_if)]
        if response.hovered() {
            if let Some(pointer_pos) = response.hover_pos() {
                let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let scroll_up = scroll > 0.0;
                    self.handle_scroll(output.plot_rect, pointer_pos, scroll_up, &mut bands, state);
                }
            }
        }

        #[allow(clippy::collapsible_if)]
        if response.clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if let Some(band) = EqGeometry::hit_test(output.plot_rect, pointer_pos, &bands) {
                    self.active_band = Some(band);
                }
            }
        }

        #[allow(clippy::collapsible_if)]
        if response.drag_started() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let hit = EqGeometry::hit_test(output.plot_rect, pointer_pos, &bands);
                if let Some(band) = hit {
                    self.active_band = Some(band);
                }
                self.active_band_drag = hit;
            }
        }

        #[allow(clippy::collapsible_if)]
        if response.dragged() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                self.handle_drag(output.plot_rect, pointer_pos, &mut bands, state);
            }
        }
        if response.drag_stopped() {
            self.active_band_drag = None;
        }

        ui.add_space(5.0);
        let mut is_advanced = state.equaliser.mode == EQMode::Advanced;

        // We need to force a 28px height here, so that we center
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                vec2(f32::INFINITY, 26.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.add_space(20.0);

                    if ui.checkbox(&mut is_advanced, "Advanced").changed() {
                        let new_mode = if is_advanced {
                            EQMode::Advanced
                        } else {
                            EQMode::Simple
                        };
                        state.equaliser.mode = new_mode;
                        let _ = state.handle_message(Message::Equaliser(Equaliser::Mode(new_mode)));

                        self.eq_mode = new_mode;
                        self.view.invalidate_all();

                        // Update the bands
                        bands = state.equaliser.bands[state.equaliser.mode];

                        // Ok, first we need to check whether this band is enabled
                        if let Some(node) = self.active_band
                            && !bands[node].enabled
                        {
                            self.active_band = None;

                            // Try and find an active band
                            for band in EQBand::iter() {
                                if bands[band].enabled {
                                    self.active_band = Some(band);
                                    break;
                                }
                            }
                        }
                    }
                },
            );

            if let Some(active) = self.active_band {
                let active_band = &mut bands[active];

                if is_advanced {
                    ui.separator();

                    ui.allocate_ui_with_layout(
                        vec2(f32::INFINITY, 26.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.horizontal_centered(|ui| {
                                ui.style_mut().spacing.item_spacing = vec2(1.0, 0.0);

                                for band in EQBandType::iter() {
                                    if band == NotSet {
                                        continue;
                                    }

                                    let is_active = active_band.band_type == band;
                                    let icon = match band {
                                        LowPassFilter => "eq_low_pass",
                                        HighPassFilter => "eq_high_pass",
                                        NotchFilter => "eq_notch",
                                        BellBand => "eq_bell",
                                        LowShelf => "eq_low_shelf",
                                        HighShelf => "eq_high_shelf",
                                        _ => "",
                                    };
                                    let position = match band {
                                        LowPassFilter => ButtonPosition::First,
                                        HighShelf => ButtonPosition::Last,
                                        _ => ButtonPosition::Middle,
                                    };
                                    if eq_mode(ui, icon, is_active, position).clicked() {
                                        let msg = Equaliser::Type(mode, active, band);
                                        let _ = state.handle_message(Message::Equaliser(msg));

                                        active_band.band_type = band;
                                        self.view.invalidate_band(active);
                                    }
                                }
                            });
                        },
                    );
                    ui.separator();

                    ui.label("Frequency: ");
                    let drag = draw_draggable(&mut active_band.frequency, 20..=20000, "Hz");
                    if ui.add_sized([75.0, 20.0], drag).changed() {
                        let value = EQFrequency(active_band.frequency as f32);
                        let msg = Equaliser::Frequency(mode, active, value);
                        let _ = state.handle_message(Message::Equaliser(msg));

                        self.view.invalidate_band(active);
                    }
                }

                ui.separator();
                ui.label("Gain: ");
                let enabled = band_type_has_gain(active_band.band_type);
                let mut zero = 0.0;
                let value = if enabled {
                    &mut active_band.gain
                } else {
                    &mut zero
                };

                let drag = draw_draggable(value, -12.0..=12.0, "dB");
                if ui
                    .add_enabled(enabled, |ui: &mut Ui| ui.add_sized([75.0, 20.0], drag))
                    .changed()
                {
                    let value = EQGain(active_band.gain);
                    let msg = Equaliser::Gain(mode, active, value);
                    let _ = state.handle_message(Message::Equaliser(msg));

                    self.view.invalidate_band(active);
                }

                if is_advanced {
                    ui.separator();

                    ui.label("Q: ");
                    let drag = draw_draggable(&mut active_band.q, 0.1..=10.0, "");
                    if ui.add_sized([75.0, 20.0], drag).changed() {
                        let value = EQQ(active_band.q);
                        let msg = Equaliser::Q(mode, active, value);
                        let _ = state.handle_message(Message::Equaliser(msg));

                        self.view.invalidate_band(active);
                    }
                }
            }

            // Render the Add/Remove Band buttons regardless of what's there
            if is_advanced {
                ui.separator();

                let enabled = bands.values_mut().any(|b| !b.enabled);
                let button = Button::new("Add Band");
                #[allow(clippy::collapsible_if)]
                if ui.add_enabled(enabled, button).clicked() {
                    if let Some((band, eq)) = bands.iter_mut().find(|(_, b)| !b.enabled) {
                        if eq.band_type == NotSet {
                            warn!("EQ Band doesn't have type set, defaulting to BellBand");

                            let msg = Equaliser::Type(mode, band, BellBand);
                            let _ = state.handle_message(Message::Equaliser(msg));
                            eq.band_type = BellBand;
                        }

                        let msg = Equaliser::Enabled(mode, band, true);
                        let _ = state.handle_message(Message::Equaliser(msg));

                        eq.enabled = true;
                        self.view.invalidate_band(band)
                    }
                }

                if let Some(active) = self.active_band {
                    let enabled = bands.values().filter(|b| b.enabled).count() > 0;
                    let button = Button::new("-");
                    if ui.add_enabled(enabled, button).clicked() {
                        let msg = Equaliser::Enabled(mode, active, false);
                        let _ = state.handle_message(Message::Equaliser(msg));

                        bands[active].enabled = false;
                        self.view.invalidate_band(active);

                        // Try and find a new band to set active
                        self.active_band = None;

                        // Try and find an active band
                        for band in EQBand::iter() {
                            if bands[band].enabled {
                                self.active_band = Some(band);
                                break;
                            }
                        }
                    }
                }

                if self.active_band.is_none() {
                    let button = Button::new("Load Default");
                    if ui.add_enabled(true, button).clicked() {
                        self.load_default_state(state);
                    }
                }
            }
        });
        response
    }

    /// Handle drag interactions with the control points
    fn handle_drag(
        &mut self,
        plot_rect: egui::Rect,
        pointer_pos: egui::Pos2,
        bands: &mut Bands,
        state: &mut BeacnAudioState,
    ) {
        // We don't have an active item, so there's nothing to do
        let Some(active) = self.active_band_drag else {
            return;
        };
        let band = &mut bands[active];

        if self.eq_mode != EQMode::Simple {
            // Can't change the Frequency in simple mode, only the gain.
            let frequency = EqGeometry::x_to_freq(pointer_pos.x, plot_rect);
            let frequency = frequency.clamp(MIN_FREQUENCY as f32, MAX_FREQUENCY as f32);
            band.frequency = frequency as u32;

            let value = EQFrequency(band.frequency as f32);
            let msg = Equaliser::Frequency(self.eq_mode, active, value);
            let _ = state.handle_message(Message::Equaliser(msg));
        }

        // If this band supports gain, update it.
        if band_type_has_gain(band.band_type) {
            let gain = EqGeometry::y_to_db(pointer_pos.y, plot_rect).clamp(MIN_GAIN, MAX_GAIN);
            band.gain = (gain * 10.0).round() / 10.0;

            let value = EQGain(band.gain);
            let msg = Equaliser::Gain(self.eq_mode, active, value);
            let _ = state.handle_message(Message::Equaliser(msg));
        }

        // Clear out the cache for this band as it needs a redraw
        self.view.invalidate_band(active);
    }

    fn handle_scroll(
        &mut self,
        plot_rect: egui::Rect,
        pointer_position: egui::Pos2,
        scroll_up: bool,
        bands: &mut Bands,
        state: &mut BeacnAudioState,
    ) {
        if self.eq_mode == EQMode::Simple {
            // Can't adjust the Q in simple mode
            return;
        }

        if let Some(band) = EqGeometry::hit_test(plot_rect, pointer_position, bands) {
            self.active_band = Some(band);

            let q = bands[band].q;
            let new_q = if scroll_up { q + 0.2 } else { q - 0.2 };
            let new = new_q.clamp(0.1, 10.0);
            let rounded = (new * 10.0).round() / 10.0;
            bands[band].q = rounded;

            let msg = Equaliser::Q(self.eq_mode, band, EQQ(rounded));
            let _ = state.handle_message(Message::Equaliser(msg));

            // Invalidate existing renders for this band
            self.view.invalidate_band(band);
        }
    }
}

pub enum ButtonPosition {
    First,
    Middle,
    Last,
}

pub fn eq_mode(ui: &mut Ui, img: &str, active: bool, pos: ButtonPosition) -> Response {
    let image = SVG.get(img).unwrap().clone();

    let tint_colour = if active {
        Color32::WHITE
    } else {
        Color32::from_rgb(120, 120, 120)
    };

    let corner_radius = match pos {
        ButtonPosition::First => CornerRadius {
            nw: 6,
            ne: 0,
            sw: 6,
            se: 0,
        },
        ButtonPosition::Middle => CornerRadius::ZERO,
        ButtonPosition::Last => CornerRadius {
            nw: 0,
            ne: 6,
            sw: 0,
            se: 6,
        },
    };

    ui.scope(|ui| {
        ui.style_mut().spacing.button_padding = vec2(5.0, 5.0);
        ui.add(
            Button::image(
                Image::new(image)
                    .tint(tint_colour)
                    .fit_to_exact_size(vec2(35., 45.)),
            )
            .corner_radius(corner_radius)
            .selected(active),
        )
    })
    .inner
}
