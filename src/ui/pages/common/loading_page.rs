use crate::devices::manager::DefinitionState;
use crate::devices::states::{LoadState, State};
use crate::ui::app::DeviceState;
use crate::ui::pages::page::{Page, PageMessage};
use beacn_lib::manager::DeviceType;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::container;
use iced::{Element, Length};

pub(crate) struct LoadingPage;
impl LoadingPage {
    pub fn new() -> Self {
        Self
    }
}

impl Page for LoadingPage {
    fn icon(&self) -> &'static str {
        "hourglass"
    }

    fn should_show_fn(&self, device: &DeviceState) -> bool {
        // We should always show if the device is directly in error..
        if matches!(device.definition().state, DefinitionState::Error(_)) {
            return false;
        }

        // Check the State state..
        match device {
            DeviceState::Audio(state) => state.device_state.state == LoadState::Loading,
            DeviceState::Control(state) => state.device_state.state == LoadState::Loading,
        }
    }

    fn view_fn(&self, device: &DeviceState) -> Element<'_, PageMessage> {
        let device_type = match device.definition().device_type {
            DeviceType::BeacnMic => "Beacn Mic",
            DeviceType::BeacnStudio => "Beacn Studio",
            DeviceType::BeacnMixCreate => "Beacn Mix Create",
            DeviceType::BeacnMix => "Beacn Mix",
        };
        let text = format!("Loading {}...", device_type);

        container(
            iced::widget::text(text)
                .size(20)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }
}
