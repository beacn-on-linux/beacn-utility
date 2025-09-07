use crate::ui::audio_pages::AudioPage;
use crate::ui::shared_pages::errors::display_errors;
use crate::ui::states::audio_state::BeacnAudioState;
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

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        display_errors(ui, &state.device_state.state, &state.device_state.errors);
    }
}
