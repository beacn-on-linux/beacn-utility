use crate::ui::states::audio_state::BeacnAudioState;
use beacn_lib::audio::BeacnAudioDevice;
use egui::Ui;

pub(crate) mod about;
pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod lighting;

mod config_pages;

pub trait AudioPage {
    fn icon(&self) -> &'static str;
    fn show_on_error(&self) -> bool;
    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState);
}
