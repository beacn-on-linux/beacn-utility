use crate::pages::MicPage;
use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use egui::Ui;

#[allow(unused)]
pub struct About {}

impl About {
    pub fn new() -> Self {
        Self {}
    }
}

impl MicPage for About {
    fn icon(&self) -> &'static str {
        "gear"
    }

    fn ui(&mut self, ui: &mut Ui, _mic: &BeacnMic, _state: &mut BeacnMicState) {
        ui.heading("About Section");
    }
}
