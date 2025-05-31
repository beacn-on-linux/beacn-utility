pub(crate) mod compressor;
pub(crate) mod expander;
pub(crate) mod headphones;
pub(crate) mod mic_setup;
pub(crate) mod suppressor;
pub(crate) mod equaliser;

use beacn_mic_lib::audio::BeacnAudioDevice;
use crate::state::BeacnMicState;
use egui::Ui;

pub trait ConfigPage {
    fn title(&self) -> &'static str;
    fn ui(&mut self, ui: &mut Ui, mic: &Box<dyn BeacnAudioDevice>, state: &mut BeacnMicState);
}
