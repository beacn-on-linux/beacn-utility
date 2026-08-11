use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::audio::config_pages::{ChildMessage, ConfigPage};
use iced::widget::container;
use iced::widget::text;
use iced::{Element, Task};

pub struct Headphones;

#[derive(Debug, Clone)]
pub(crate) enum HeadphonesMessage {}

impl ConfigPage for Headphones {
    fn title(&self) -> &'static str {
        "Headphones"
    }

    fn update(&mut self, device: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
        Task::none()
    }

    fn view(&self, device: &AudioState) -> Element<'_, ChildMessage> {
        container(text("Headphones")).into()
    }
}
