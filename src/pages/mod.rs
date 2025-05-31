use beacn_mic_lib::audio::BeacnAudioDevice;
use crate::state::BeacnMicState;
use egui::Ui;

pub(crate) mod about;
pub(crate) mod config;
pub(crate) mod lighting;
pub(crate) mod error;

mod config_pages;

pub trait AudioPage {
    fn icon(&self) -> &'static str;
    fn show_on_error(&self) -> bool;
    fn ui(&mut self, ui: &mut Ui, mic: &Box<dyn BeacnAudioDevice>, state: &mut BeacnMicState);
}
