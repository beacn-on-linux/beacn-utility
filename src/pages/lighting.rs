use crate::pages::MicPage;
use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use beacn_mic_lib::messages::lighting::LightingMode::{
    Gradient, ReactiveMeterDown, ReactiveMeterUp, ReactiveRing, Solid, SparkleMeter, SparkleRandom,
    Spectrum,
};
use egui::{Align, Label, Layout, Response, RichText, Ui};

use crate::state::Lighting as LightingState;

#[allow(unused)]
pub struct Lighting {}

impl Lighting {
    pub fn new() -> Self {
        Self {}
    }
}

impl MicPage for Lighting {
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

impl Lighting {
    fn draw_types(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) {
        let mode = state.mode;

        let solid = mode == Solid;
        let gradient = mode == Gradient;
        let reactive = mode == ReactiveRing || mode == ReactiveMeterUp || mode == ReactiveMeterDown;
        let sparkle = mode == SparkleMeter || mode == SparkleRandom;
        let spectrum = mode == Spectrum;

        if ui.selectable_label(solid, "Solid Colour").clicked() {
            state.mode = Solid;
        };
        ui.add_space(10.0);
        if ui.selectable_label(gradient, "Gradient").clicked() {
            state.mode = Gradient;
        };
        ui.add_space(10.0);
        if ui.selectable_label(reactive, "Reactive Meter").clicked() {
            // Only change this if we're not already set to a reactive mode.
            if !reactive {
                state.mode = ReactiveRing;
            }
        };
        ui.add_space(10.0);
        if ui.selectable_label(sparkle, "Sparkle").clicked() {
            // Only change this if we're not already set to a sparkle mode.
            if !sparkle {
                state.mode = SparkleMeter;
            }
        };
        ui.add_space(10.0);
        if ui.selectable_label(spectrum, "Spectrum Cycle").clicked() {
            state.mode = Spectrum;
        };
    }

    fn draw_area(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
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

    fn draw_solid(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.label("Solid")
    }
    fn draw_gradient(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.label("Gradient")
    }
    fn draw_reactive(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.label("Reactive")
    }
    fn draw_sparkle(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.label("Sparkle")
    }
    fn draw_spectrum(&self, ui: &mut Ui, mic: &BeacnMic, state: &mut LightingState) -> Response {
        ui.label("Spectrum")
    }
}
