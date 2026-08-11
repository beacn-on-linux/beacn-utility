use crate::devices::states::audio::AudioState;
use crate::ui_egui::audio_pages::AudioPage;
use crate::ui_egui::shared_pages::errors::display_errors;
use egui::Ui;

pub struct ErrorPage {}

impl ErrorPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for ErrorPage {
    fn icon(&self) -> &'static str {
        "error"
    }

    fn show_on_error(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut AudioState) {
        display_errors(
            ui,
            &state.device_state.state,
            &state.device_definition.location,
            &state.device_state.errors,
        );
    }
}
