use crate::ui::audio_pages::config_pages::ConfigPage;
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::widgets::{draw_range, toggle_button};
use beacn_lib::audio::BeacnAudioDevice;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::bass_enhancement::BassPreset::{
    Preset1, Preset2, Preset3, Preset4,
};
use beacn_lib::audio::messages::bass_enhancement::{BassAmount, BassEnhancement};
use beacn_lib::audio::messages::deesser::DeEsser;
use beacn_lib::audio::messages::exciter::{Exciter, ExciterFreq};
use beacn_lib::audio::messages::mic_setup::{MicGain, MicSetup, StudioMicGain};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::Percent;
use egui::{Align, Label, Layout, Ui};
use log::debug;

pub struct MicSetupPage;

impl ConfigPage for MicSetupPage {
    fn title(&self) -> &'static str {
        "Mic Setup"
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        let device_type = state.device_definition.device_type;
        let spacing = 10.0;

        ui.horizontal_centered(|ui| {
            ui.add_space(spacing);

            let mut mic_setup = state.mic_setup;

            // The Beacn Studio has a different range for the Mic Gain, so we'll set it here.
            let range = match device_type {
                DeviceType::BeacnMic => 3..=20,
                DeviceType::BeacnStudio => 0..=69, // Nice.
                _ => panic!("This shouldn't happen"),
            };
            if draw_range(ui, &mut mic_setup.gain, range, "Mic Gain", "dB") {
                let message = match device_type {
                    DeviceType::BeacnMic => {
                        let value = MicGain(mic_setup.gain as u32);
                        Message::MicSetup(MicSetup::MicGain(value))
                    }
                    DeviceType::BeacnStudio => {
                        let value = StudioMicGain(mic_setup.gain as u32);
                        Message::MicSetup(MicSetup::StudioMicGain(value))
                    }
                    _ => panic!("This shouldn't happen"),
                };
                state.handle_message(message).expect("Failed to Send Message");
            }

            ui.add_space(spacing);
            ui.separator();
            ui.add_space(spacing);

            let mut de_esser = state.de_esser;
            if draw_range(ui, &mut de_esser.amount, 0..=100, "De-Esser", "%") {
                let value = Percent(de_esser.amount as f32);
                let message = Message::DeEsser(DeEsser::Amount(value));
                state.handle_message(message).expect("Failed to Send Message");
                debug!("DeEsser Change: {}", de_esser.amount);
            }

            ui.add_space(spacing);
            ui.separator();
            ui.add_space(spacing);

            ui.vertical(|ui| {
                ui.add_sized([184., 0.], |ui: &mut Ui| {
                    ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                        ui.label("Bass Enhancer");
                        ui.add_space(2.0);
                        ui.separator();
                    })
                    .response
                });

                ui.add_space(2.0);

                let mut bass = state.bass_enhancement;
                ui.horizontal_centered(|ui| {
                    ui.add_sized([80.0, ui.available_height()], |ui: &mut Ui| {
                        ui.vertical(|ui| {
                            ui.add_sized([80.0, 0.], Label::new("Style"));
                            ui.add_space(5.0);

                            // A little bit of abstraction here to keep lines readable
                            let button_size = [35.0, 35.0];

                            // Create the Buttons
                            let b1 = toggle_button(ui, bass.preset == Preset1, "1");
                            let b2 = toggle_button(ui, bass.preset == Preset2, "2");
                            let b3 = toggle_button(ui, bass.preset == Preset3, "3");
                            let b4 = toggle_button(ui, bass.preset == Preset4, "4");

                            ui.horizontal(|ui| {
                                if ui.add_sized(button_size, b1).clicked() {
                                    let messages = BassEnhancement::get_preset(Preset1);
                                    for message in messages {
                                        state
                                            .handle_message(message)
                                            .expect("Failed to Send Message");
                                    }
                                    bass.preset = Preset1;
                                }

                                if ui.add_sized(button_size, b2).clicked() {
                                    let messages = BassEnhancement::get_preset(Preset2);
                                    for message in messages {
                                        state
                                            .handle_message(message)
                                            .expect("Failed to Send Message");
                                    }
                                    bass.preset = Preset2;
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.add_sized(button_size, b3).clicked() {
                                    let messages = BassEnhancement::get_preset(Preset3);
                                    for message in messages {
                                        state
                                            .handle_message(message)
                                            .expect("Failed to Send Message");
                                    }
                                    bass.preset = Preset3;
                                }
                                if ui.add_sized(button_size, b4).clicked() {
                                    let messages = BassEnhancement::get_preset(Preset4);
                                    for message in messages {
                                        state
                                            .handle_message(message)
                                            .expect("Failed to Send Message");
                                    }
                                    bass.preset = Preset4;
                                }
                            })
                        })
                        .response
                    });

                    if draw_range(ui, &mut bass.amount, 0..=10, "Amount", "") {
                        let value = BassAmount(bass.amount as f32);
                        let message = Message::BassEnhancement(BassEnhancement::Amount(value));
                        state.handle_message(message).expect("Failed to Send Message");
                    }
                });
            });

            ui.add_space(spacing);
            ui.separator();
            ui.add_space(spacing);

            ui.vertical(|ui| {
                ui.add_sized([169., 0.], |ui: &mut Ui| {
                    ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                        ui.label("Exciter");
                        ui.add_space(2.0);
                        ui.separator();
                    })
                    .response
                });

                ui.add_space(2.0);

                ui.horizontal_centered(|ui| {
                    let mut excite = state.exciter;
                    if draw_range(ui, &mut excite.amount, 0..=100, "Amount", "%") {
                        let value = Percent(excite.amount as f32);
                        let message = Message::Exciter(Exciter::Amount(value));
                        state.handle_message(message).expect("Failed to Send Message");
                    }

                    if draw_range(ui, &mut excite.freq, 600..=5000, "Freq", "Hz") {
                        let value = ExciterFreq(excite.freq as f32);
                        let message = Message::Exciter(Exciter::Frequency(value));
                        state.handle_message(message).expect("Failed to Send Message");
                    }
                })
            });

            ui.add_space(spacing);
            ui.separator();
        });
    }
}
