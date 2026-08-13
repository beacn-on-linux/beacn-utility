pub(crate) mod compressor;
pub(crate) mod expander;
pub(crate) mod headphones;
pub(crate) mod mic_equaliser;
pub(crate) mod mic_setup;
pub(crate) mod suppressor;

use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::audio::config_pages::compressor::CompressorMessage;
use crate::ui_iced::pages::audio::config_pages::expander::ExpanderMessage;

use crate::ui_iced::pages::audio::config_pages::headphones::HeadphonesMessage;
use beacn_lib::audio::messages::Message;
use iced::{Element, Task};

#[derive(Debug, Clone)]
pub(crate) enum ChildMessage {
    State(Message),
    Expander(ExpanderMessage),
    Compressor(CompressorMessage),
    Headphones(HeadphonesMessage),
}

pub trait ConfigPage {
    fn title(&self) -> &'static str;

    fn update(&mut self, _device: &mut AudioState, _msg: ChildMessage) -> Task<ChildMessage> {
        Task::none()
    }

    fn view(&self, device: &AudioState) -> Element<'_, ChildMessage>;
}

/// Maps a value from one range to another.
fn map_to_range<T>(value: T, value_min: T, value_max: T, target_min: T, target_max: T) -> f32
where
    T: Into<f32>,
{
    let value = value.into();
    let value_min = value_min.into();
    let value_max = value_max.into();
    let target_min = target_min.into();
    let target_max = target_max.into();

    target_min + ((target_max - target_min) * (value - value_min)) / (value_max - value_min)
}
