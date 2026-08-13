use crate::devices::manager::DefinitionState;
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::ui::pages::page::{AudioPage, PageMessage};
use beacn_lib::manager::DeviceType;
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
        // We're a Beacn Studio, we're not errored, and we're not driverless :D
        state.definition().device_type == DeviceType::BeacnStudio
            && !matches!(state.definition().state, DefinitionState::Error(_))
            && state.headphones.studio_driverless == Some(false)
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        Task::none()
    }

    fn view(&self, state: &AudioState) -> iced::Element<'_, PageMessage> {
        container(text("Studio Link")).into()
    }
}
