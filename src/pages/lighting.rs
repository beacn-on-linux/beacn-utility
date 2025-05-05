use crate::pages::MicPage;
use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use egui::Ui;

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

    fn ui(&mut self, ui: &mut Ui, _mic: &BeacnMic, _state: &mut BeacnMicState) {
        ui.heading("Lighting Section");
    }
}
