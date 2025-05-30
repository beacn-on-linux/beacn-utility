use crate::pages::MicPage;
use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use beacn_mic_lib::messages::{Message};
use beacn_mic_lib::messages::lighting::{Lighting, LightingBrightness, LightingMeterSensitivty, LightingMeterSource, LightingMode, LightingSpeed};
use beacn_mic_lib::messages::lighting::LightingMode::{
    Gradient, ReactiveMeterDown, ReactiveMeterUp, ReactiveRing, Solid, SparkleMeter, SparkleRandom,
    Spectrum,
};
use beacn_mic_lib::types::RGB;
use eframe::glow::RGB;
use egui::{Align, Label, Layout, Rect, Response, RichText, Ui};

use crate::state::Lighting as LightingState;

#[allow(unused)]
pub struct LightingPage {}

impl LightingPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl MicPage for LightingPage {
    fn icon(&self) -> &'static str {
        "bulb"
    }

    fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState) {
        // Lighting is relatively simple, we have a persistent bottom pane, and a top pane
        ui.add_sized(
            [ui.available_width(), ui.available_height() - 200.],
            |ui: &mut Ui| {
                ui.vertical_centered(|ui: &mut Ui| {
                    ui.horizontal_top(|ui| {
                        ui.add_sized([120.0, ui.available_height()], |ui: &mut Ui| {
                            ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                                ui.label(RichText::new("Lighting Style").strong());
                                ui.add_space(15.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(15.);
                                    ui.vertical(|ui| {
                                        self.draw_types(ui, mic, &mut state.lighting);
                                    });
                                })
                                .inner
                            })
                            .response
                        });
                        ui.separator();
                        ui.add_sized(ui.available_size(), |ui: &mut Ui| {
                            ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                                self.draw_area(ui, mic, &mut state.lighting)
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

        ui.add_sized(ui.available_size(), Label::new("Bottom Section"));

        ui.heading("Lighting Section");
    }
}

impl LightingPage {
    fn draw_types(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) {
        let mode = state.mode;

        let solid = mode == Solid;
        let gradient = mode == Gradient;
        let reactive = mode == ReactiveRing || mode == ReactiveMeterUp || mode == ReactiveMeterDown;
        let sparkle = mode == SparkleMeter || mode == SparkleRandom;
        let spectrum = mode == Spectrum;

        if ui.selectable_label(solid, "Solid Colour").clicked() {
            state.mode = Solid;
            let message = Message::Lighting(Lighting::Mode(Solid));
            let _ = mic.set_value(message);
        };
        ui.add_space(10.0);
        if ui.selectable_label(gradient, "Gradient").clicked() {
            state.mode = Gradient;
            let message = Message::Lighting(Lighting::Mode(Gradient));
            let _ = mic.set_value(message);
        };
        ui.add_space(10.0);
        if ui.selectable_label(reactive, "Reactive Meter").clicked() {
            // Only change this if we're not already set to a reactive mode.
            if !reactive {
                let message = Message::Lighting(Lighting::Mode(ReactiveRing));
                let _ = mic.set_value(message);
                state.mode = ReactiveRing;
            }
        };
        ui.add_space(10.0);
        if ui.selectable_label(sparkle, "Sparkle").clicked() {
            // Only change this if we're not already set to a sparkle mode.
            if !sparkle {
                state.mode = SparkleMeter;
                let message = Message::Lighting(Lighting::Mode(SparkleMeter));
                let _ = mic.set_value(message);
            }
        };
        ui.add_space(10.0);
        if ui.selectable_label(spectrum, "Spectrum Cycle").clicked() {
            let message = Message::Lighting(Lighting::Mode(Spectrum));
            let _ = mic.set_value(message);
            state.mode = Spectrum;
        };
    }

    fn draw_area(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        match state.mode {
            Solid => self.draw_solid(ui, mic, state),
            Spectrum => self.draw_spectrum(ui, mic, state),
            Gradient => self.draw_gradient(ui, mic, state),
            ReactiveRing | ReactiveMeterUp | ReactiveMeterDown => {
                self.draw_reactive(ui, mic, state)
            }
            SparkleRandom | SparkleMeter => self.draw_sparkle(ui, mic, state),
        }
    }

    fn draw_solid(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.vertical(|ui| {
            self.draw_primary_colour(ui, mic, &mut state.colour1);
            self.draw_ring_brightness(ui, mic, &mut state.brightness);
        }).response
    }
    fn draw_gradient(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.vertical(|ui| {
            self.draw_primary_colour(ui, mic, &mut state.colour1);
            self.draw_secondary_colour(ui, mic, &mut state.colour2);
            self.draw_speed_direction(ui, mic, &mut state.speed);
            self.draw_ring_brightness(ui, mic, &mut state.brightness);
        }).response
    }
    fn draw_reactive(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.vertical(|ui| {
            ui.label("Behaviour");

            ui.vertical(|ui| {
                if ui.radio_value(&mut state.mode, ReactiveRing, "Whole Ring Meter").changed() {
                    let message = Message::Lighting(Lighting::Mode(state.mode));
                    let _ = mic.set_value(message);
                }
                if ui.radio_value(&mut state.mode, ReactiveMeterUp, "Bar Meter Up").changed() {
                    let message = Message::Lighting(Lighting::Mode(state.mode));
                    let _ = mic.set_value(message);
                }
                if ui.radio_value(&mut state.mode, ReactiveMeterDown, "Bar Meter Down").changed() {
                    let message = Message::Lighting(Lighting::Mode(state.mode));
                    let _ = mic.set_value(message);
                }
            });
            ui.add_space(4.);

            self.draw_primary_colour(ui, mic, &mut state.colour1);
            self.draw_secondary_colour(ui, mic, &mut state.colour2);
            self.draw_meter_sensitivity(ui, mic, &mut state.sensitivity);
            self.draw_ring_brightness(ui, mic, &mut state.brightness);
            self.draw_meter_source(ui, mic, &mut state.source);
        }).response
    }
    fn draw_sparkle(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.vertical(|ui| {
            ui.label("Behaviour");

            ui.horizontal(|ui| {
                if ui.radio_value(&mut state.mode, SparkleRandom, "Sparkle Random").changed() {
                    let message = Message::Lighting(Lighting::Mode(state.mode));
                    let _ = mic.set_value(message);
                }
                if ui.radio_value(&mut state.mode, SparkleMeter, "Sparkle Meter").changed() {
                    let message = Message::Lighting(Lighting::Mode(state.mode));
                    let _ = mic.set_value(message);
                }
            });
            ui.add_space(4.);

            self.draw_primary_colour(ui, mic, &mut state.colour1);
            self.draw_secondary_colour(ui, mic, &mut state.colour2);
            self.draw_meter_sensitivity(ui, mic, &mut state.sensitivity);
            self.draw_speed_direction(ui, mic, &mut state.speed);
            self.draw_ring_brightness(ui, mic, &mut state.brightness);
            self.draw_meter_source(ui, mic, &mut state.source);
        }).response
    }
    fn draw_spectrum(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.vertical(|ui| {
            self.draw_speed_direction(ui, mic, &mut state.speed);
            self.draw_ring_brightness(ui, mic, &mut state.brightness);
        }).response
    }

    fn draw_primary_colour(&mut self, ui: &mut Ui, mic: &BeacnMic, colour: &mut [u8; 3]) {
        ui.label("Primary Colour");
        if ui.color_edit_button_srgb(colour).changed() {
            let message = RGB {
                red: colour[0],
                green: colour[1],
                blue: colour[2],
                alpha: 0,
            };
            let message = Message::Lighting(Lighting::Colour1(message));
            let _ = mic.set_value(message);
        }
        ui.add_space(4.);
    }

    fn draw_secondary_colour(&mut self, ui: &mut Ui, mic: &BeacnMic, colour: &mut [u8; 3],) {
        ui.label("Secondary Colour");
        if ui.color_edit_button_srgb(colour).changed() {
            let message = RGB {
                red: colour[0],
                green: colour[1],
                blue: colour[2],
                alpha: 0,
            };
            let message = Message::Lighting(Lighting::Colour2(message));
            let _ = mic.set_value(message);
        }
        ui.add_space(4.);
    }

    fn draw_speed_direction(&mut self, ui: &mut Ui, mic: &BeacnMic, speed: &mut i32) {
        ui.label("Speed and Direction");
        if ui.add(egui::Slider::new(speed, -10..=10)).changed() {
            let message = Message::Lighting(Lighting::Speed(LightingSpeed(*speed)));
            let _ = mic.set_value(message);
        };
        ui.add_space(4.);
    }

    fn draw_meter_sensitivity(&mut self, ui: &mut Ui, mic: &BeacnMic, sensitivity: &mut f32) {
        ui.label("Meter Sensitivity");
        if ui.add(egui::Slider::new(sensitivity, 0.0..=10.0)).changed() {
            let value = Lighting::MeterSensitivity(LightingMeterSensitivty(*sensitivity));
            let message = Message::Lighting(value);
            let _ = mic.set_value(message);
        }
        ui.add_space(4.);
    }

    fn draw_meter_source(&mut self, ui: &mut Ui, mic: &BeacnMic, source: &mut LightingMeterSource) {
        ui.label("Meter Source");
        egui::ComboBox::from_label("")
            .selected_text(
                match source {
                    LightingMeterSource::Microphone => "Microphone",
                    LightingMeterSource::Headphones => "Headphones",
                }
            )
            .show_ui(ui, |ui| {
                // TODO: There are better ways to do this
                if ui.selectable_value(source, LightingMeterSource::Microphone, "Microphone").changed() {
                    let message = Message::Lighting(Lighting::MeterSource(*source));
                    let _ = mic.set_value(message);
                }
                if ui.selectable_value(source, LightingMeterSource::Headphones, "Headphones").changed() {
                    let message = Message::Lighting(Lighting::MeterSource(*source));
                    let _ = mic.set_value(message);
                }
            });
        ui.add_space(4.);
    }

    fn draw_ring_brightness(&mut self, ui: &mut Ui, mic: &BeacnMic, brightness: &mut i32) {
        ui.label("Ring Brightness");
        if ui.add(egui::Slider::new(brightness, 0..=100)).changed() {
            let value = Lighting::Brightness(LightingBrightness(*brightness));
            let message = Message::Lighting(value);
            let _ = mic.set_value(message);
        }
        ui.add_space(4.)
    }
}
