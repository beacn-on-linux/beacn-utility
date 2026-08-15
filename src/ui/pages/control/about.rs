use crate::devices::states::control::ControlState;
use crate::ui::pages::info_row;
use crate::ui::pages::page::{ControllerPage, PageMessage};
use crate::ui::widgets::helpers::composite::draw_horizontal_range;
use beacn_lib::controller::messages::Message;
use beacn_lib::manager::DeviceType;
use iced::widget::{Space, container, row, rule, text};
use iced::{Alignment, Element, Task};
use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub(crate) enum ControlAboutMessage {
    ButtonBrightness(u8),
    DisplayBrightness(u8),
    DisplayDim(Duration),
}

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
        let PageMessage::ControlAbout(msg) = message else {
            return Task::none();
        };
        match msg {
            ControlAboutMessage::ButtonBrightness(brightness) => {
                let msg = Message::ButtonBrightness(brightness);
                let _ = state.handle_message(msg, true);
            }
            ControlAboutMessage::DisplayBrightness(brightness) => {
                let msg = Message::DisplayBrightness(brightness);
                let _ = state.handle_message(msg, true);
            }
            ControlAboutMessage::DisplayDim(duration) => {
                let msg = Message::DisplayDimTime(duration);
                let _ = state.handle_message(msg, true);
            }
        }

        Task::none()
    }

    fn view(&self, state: &ControlState) -> iced::Element<'_, PageMessage> {
        let title = match state.device_definition.device_type {
            DeviceType::BeacnMix => "About Beacn Mix",
            DeviceType::BeacnMixCreate => "About Beacn Mix Create",
            _ => unreachable!(),
        };

        let location = &state.device_definition.location;
        let location_text = format!("{}:{}", location.bus_id, location.device_address);
        let serial_text = state.device_definition.device_info.serial.clone();
        let version_text = state.device_definition.device_info.version.to_string();

        let label = text("Display Brightness:").width(120);
        let value = state.saved_settings.display_brightness;
        let range = 1..=100;
        let msg = ControlAboutMessage::DisplayBrightness;
        let display_brightness = draw_horizontal_range("", value, range, "%", msg);
        let display_brightness = row![label, display_brightness].align_y(Alignment::Center);

        let label = text("Display Timeout:").width(120);
        let value = state.saved_settings.display_dim.as_secs();
        let range = 30..=300;
        let msg = |v| ControlAboutMessage::DisplayDim(Duration::from_secs(v));
        let display_dim = draw_horizontal_range("", value, range, "s", msg);
        let display_dim = row![label, display_dim].align_y(Alignment::Center);

        let label = text("Button Brightness:").width(120);
        let value = state.saved_settings.button_brightness;
        let range = 1..=10;
        let msg = ControlAboutMessage::ButtonBrightness;
        let button_brightness = draw_horizontal_range("", value, range, "", msg);
        let button_brightness = row![label, button_brightness].align_y(Alignment::Center);

        let mut content = iced::widget::column![
            text(title).size(24),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
            info_row("USB Location:", location_text),
            info_row("Serial:", serial_text),
            info_row("Version:", version_text),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
            display_brightness,
        ];

        if state.device_definition.device_type != DeviceType::BeacnMix {
            content = content.push(button_brightness);
        }
        content = content.push(display_dim).spacing(8);

        let element = Element::from(container(content).padding(20));
        element.map(PageMessage::ControlAbout)
    }
}
