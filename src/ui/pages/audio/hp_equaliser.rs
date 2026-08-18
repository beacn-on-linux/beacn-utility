use crate::devices::states::audio::AudioState;
use crate::ui::pages::page::{AudioPage, PageMessage};
use iced::widget::{container, text};
use iced::{Element, Task};

#[derive(Debug, Clone)]
pub enum HPEQMessage {}

pub struct HPEqualiser {}

impl HPEqualiser {
    pub fn new() -> Self {
        Self {}
    }

    fn update(&mut self, state: &mut AudioState, msg: HPEQMessage) -> Task<HPEQMessage> {
        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        container(text("HP Equaliser")).into()
    }
}

impl AudioPage for HPEqualiser {
    fn icon(&self) -> &'static str {
        "headphones"
    }

    fn update(&mut self, state: &mut AudioState, msg: PageMessage) -> Task<PageMessage> {
        if let PageMessage::AudioHPEqualiser(msg) = msg {
            return self.update(state, msg).map(PageMessage::AudioHPEqualiser);
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, PageMessage> {
        self.view(state).map(PageMessage::AudioHPEqualiser)
    }
}
