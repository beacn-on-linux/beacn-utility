use crate::ui::states::audio_state::BeacnAudioState;
use beacn_lib::manager::DeviceType;
use egui::Ui;

pub(crate) mod about;
pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod lighting;
pub(crate) mod link;

mod config_pages;

pub trait AudioPage {
    fn icon(&self) -> &'static str;
    fn show_on_error(&self) -> bool { false }
    fn is_link_page(&self) -> bool { false }
    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState);

    fn is_studio_with_link(&self, state: &BeacnAudioState) -> bool {
        state.device_definition.device_type == DeviceType::BeacnStudio
            && state.headphones.studio_driverless
    }
}