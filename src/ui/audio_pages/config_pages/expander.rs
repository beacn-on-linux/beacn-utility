use crate::ui::audio_pages::config_pages::{ConfigPage, map_to_range};
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::widgets::{get_slider, toggle_button};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::expander::ExpanderMode::{Advanced, Simple};
use beacn_lib::audio::messages::expander::{
    Expander, ExpanderMode, ExpanderRatio, ExpanderThreshold,
};
use beacn_lib::types::TimeFrame;
use egui::Ui;
use strum::IntoEnumIterator;

pub struct ExpanderPage;

impl ConfigPage for ExpanderPage {
    fn title(&self) -> &'static str {
        "Expander"
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        let mut expander = state.expander;

        // Extract out all the current values
        let values = &mut expander.values[expander.mode];

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if ui.checkbox(&mut values.enabled, "Enabled").changed() {
                    for mode in ExpanderMode::iter() {
                        let message = Message::Expander(Expander::Enabled(mode, values.enabled));
                        state.handle_message(message).expect("Failed");
                    }
                }

                ui.add_space(5.);

                ui.horizontal(|ui| {
                    let s = toggle_button(ui, expander.mode == Simple, "Simple");
                    let a = toggle_button(ui, expander.mode == Advanced, "Advanced");

                    if ui.add_sized([105., 20.], s).clicked() {
                        let message = Message::Expander(Expander::Mode(Simple));
                        state.handle_message(message).expect("Failed");
                        expander.mode = Simple;
                    }
                    if ui.add_sized([105., 20.], a).clicked() {
                        let message = Message::Expander(Expander::Mode(Advanced));
                        state.handle_message(message).expect("Failed");
                        expander.mode = Advanced;
                    }
                });

                ui.add_space(5.);

                let s = get_slider(ui, "Threshold", "dB", &mut values.threshold, -90..=0);
                if s.changed() {
                    let value = ExpanderThreshold(values.threshold as f32);
                    let message = Message::Expander(Expander::Threshold(expander.mode, value));
                    state.handle_message(message).expect("Failed");
                }

                ui.add_space(5.);

                if expander.mode == Simple {
                    ui.horizontal_centered(|ui| {
                        // Shoutout to Beacn for helping out with how this maps, between 0 and 50%
                        // the ratio is mapped between 1 and 3, and between 51 and 100% it is mapped
                        // between 3.1 and 10.0, so we can grab the amount here.
                        let mut amount = if values.ratio <= 3.0 {
                            map_to_range(values.ratio, 1.0, 3.0, 0.0, 50.0)
                        } else {
                            map_to_range(values.ratio, 3.1, 10.0, 50.1, 100.0)
                        }
                        .round() as u8;

                        let s = get_slider(ui, "Amount", "%", &mut amount, 0..=100);
                        if s.changed() {
                            // Now do the reverse, update the ratio based on the amount
                            let ratio = if amount <= 50 {
                                map_to_range(amount as f32, 0.0, 50.0, 1.0, 3.0)
                            } else {
                                map_to_range(amount as f32, 51.0, 100.0, 3.1, 10.0)
                            };

                            // Round to 2 decimal places, and store in the state
                            values.ratio = (ratio * 100.0).round() / 100.0;

                            // Send it
                            let value = ExpanderRatio(values.ratio);
                            let message = Message::Expander(Expander::Ratio(Simple, value));
                            state.handle_message(message).expect("Failed");
                        }
                    });
                } else if expander.mode == Advanced {
                    let s = get_slider(ui, "Ratio", ":1", &mut values.ratio, 1.0..=10.0);
                    if s.changed() {
                        let value = ExpanderRatio(values.ratio);
                        let message = Message::Expander(Expander::Ratio(Simple, value));
                        state.handle_message(message).expect("Failed");
                    }

                    ui.add_space(5.);

                    let s = get_slider(ui, "Attack", "ms", &mut values.attack, 0..=2000);
                    if s.changed() {
                        let value = TimeFrame(values.attack as f32);
                        let message = Message::Expander(Expander::Attack(Advanced, value));
                        state.handle_message(message).expect("Failed");
                    }

                    ui.add_space(5.);

                    let s = get_slider(ui, "Release", "ms", &mut values.release, 0..=2000);
                    if s.changed() {
                        let value = TimeFrame(values.release as f32);
                        let message = Message::Expander(Expander::Release(Advanced, value));
                        state.handle_message(message).expect("Failed");
                    }
                }
            });
        });
    }
}
