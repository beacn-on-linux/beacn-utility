use crate::devices::manager::DefinitionState;
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::page::{AudioPage, PageMessage};
use iced::Task;
use iced::widget::{container, text};

pub struct StudioLink;
impl StudioLink {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for StudioLink {
    fn icon(&self) -> &'static str {
        "left_right"
    }

    fn should_show(&self, state: &AudioState) -> bool {
        !matches!(state.definition().state, DefinitionState::Error(_))

        // state.definition().device_type == DeviceType::BeacnStudio
        //     && state.headphones.studio_driverless == Some(false)
        //true
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        Task::none()
    }

    fn view(&self, state: &AudioState) -> iced::Element<'_, PageMessage> {
        container(text("Studio Link")).into()
    }
}
