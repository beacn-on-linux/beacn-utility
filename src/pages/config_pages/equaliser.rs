use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use egui::{Response, Ui};
use log::debug;

pub struct Equaliser;

impl Equaliser {
    pub fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState) {
        let eq = &mut state.equaliser;
        let mode = eq.mode;

        ui.horizontal_centered(|ui| {
            for (band, eq) in &mut eq.bands[mode] {
                ui.vertical(|ui| {
                    // Only show this band if it's enabled
                    if eq.enabled {
                        ui.label(format!("Type: {:?}", eq.band_type));
                        ui.label(format!("Frequency: {}", eq.frequency));
                        ui.label(format!("Gain: {}", eq.gain));
                        ui.label(format!("Q: {}", eq.q));
                    }
                });
            }
        });
    }
}
