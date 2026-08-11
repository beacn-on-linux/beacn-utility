use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::audio::config_pages::{ChildMessage, ConfigPage};
use iced::widget::container;
use iced::widget::text;
use iced::{Element, Task};

pub struct Compressor;

#[derive(Debug, Clone)]
pub(crate) enum CompressorMessage {}

impl ConfigPage for Compressor {
    fn title(&self) -> &'static str {
        "Compressor"
    }

    fn update(&mut self, device: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
        Task::none()
    }

    fn view(&self, device: &AudioState) -> Element<'_, ChildMessage> {
        container(text("Compressor")).into()
    }
}
