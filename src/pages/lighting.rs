use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use egui::Ui;
use crate::pages::MicPage;

#[allow(unused)]
pub struct Lighting {}

impl Lighting {
    pub fn new() -> Self {
        Self {}
    }
}

impl MicPage for Lighting {
    fn icon(&self) -> &'static str {
        "🟢"
    }

    fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState) {
        ui.heading("Lighting Section");
    }
}
