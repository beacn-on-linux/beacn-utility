pub(crate) mod compressor;
pub(crate) mod expander;
pub(crate) mod headphones;
pub(crate) mod mic_setup;
pub(crate) mod suppressor;

use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use egui::Ui;

pub trait ConfigPage {
    fn title(&self) -> &'static str;
    fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState);
}
