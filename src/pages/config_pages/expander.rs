use crate::pages::config_pages::ConfigPage;
use crate::state::BeacnMicState;
use crate::widgets::{get_slider, toggle_button};
use beacn_lib::audio::BeacnAudioDevice;
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

    fn ui(&mut self, ui: &mut Ui, mic: &Box<dyn BeacnAudioDevice>, state: &mut BeacnMicState) {
        let expander = &mut state.expander;

        // Extract out all the current values
        let values = &mut expander.values[expander.mode];

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if ui.checkbox(&mut values.enabled, "Enabled").changed() {
                    for mode in ExpanderMode::iter() {
                        let message = Message::Expander(Expander::Enabled(mode, values.enabled));
                        mic.set_value(message).expect("Failed to Send Message");
                    }
                }

                ui.add_space(5.);

                ui.horizontal(|ui| {
                    let s = toggle_button(ui, expander.mode == Simple, "Simple");
                    let a = toggle_button(ui, expander.mode == Advanced, "Advanced");

                    if ui.add_sized([105., 20.], s).clicked() {
                        let message = Message::Expander(Expander::Mode(Simple));
                        mic.set_value(message).expect("Failed to Send Message");
                        expander.mode = Simple;
                    }
                    if ui.add_sized([105., 20.], a).clicked() {
                        let message = Message::Expander(Expander::Mode(Advanced));
                        mic.set_value(message).expect("Failed to Send Message");
                        expander.mode = Advanced;
                    }
                });

                ui.add_space(5.);

                let s = get_slider(ui, "Threshold", "dB", &mut values.threshold, -90..=0);
                if s.changed() {
                    let value = ExpanderThreshold(values.threshold as f32);
                    let message = Message::Expander(Expander::Threshold(expander.mode, value));
                    mic.set_value(message).expect("Failed to Send Message");
                }

                ui.add_space(5.);

                if expander.mode == Simple {
                    ui.horizontal_centered(|ui| {
                        // TODO: This should technically be 'Amount'
                        // We're using ratio here because that's the value that's changed, but I
                        // don't know the calculation to get from a percent to a ratio
                        let s = get_slider(ui, "Ratio", ":1", &mut values.ratio, 0.0..=10.0);
                        if s.changed() {
                            let value = ExpanderRatio(values.ratio);
                            let message = Message::Expander(Expander::Ratio(Simple, value));
                            mic.set_value(message).expect("Failed to Send Message");
                        }
                    });
                } else if expander.mode == Advanced {
                    let s = get_slider(ui, "Ratio", ":1", &mut values.ratio, 0.0..=10.0);
                    if s.changed() {
                        let value = ExpanderRatio(values.ratio);
                        let message = Message::Expander(Expander::Ratio(Simple, value));
                        mic.set_value(message).expect("Failed to Send Message");
                    }

                    ui.add_space(5.);

                    let s = get_slider(ui, "Attack", "ms", &mut values.attack, 0..=2000);
                    if s.changed() {
                        let value = TimeFrame(values.attack as f32);
                        let message = Message::Expander(Expander::Attack(Advanced, value));
                        mic.set_value(message).expect("Failed to Send Message");
                    }

                    ui.add_space(5.);

                    let s = get_slider(ui, "Release", "ms", &mut values.release, 0..=2000);
                    if s.changed() {
                        let value = TimeFrame(values.release as f32);
                        let message = Message::Expander(Expander::Release(Advanced, value));
                        mic.set_value(message).expect("Failed to Send Message");
                    }
                }
            });
        });
    }
}
