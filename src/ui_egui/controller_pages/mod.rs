pub(crate) mod about;
pub(crate) mod error;

use crate::devices::states::control::ControlState;
use egui::{Context, Ui};

pub trait ControllerPage {
    fn icon(&self) -> &'static str;
    fn show_on_error(&self) -> bool;
    fn ui(&mut self, ui: &mut Ui, state: &mut ControlState);

    fn on_page_open(&mut self, _: &Context) {}
    fn on_page_close(&mut self, _: &Context) {}

    fn on_close(&mut self) {}
}
