pub(crate) mod about;
pub(crate) mod error;

use crate::ui::states::controller_state::BeacnControllerState;
use egui::{Context, Ui};

pub trait ControllerPage {
    fn icon(&self) -> &'static str;
    fn show_on_error(&self) -> bool;
    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnControllerState);

    fn on_page_open(&mut self, ctx: &Context) {}
    fn on_page_close(&mut self, ctx: &Context) {}

    fn on_close(&mut self) {}
}
