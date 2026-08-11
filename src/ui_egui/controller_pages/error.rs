use crate::ui_egui::controller_pages::ControllerPage;
use crate::ui_egui::shared_pages::errors::display_errors;
use crate::ui_egui::states::controller_state::BeacnControllerState;
use egui::Ui;

pub struct ErrorPage {}

impl ErrorPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl ControllerPage for ErrorPage {
    fn icon(&self) -> &'static str {
        "error"
    }

    fn show_on_error(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnControllerState) {
        display_errors(
            ui,
            &state.device_state.state,
            &state.device_definition.location,
            &state.device_state.errors,
        );
    }
}
