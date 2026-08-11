use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::page::{AudioPage, PageMessage};
use iced::Task;
use iced::widget::{container, text};

pub struct Lighting;
impl Lighting {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for Lighting {
    fn icon(&self) -> &'static str {
        "bulb"
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        Task::none()
    }

    fn view(&self, state: &AudioState) -> iced::Element<'_, PageMessage> {
        container(text("Lighting")).into()
    }
}
