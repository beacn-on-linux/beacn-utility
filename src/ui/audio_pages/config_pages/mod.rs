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

/// Maps a value from one range to another.
fn map_to_range<T>(value: T, value_min: T, value_max: T, target_min: T, target_max: T) -> f32
where
    T: Into<f32>,
{
    let value = value.into();
    let value_min = value_min.into();
    let value_max = value_max.into();
    let target_min = target_min.into();
    let target_max = target_max.into();

    target_min + ((target_max - target_min) * (value - value_min)) / (value_max - value_min)
}
