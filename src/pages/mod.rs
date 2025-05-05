use beacn_mic_lib::device::BeacnMic;
use egui::Ui;
use crate::state::BeacnMicState;

pub(crate) mod about;
pub(crate) mod config;
pub(crate) mod lighting;

mod config_pages;

pub trait MicPage {
    fn icon(&self) -> &'static str;
    fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState);
}