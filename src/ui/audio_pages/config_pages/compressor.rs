use crate::ui::audio_pages::config_pages::{ConfigPage, map_to_range};
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::widgets::{draw_range, get_slider, toggle_button};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::compressor::CompressorMode::{Advanced, Simple};
use beacn_lib::audio::messages::compressor::{
    Compressor, CompressorMode, CompressorRatio, CompressorThreshold,
};
use beacn_lib::types::{MakeUpGain, TimeFrame};
use egui::Ui;
use strum::IntoEnumIterator;

pub struct CompressorPage;

impl ConfigPage for CompressorPage {
    fn title(&self) -> &'static str {
        "Compressor"
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        let mut comp = state.compressor;

        // Extract out all the current values
        let values = &mut comp.values[comp.mode];

        ui.horizontal_top(|ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if ui.checkbox(&mut values.enabled, "Enabled").changed() {
                        // Note, when we change this, we should update the value on both modes
                        for mode in CompressorMode::iter() {
                            let msg =
                                Message::Compressor(Compressor::Enabled(mode, values.enabled));
                            let _ = state.handle_message(msg);
                        }
                    }

                    ui.add_space(5.);

                    ui.horizontal(|ui| {
                        let s = toggle_button(ui, comp.mode == Simple, "Simple");
                        let a = toggle_button(ui, comp.mode == Advanced, "Advanced");

                        if ui.add_sized([105., 20.], s).clicked() {
                            let msg = Message::Compressor(Compressor::Mode(Simple));
                            state.handle_message(msg).expect("Failed");
                            comp.mode = Simple;
                        }
                        if ui.add_sized([105., 20.], a).clicked() {
                            let msg = Message::Compressor(Compressor::Mode(Advanced));
                            state.handle_message(msg).expect("Failed");
                            comp.mode = Advanced;
                        }
                    });

                    ui.add_space(5.);

                    // Threshold is a common slider
                    let s = get_slider(ui, "Threshold", "dB", &mut values.threshold, -90..=0);
                    if s.changed() {
                        let value = CompressorThreshold(values.threshold as f32);
                        let msg = Message::Compressor(Compressor::Threshold(comp.mode, value));
                        state.handle_message(msg).expect("Failed");
                    }

                    ui.add_space(5.);
                    if comp.mode == Simple {
                        ui.horizontal_centered(|ui| {
                            // Map the ratio to an amount
                            let amount = map_to_range(values.ratio, 1.0, 10.0, 0.0, 10.0);
                            let mut amount = amount.round() as u8;

                            let s = get_slider(ui, "Amount", "", &mut amount, 0..=10);
                            if s.changed() {
                                let ratio = map_to_range(amount as f32, 0.0, 10.0, 1.0, 10.0);

                                // Round the ratio to 2 decimal places, and store it
                                values.ratio = (ratio * 100.0).round() / 100.0;

                                // Send it
                                let ratio = CompressorRatio(values.ratio);
                                let message = Message::Compressor(Compressor::Ratio(Simple, ratio));
                                state.handle_message(message).expect("Failed");
                            }
                        });
                    } else if comp.mode == Advanced {
                        let s = get_slider(ui, "Ratio", ":1", &mut values.ratio, 0.0..=10.0);
                        if s.changed() {
                            let ratio = CompressorRatio(values.ratio);
                            let message = Message::Compressor(Compressor::Ratio(Advanced, ratio));
                            state.handle_message(message).expect("Failed");
                        }

                        ui.add_space(5.);

                        let s = get_slider(ui, "Attack", "ms", &mut values.attack, 1..=2000);
                        if s.changed() {
                            let attack = TimeFrame(values.attack as f32);
                            let message = Message::Compressor(Compressor::Attack(Advanced, attack));
                            state.handle_message(message).expect("Failed");
                        }

                        ui.add_space(5.);

                        let s = get_slider(ui, "Release", "ms", &mut values.release, 1..=2000);
                        if s.changed() {
                            let release = TimeFrame(values.release as f32);
                            let message =
                                Message::Compressor(Compressor::Release(Advanced, release));
                            state.handle_message(message).expect("Failed");
                        }
                    }
                });
            });

            ui.add_space(20.);
            if draw_range(ui, &mut values.makeup, 0.0..=12.0, "Make-up Gain", "dB") {
                let makeup = MakeUpGain(values.makeup);
                let message = Message::Compressor(Compressor::MakeupGain(comp.mode, makeup));
                state.handle_message(message).expect("Failed");
            }
        });
    }
}
