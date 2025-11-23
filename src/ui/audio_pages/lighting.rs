use crate::ui::audio_pages::AudioPage;
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::states::audio_state::Lighting as LightingState;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::lighting::LightingMode::{
    Gradient, ReactiveMeterDown, ReactiveMeterUp, ReactiveRing, Solid, SparkleMeter, SparkleRandom,
    Spectrum,
};
use beacn_lib::audio::messages::lighting::{
    Lighting, LightingBrightness, LightingMeterSensitivty, LightingMeterSource, LightingMuteMode,
    LightingSpeed, LightingSuspendBrightness, LightingSuspendMode, StudioLightingMode,
};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::RGBA;
use egui::{Align, Layout, Response, RichText, Ui};

const TYPE_WIDTH: f32 = 120.0;
const LABEL_WIDTH: f32 = 125.0;
const CONTROL_WIDTH: f32 = 180.0;

pub struct LightingPage {}

impl LightingPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for LightingPage {
    fn icon(&self) -> &'static str {
        "bulb"
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        let device_type = state.device_definition.device_type;
        let mut lighting = state.lighting;

        // Lighting is relatively simple, we have a persistent bottom pane, and a top pane
        ui.add_sized(
            [ui.available_width(), ui.available_height() - 190.],
            |ui: &mut Ui| {
                ui.vertical_centered(|ui: &mut Ui| {
                    ui.horizontal_top(|ui| {
                        ui.add_sized([120.0, ui.available_height()], |ui: &mut Ui| {
                            ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                                ui.label(RichText::new("Lighting Style").strong());
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(15.);
                                    ui.vertical(|ui| match device_type {
                                        DeviceType::BeacnMic => {
                                            self.draw_types_mic(ui, state, &mut lighting)
                                        }
                                        DeviceType::BeacnStudio => {
                                            self.draw_types_studio(ui, state, &mut lighting)
                                        }
                                        _ => {
                                            ui.label("You shouldn't see this :)");
                                        }
                                    });
                                })
                                .inner
                            })
                            .response
                        });
                        ui.separator();
                        ui.add_sized(ui.available_size(), |ui: &mut Ui| {
                            ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                                self.draw_area(ui, state, &mut lighting)
                            })
                            .inner
                        })
                    })
                    .inner
                })
                .inner
            },
        );

        ui.separator();
        ui.add_space(5.0);
        ui.label(
            RichText::new("Other Lighting Options (note, this do not work cleanly under Linux)")
                .strong(),
        );
        ui.add_space(5.0);
        ui.separator();

        ui.add_sized(ui.available_size(), |ui: &mut Ui| {
            ui.horizontal(|ui: &mut Ui| {
                let separator_width = ui.spacing().item_spacing.x;
                let available_width = ui.available_width() - separator_width;
                let panel_width = available_width / 2.0;

                ui.add_sized([panel_width, ui.available_height()], |ui: &mut Ui| {
                    ui.vertical(|ui| {
                        let mute_mode = &mut state.lighting.mute_mode;

                        // The easiest way to handle this is to monitor the previous and see if it's
                        // changed, rather than having .click or .change on each radio
                        let previous = *mute_mode;

                        ui.label(RichText::new("When Muted").strong());
                        ui.add_space(10.);
                        ui.radio_value(mute_mode, LightingMuteMode::Nothing, "Do Nothing");
                        ui.radio_value(mute_mode, LightingMuteMode::Off, "Turn off LED ring");
                        ui.radio_value(
                            mute_mode,
                            LightingMuteMode::Solid,
                            "Turn LED ring to a solid colour",
                        );

                        if *mute_mode != previous {
                            let message = Message::Lighting(Lighting::MuteMode(*mute_mode));
                            state
                                .handle_message(message)
                                .expect("Failed to Send Message");
                        }

                        if state.lighting.mute_mode == LightingMuteMode::Solid {
                            ui.add_space(15.);
                            self.draw_colour_picker(
                                ui,
                                state,
                                &mut lighting.mute_colour,
                                "Mute Colour",
                                |rgba| Message::Lighting(Lighting::MuteColour(rgba)),
                            );
                        }
                    })
                    .response
                });
                ui.separator();
                ui.add_sized([panel_width, ui.available_height()], |ui: &mut Ui| {
                    ui.vertical(|ui| {
                        let suspend_mode = &mut state.lighting.suspend_mode;

                        // The easiest way to handle this is to monitor the previous and see if it's
                        // changed, rather than having .click or .change on each radio
                        let previous = *suspend_mode;

                        ui.label(RichText::new("When USB is Suspended").strong());
                        ui.add_space(10.);
                        ui.radio_value(suspend_mode, LightingSuspendMode::Nothing, "Do Nothing");
                        ui.radio_value(suspend_mode, LightingSuspendMode::Off, "Turn off LED ring");
                        ui.radio_value(
                            suspend_mode,
                            LightingSuspendMode::Brightness,
                            "Change the brightness:",
                        );
                        if *suspend_mode != previous {
                            let message = Message::Lighting(Lighting::SuspendMode(*suspend_mode));
                            state
                                .handle_message(message)
                                .expect("Failed to Send Message");
                        }

                        if ui
                            .add(egui::Slider::new(
                                &mut state.lighting.suspend_brightness,
                                0..=100,
                            ))
                            .changed()
                        {
                            // We need to change the suspend mode if this is interacted with
                            if state.lighting.suspend_mode != LightingSuspendMode::Brightness {
                                let message = Message::Lighting(Lighting::SuspendMode(
                                    LightingSuspendMode::Brightness,
                                ));
                                state
                                    .handle_message(message)
                                    .expect("Failed to Send Message");
                            }

                            let value = Lighting::SuspendBrightness(LightingSuspendBrightness(
                                state.lighting.suspend_brightness,
                            ));
                            let message = Message::Lighting(value);
                            state
                                .handle_message(message)
                                .expect("Failed to Send Message");
                        }
                    })
                    .response
                });
            })
            .response
        });
    }
}

impl LightingPage {
    fn draw_types_mic(&self, ui: &mut Ui, config: &mut BeacnAudioState, state: &mut LightingState) {
        let mode = state.mic_mode;

        let solid = mode == Solid;
        let gradient = mode == Gradient;
        let reactive = mode == ReactiveRing || mode == ReactiveMeterUp || mode == ReactiveMeterDown;
        let sparkle = mode == SparkleMeter || mode == SparkleRandom;
        let spectrum = mode == Spectrum;

        // Change the padding on the Selectable Labels
        let style = ui.style_mut();
        style.spacing.button_padding = egui::vec2(8.0, 4.0);

        self.draw_lighting_style(ui, config, "Solid Colour", solid, || {
            Some(Message::Lighting(Lighting::Mode(Solid)))
        });
        self.draw_lighting_style(ui, config, "Gradient", gradient, || {
            Some(Message::Lighting(Lighting::Mode(Gradient)))
        });
        self.draw_lighting_style(ui, config, "Reactive Meter", reactive, || {
            (!reactive).then_some(Message::Lighting(Lighting::Mode(ReactiveRing)))
        });
        self.draw_lighting_style(ui, config, "Sparkle", sparkle, || {
            (!sparkle).then_some(Message::Lighting(Lighting::Mode(SparkleRandom)))
        });
        self.draw_lighting_style(ui, config, "Spectrum Cycle", spectrum, || {
            Some(Message::Lighting(Lighting::Mode(Spectrum)))
        });
    }

    fn draw_types_studio(
        &self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        state: &mut LightingState,
    ) {
        let mode = state.studio_mode;

        let solid = mode == StudioLightingMode::Solid;
        let peak_meter = mode == StudioLightingMode::PeakMeter;
        let spectrum = mode == StudioLightingMode::SolidSpectrum;

        let style = ui.style_mut();
        style.spacing.button_padding = egui::vec2(8.0, 4.0);

        self.draw_lighting_style(ui, config, "Solid Colour", solid, || {
            Some(Message::Lighting(Lighting::StudioMode(
                StudioLightingMode::Solid,
            )))
        });
        self.draw_lighting_style(ui, config, "Peak Meter", peak_meter, || {
            Some(Message::Lighting(Lighting::StudioMode(
                StudioLightingMode::PeakMeter,
            )))
        });
        self.draw_lighting_style(ui, config, "Solid Spectrum", spectrum, || {
            Some(Message::Lighting(Lighting::StudioMode(
                StudioLightingMode::SolidSpectrum,
            )))
        });
    }

    fn draw_area(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        state: &mut LightingState,
    ) -> Response {
        if config.device_definition.device_type == DeviceType::BeacnStudio {
            match state.studio_mode {
                StudioLightingMode::Solid => self.draw_solid(ui, config, state),
                StudioLightingMode::PeakMeter => self.draw_reactive(ui, config, state),
                StudioLightingMode::SolidSpectrum => self.draw_spectrum(ui, config, state),
            }
        } else {
            match state.mic_mode {
                Solid => self.draw_solid(ui, config, state),
                Spectrum => self.draw_spectrum(ui, config, state),
                Gradient => self.draw_gradient(ui, config, state),
                ReactiveRing | ReactiveMeterUp | ReactiveMeterDown => {
                    self.draw_reactive(ui, config, state)
                }
                SparkleRandom | SparkleMeter => self.draw_sparkle(ui, config, state),
            }
        }
    }

    fn draw_solid(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        state: &mut LightingState,
    ) -> Response {
        ui.vertical(|ui| {
            ui.add_space(4.);
            self.draw_primary_colour(ui, config, &mut state.colour1);
            ui.add_space(15.);
            self.draw_ring_brightness(ui, config, &mut state.brightness);
        })
        .response
    }
    fn draw_gradient(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        state: &mut LightingState,
    ) -> Response {
        ui.vertical(|ui| {
            ui.add_space(4.);
            self.draw_primary_colour(ui, config, &mut state.colour1);
            self.draw_secondary_colour(ui, config, &mut state.colour2);
            ui.add_space(15.0);
            self.draw_speed_direction(ui, config, &mut state.speed);
            self.draw_ring_brightness(ui, config, &mut state.brightness);
        })
        .response
    }
    fn draw_reactive(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        state: &mut LightingState,
    ) -> Response {
        ui.vertical(|ui| {
            if config.device_definition.device_type == DeviceType::BeacnMic {
                ui.label("Behaviour");
                ui.vertical(|ui| {
                    if ui
                        .radio_value(&mut state.mic_mode, ReactiveRing, "Whole Ring Meter")
                        .changed()
                    {
                        let message = Message::Lighting(Lighting::Mode(state.mic_mode));
                        let _ = config.handle_message(message);
                    }
                    if ui
                        .radio_value(&mut state.mic_mode, ReactiveMeterUp, "Bar Meter Up")
                        .changed()
                    {
                        let message = Message::Lighting(Lighting::Mode(state.mic_mode));
                        let _ = config.handle_message(message);
                    }
                    if ui
                        .radio_value(&mut state.mic_mode, ReactiveMeterDown, "Bar Meter Down")
                        .changed()
                    {
                        let message = Message::Lighting(Lighting::Mode(state.mic_mode));
                        let _ = config.handle_message(message);
                    }
                });
                ui.add_space(15.);
            }
            ui.add_space(4.);
            self.draw_primary_colour(ui, config, &mut state.colour1);
            self.draw_secondary_colour(ui, config, &mut state.colour2);
            ui.add_space(15.);
            self.draw_meter_sensitivity(ui, config, &mut state.sensitivity);
            self.draw_ring_brightness(ui, config, &mut state.brightness);
            ui.add_space(15.);
            self.draw_meter_source(ui, config, &mut state.source);
        })
        .response
    }
    fn draw_sparkle(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        state: &mut LightingState,
    ) -> Response {
        ui.vertical(|ui| {
            ui.label("Behaviour");

            if ui
                .radio_value(&mut state.mic_mode, SparkleRandom, "Sparkle Random")
                .changed()
            {
                let message = Message::Lighting(Lighting::Mode(state.mic_mode));
                let _ = config.handle_message(message);
            }
            if ui
                .radio_value(&mut state.mic_mode, SparkleMeter, "Sparkle Meter")
                .changed()
            {
                let message = Message::Lighting(Lighting::Mode(state.mic_mode));
                let _ = config.handle_message(message);
            }

            ui.add_space(15.);

            self.draw_primary_colour(ui, config, &mut state.colour1);
            self.draw_secondary_colour(ui, config, &mut state.colour2);
            ui.add_space(15.);

            self.draw_meter_sensitivity(ui, config, &mut state.sensitivity);
            self.draw_speed_direction(ui, config, &mut state.speed);
            self.draw_ring_brightness(ui, config, &mut state.brightness);
            ui.add_space(15.);

            self.draw_meter_source(ui, config, &mut state.source);
        })
        .response
    }
    fn draw_spectrum(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        state: &mut LightingState,
    ) -> Response {
        ui.vertical(|ui| {
            ui.add_space(4.);
            self.draw_speed_direction(ui, config, &mut state.speed);
            self.draw_ring_brightness(ui, config, &mut state.brightness);
        })
        .response
    }

    fn draw_primary_colour(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        colour: &mut [u8; 3],
    ) {
        self.draw_colour_picker(ui, config, colour, "Primary Colour", |rgba| {
            Message::Lighting(Lighting::Colour1(rgba))
        });
    }

    fn draw_secondary_colour(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        colour: &mut [u8; 3],
    ) {
        self.draw_colour_picker(ui, config, colour, "Secondary Colour", |rgba| {
            Message::Lighting(Lighting::Colour2(rgba))
        });
    }

    fn draw_speed_direction(&mut self, ui: &mut Ui, config: &mut BeacnAudioState, speed: &mut i32) {
        self.draw_slider(ui, config, speed, -10..=10, "Speed and Direction:", |val| {
            let value = Lighting::Speed(LightingSpeed(val));
            Message::Lighting(value)
        });
    }

    fn draw_meter_sensitivity(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        sense: &mut f32,
    ) {
        self.draw_slider(ui, config, sense, 1.0..=10.0, "Meter Sensitivity:", |val| {
            let value = Lighting::MeterSensitivity(LightingMeterSensitivty(val));
            Message::Lighting(value)
        });
    }

    fn draw_meter_source(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        source: &mut LightingMeterSource,
    ) {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(LABEL_WIDTH, ui.spacing().interact_size.y),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_width(LABEL_WIDTH);
                    ui.label("Meter Source: ");
                },
            );

            ui.allocate_ui_with_layout(
                egui::vec2(CONTROL_WIDTH, ui.spacing().interact_size.y),
                Layout::left_to_right(Align::Center),
                |ui| {
                    let entries = [
                        (LightingMeterSource::Microphone, "Microphone"),
                        (LightingMeterSource::Headphones, "Headphones"),
                    ];

                    ui.spacing_mut().combo_width = CONTROL_WIDTH;
                    egui::ComboBox::from_label("")
                        .selected_text(match source {
                            LightingMeterSource::Microphone => "Microphone",
                            LightingMeterSource::Headphones => "Headphones",
                        })
                        .show_ui(ui, |ui| {
                            for (variant, label) in entries {
                                if ui.selectable_value(source, variant, label).changed() {
                                    let message = Message::Lighting(Lighting::MeterSource(*source));
                                    let _ = config.handle_message(message);
                                }
                            }
                        });
                },
            );
        });
        ui.add_space(4.);
    }

    fn draw_ring_brightness(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        brightness: &mut i32,
    ) {
        self.draw_slider(ui, config, brightness, 0..=100, "Ring Brightness:", |val| {
            let value = Lighting::Brightness(LightingBrightness(val));
            Message::Lighting(value)
        });
    }

    fn draw_lighting_style(
        &self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        label: &str,
        checked: bool,
        message_fn: impl FnOnce() -> Option<Message>,
    ) {
        ui.add_sized([TYPE_WIDTH, 0.0], |ui: &mut Ui| {
            ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                let label = ui.selectable_label(checked, label);
                if label.clicked()
                    && let Some(message) = message_fn()
                {
                    let _ = config.handle_message(message);
                }

                label
            })
            .inner
        });
    }

    fn draw_colour_picker(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        colour: &mut [u8; 3],
        label: &str,
        message_fn: impl FnOnce(RGBA) -> Message,
    ) {
        ui.horizontal(|ui| {
            if ui.color_edit_button_srgb(colour).changed() {
                let rgba = RGBA {
                    red: colour[0],
                    green: colour[1],
                    blue: colour[2],
                    alpha: 0,
                };
                let message = message_fn(rgba);
                let _ = config.handle_message(message);
            }
            ui.add_space(2.);
            ui.label(label);
        });

        ui.add_space(4.);
    }

    fn draw_slider<T>(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        value: &mut T,
        range: std::ops::RangeInclusive<T>,
        label: &str,
        message_fn: impl FnOnce(T) -> Message,
    ) where
        T: egui::emath::Numeric + Copy,
    {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(LABEL_WIDTH, ui.spacing().interact_size.y),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_width(LABEL_WIDTH);
                    ui.label(label);
                },
            );

            ui.allocate_ui_with_layout(
                egui::vec2(CONTROL_WIDTH, ui.spacing().interact_size.y),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().slider_width = CONTROL_WIDTH;
                    if ui.add(egui::Slider::new(value, range)).changed() {
                        let message = message_fn(*value);
                        let _ = config.handle_message(message);
                    }
                },
            );
        });
        ui.add_space(4.);
    }
}
