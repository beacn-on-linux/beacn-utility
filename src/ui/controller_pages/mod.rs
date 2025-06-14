pub(crate) mod about;

use crate::ui::states::controller_state::BeacnControllerState;
use egui::Ui;

pub trait ControllerPage {
    fn icon(&self) -> &'static str;
    fn show_on_error(&self) -> bool;
    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnControllerState);
}
