use crate::ui::audio_pages::config_pages::ConfigPage;
use crate::ui::states::audio_state::BeacnAudioState;
use crate::widgets::{draw_range, get_slider, toggle_button};
use beacn_lib::audio::messages::compressor::CompressorMode::{Advanced, Simple};
use beacn_lib::audio::messages::compressor::{
    Compressor, CompressorMode, CompressorRatio, CompressorThreshold,
};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::BeacnAudioDevice;
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
                            let _ = state.send_message(msg);
                        }
                    }

                    ui.add_space(5.);

                    ui.horizontal(|ui| {
                        let s = toggle_button(ui, comp.mode == Simple, "Simple");
                        let a = toggle_button(ui, comp.mode == Advanced, "Advanced");

                        if ui.add_sized([105., 20.], s).clicked() {
                            let msg = Message::Compressor(Compressor::Mode(Simple));
                            state.send_message(msg).expect("Failed to Send Message");
                            comp.mode = Simple;
                        }
                        if ui.add_sized([105., 20.], a).clicked() {
                            let msg = Message::Compressor(Compressor::Mode(Advanced));
                            state.send_message(msg).expect("Failed to Send Message");
                            comp.mode = Advanced;
                        }
                    });

                    ui.add_space(5.);

                    // Threshold is a common slider
                    let s = get_slider(ui, "Threshold", "dB", &mut values.threshold, -90..=0);
                    if s.changed() {
                        let value = CompressorThreshold(values.threshold as f32);
                        let msg = Message::Compressor(Compressor::Threshold(comp.mode, value));
                        state.send_message(msg).expect("Failed to Send Message");
                    }

                    ui.add_space(5.);
                    if comp.mode == Simple {
                        ui.horizontal_centered(|ui| {
                            // TODO: This should technically be 'Amount'
                            // We're using ratio here because that's the value that's changed, but I
                            // don't know the calculation to get from a percent to a ratio
                            let s = get_slider(ui, "Ratio", ":1", &mut values.ratio, 0.0..=10.0);
                            if s.changed() {
                                let ratio = CompressorRatio(values.ratio);
                                let message = Message::Compressor(Compressor::Ratio(Simple, ratio));
                                state.send_message(message).expect("Failed to Send Message");
                            }
                        });
                    } else if comp.mode == Advanced {
                        let s = get_slider(ui, "Ratio", ":1", &mut values.ratio, 0.0..=10.0);
                        if s.changed() {
                            let ratio = CompressorRatio(values.ratio);
                            let message = Message::Compressor(Compressor::Ratio(Advanced, ratio));
                            state.send_message(message).expect("Failed to Send Message");
                        }

                        ui.add_space(5.);

                        let s = get_slider(ui, "Attack", "ms", &mut values.attack, 1..=2000);
                        if s.changed() {
                            let attack = TimeFrame(values.attack as f32);
                            let message = Message::Compressor(Compressor::Attack(Advanced, attack));
                            state.send_message(message).expect("Failed to Send Message");
                        }

                        ui.add_space(5.);

                        let s = get_slider(ui, "Release", "ms", &mut values.release, 1..=2000);
                        if s.changed() {
                            let release = TimeFrame(values.release as f32);
                            let message =
                                Message::Compressor(Compressor::Release(Advanced, release));
                            state.send_message(message).expect("Failed to Send Message");
                        }
                    }
                });
            });

            ui.add_space(20.);
            if draw_range(ui, &mut values.makeup, 0.0..=12.0, "Make-up Gain", "dB") {
                let makeup = MakeUpGain(values.makeup);
                let message = Message::Compressor(Compressor::MakeupGain(comp.mode, makeup));
                state.send_message(message).expect("Failed to Send Message");
            }
        });
    }
}
