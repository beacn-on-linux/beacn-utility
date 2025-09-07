use crate::ui::controller_pages::ControllerPage;
use crate::ui::shared_pages::errors::display_errors;
use crate::ui::states::controller_state::BeacnControllerState;
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
        display_errors(ui, &state.device_state.state, &state.device_state.errors);
    }
}
