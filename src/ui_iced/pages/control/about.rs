use crate::devices::states::control::ControlState;
use crate::ui_iced::pages::page::{ControllerPage, PageMessage};
use iced::Task;
use iced::widget::{container, text};

pub struct About;

impl About {
    pub fn new() -> Self {
        Self {}
    }
}

impl ControllerPage for About {
    fn icon(&self) -> &'static str {
        "gear"
    }

    fn update(&mut self, state: &mut ControlState, message: PageMessage) -> Task<PageMessage> {
        Task::none()
    }

    fn view(&self, state: &ControlState) -> iced::Element<'_, PageMessage> {
        container(text("Mix / Mix Create Controls")).into()
    }
}
