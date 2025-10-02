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
                                ui.add_space(15.0);
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
                        ui.radio_value(
                            mute_mode,
                            LightingMuteMode::Solid,
                            "Turn LED ring to a solid colour",
                        );
                        ui.radio_value(mute_mode, LightingMuteMode::Off, "Turn off LED ring");

                        if *mute_mode != previous {
                            let message = Message::Lighting(Lighting::MuteMode(*mute_mode));
                            state
                                .handle_message(message)
                                .expect("Failed to Send Message");
                        }

                        ui.add_space(15.);
                        let mute_colour = &mut state.lighting.mute_colour;
                        ui.label(RichText::new("Colour").strong());
                        if ui.color_edit_button_srgb(mute_colour).changed() {
                            let message = RGBA {
                                red: mute_colour[0],
                                green: mute_colour[1],
                                blue: mute_colour[2],
                                alpha: 0,
                            };
                            let message = Message::Lighting(Lighting::MuteColour(message));
                            state
                                .handle_message(message)
                                .expect("Failed to Send Message");
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

        // let width = ui.available_width() / 2.;
        // ui.separator();

        //ui.add_sized(ui.available_size(), Label::new("Bottom Section"));

        //ui.heading("Lighting Section");
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

        if ui.selectable_label(solid, "Solid Colour").clicked() {
            state.mic_mode = Solid;
            let message = Message::Lighting(Lighting::Mode(Solid));
            let _ = config.handle_message(message);
        };
        ui.add_space(10.0);
        if ui.selectable_label(gradient, "Gradient").clicked() {
            state.mic_mode = Gradient;
            let message = Message::Lighting(Lighting::Mode(Gradient));
            let _ = config.handle_message(message);
        };
        ui.add_space(10.0);
        if ui.selectable_label(reactive, "Reactive Meter").clicked() {
            // Only change this if we're not already set to a reactive mode.
            if !reactive {
                let message = Message::Lighting(Lighting::Mode(ReactiveRing));
                let _ = config.handle_message(message);
                state.mic_mode = ReactiveRing;
            }
        };
        ui.add_space(10.0);
        if ui.selectable_label(sparkle, "Sparkle").clicked() {
            // Only change this if we're not already set to a sparkle mode.
            if !sparkle {
                state.mic_mode = SparkleMeter;
                let message = Message::Lighting(Lighting::Mode(SparkleMeter));
                let _ = config.handle_message(message);
            }
        };
        ui.add_space(10.0);
        if ui.selectable_label(spectrum, "Spectrum Cycle").clicked() {
            let message = Message::Lighting(Lighting::Mode(Spectrum));
            let _ = config.handle_message(message);
            state.mic_mode = Spectrum;
        };
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

        if ui.selectable_label(solid, "Solid Colour").clicked() {
            state.studio_mode = StudioLightingMode::Solid;
            let message = Message::Lighting(Lighting::StudioMode(StudioLightingMode::Solid));
            let _ = config.handle_message(message);
        };

        ui.add_space(10.0);
        if ui.selectable_label(peak_meter, "Peak Meter").clicked() {
            state.studio_mode = StudioLightingMode::PeakMeter;
            let message = Message::Lighting(Lighting::StudioMode(StudioLightingMode::PeakMeter));
            let _ = config.handle_message(message);
        };

        ui.add_space(10.0);
        if ui.selectable_label(spectrum, "Solid Spectrum").clicked() {
            state.studio_mode = StudioLightingMode::SolidSpectrum;
            let message =
                Message::Lighting(Lighting::StudioMode(StudioLightingMode::SolidSpectrum));
            let _ = config.handle_message(message);
        };
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
            self.draw_primary_colour(ui, config, &mut state.colour1);
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
            self.draw_primary_colour(ui, config, &mut state.colour1);
            self.draw_secondary_colour(ui, config, &mut state.colour2);
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
            ui.label("Behaviour");

            if config.device_definition.device_type == DeviceType::BeacnMic {
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
                ui.add_space(4.);
            }
            self.draw_primary_colour(ui, config, &mut state.colour1);
            self.draw_secondary_colour(ui, config, &mut state.colour2);
            self.draw_meter_sensitivity(ui, config, &mut state.sensitivity);
            self.draw_ring_brightness(ui, config, &mut state.brightness);
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

            ui.horizontal(|ui| {
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
            });
            ui.add_space(4.);

            self.draw_primary_colour(ui, config, &mut state.colour1);
            self.draw_secondary_colour(ui, config, &mut state.colour2);
            self.draw_meter_sensitivity(ui, config, &mut state.sensitivity);
            self.draw_speed_direction(ui, config, &mut state.speed);
            self.draw_ring_brightness(ui, config, &mut state.brightness);
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
        ui.label("Primary Colour");
        if ui.color_edit_button_srgb(colour).changed() {
            let message = RGBA {
                red: colour[0],
                green: colour[1],
                blue: colour[2],
                alpha: 0,
            };
            let message = Message::Lighting(Lighting::Colour1(message));
            let _ = config.handle_message(message);
        }
        ui.add_space(4.);
    }

    fn draw_secondary_colour(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        colour: &mut [u8; 3],
    ) {
        ui.label("Secondary Colour");
        if ui.color_edit_button_srgb(colour).changed() {
            let message = RGBA {
                red: colour[0],
                green: colour[1],
                blue: colour[2],
                alpha: 0,
            };
            let message = Message::Lighting(Lighting::Colour2(message));
            let _ = config.handle_message(message);
        }
        ui.add_space(4.);
    }

    fn draw_speed_direction(&mut self, ui: &mut Ui, config: &mut BeacnAudioState, speed: &mut i32) {
        ui.label("Speed and Direction");
        if ui.add(egui::Slider::new(speed, -10..=10)).changed() {
            let message = Message::Lighting(Lighting::Speed(LightingSpeed(*speed)));
            let _ = config.handle_message(message);
        };
        ui.add_space(4.);
    }

    fn draw_meter_sensitivity(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        sensitivity: &mut f32,
    ) {
        ui.label("Meter Sensitivity");
        if ui.add(egui::Slider::new(sensitivity, 0.0..=10.0)).changed() {
            let value = Lighting::MeterSensitivity(LightingMeterSensitivty(*sensitivity));
            let message = Message::Lighting(value);
            let _ = config.handle_message(message);
        }
        ui.add_space(4.);
    }

    fn draw_meter_source(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        source: &mut LightingMeterSource,
    ) {
        ui.label("Meter Source");
        egui::ComboBox::from_label("")
            .selected_text(match source {
                LightingMeterSource::Microphone => "Microphone",
                LightingMeterSource::Headphones => "Headphones",
            })
            .show_ui(ui, |ui| {
                // TODO: There are better ways to do this
                if ui
                    .selectable_value(source, LightingMeterSource::Microphone, "Microphone")
                    .changed()
                {
                    let message = Message::Lighting(Lighting::MeterSource(*source));
                    let _ = config.handle_message(message);
                }
                if ui
                    .selectable_value(source, LightingMeterSource::Headphones, "Headphones")
                    .changed()
                {
                    let message = Message::Lighting(Lighting::MeterSource(*source));
                    let _ = config.handle_message(message);
                }
            });
        ui.add_space(4.);
    }

    fn draw_ring_brightness(
        &mut self,
        ui: &mut Ui,
        config: &mut BeacnAudioState,
        brightness: &mut i32,
    ) {
        ui.label("Ring Brightness");
        if ui.add(egui::Slider::new(brightness, 0..=100)).changed() {
            let value = Lighting::Brightness(LightingBrightness(*brightness));
            let message = Message::Lighting(value);
            let _ = config.handle_message(message);
        }
        ui.add_space(4.)
    }
}
