pub(crate) mod compressor;
pub(crate) mod equaliser;
pub(crate) mod expander;
pub(crate) mod headphones;
pub(crate) mod mic_setup;
pub(crate) mod suppressor;

use crate::ui::states::audio_state::BeacnAudioState;
use egui::Ui;

pub trait ConfigPage {
    fn title(&self) -> &'static str;
    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState);
}
