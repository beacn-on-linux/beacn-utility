use crate::ui::states::audio_state::BeacnAudioState;
use beacn_lib::manager::DeviceType;
use egui::{Context, Ui};

pub(crate) mod about;
pub(crate) mod config;
pub(crate) mod equaliser;
pub(crate) mod error;
pub(crate) mod hp_equaliser;
pub(crate) mod lighting;
pub(crate) mod link;

mod config_pages;

pub trait AudioPage {
    fn icon(&self) -> &'static str;
    fn show_on_error(&self) -> bool {
        false
    }

    fn should_show(&self, _: &BeacnAudioState) -> bool {
        true
    }
    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState);

    fn on_close(&mut self) {}

    fn on_page_open(&mut self, _: &Context) {}
    fn on_page_close(&mut self, _: &Context) {}
}
