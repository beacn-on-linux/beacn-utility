use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::page::{AudioPage, PageMessage};
use crate::ui_iced::widgets::helpers::svg::svg_button_unstyled;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphones::Headphones;
use beacn_lib::manager::DeviceType;
use iced::widget::{Space, checkbox, column, row, rule, text};
use iced::{Alignment, Element, Length, Task};
use log::error;

const COMPLIANCE_MODE_INFO_URL: &str =
    "https://github.com/beacn-on-linux/beacn-utility/wiki/Beacn-Mic-Compliancy-Mode";

pub struct About;

#[derive(Debug, Clone)]
pub enum AboutMessage {
    OpenUrl(String),
    SetStudioDriverless(bool),
    SetMicClassCompliant(bool),
}

impl About {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for About {
    fn icon(&self) -> &'static str {
        "gear"
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        let PageMessage::AboutPage(message) = message else {
            return Task::none();
        };

        match message {
            AboutMessage::OpenUrl(url) => {
                let _ = open::that_detached(url);
            }
            AboutMessage::SetStudioDriverless(enabled) => {
                if state.device_definition.device_type != DeviceType::BeacnStudio {
                    error!("Studio Driverless is only supported on Beacn Studio devices.");
                    return Task::none();
                }

                let message = Message::Headphones(Headphones::StudioDriverless(enabled));
                let _ = state.handle_message(message);
            }
            AboutMessage::SetMicClassCompliant(enabled) => {
                if state.device_definition.device_type != DeviceType::BeacnMic {
                    error!("Mic Compliancy Mode is only supported on Beacn Mic devices.");
                    return Task::none();
                }

                let message = Message::Headphones(Headphones::MicClassCompliant(enabled));
                let _ = state.handle_message(message);
            }
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> iced::Element<'_, PageMessage> {
        let title = match state.device_definition.device_type {
            DeviceType::BeacnMic => "About Beacn Mic",
            DeviceType::BeacnStudio => "About Beacn Studio",
            _ => "ERROR",
        };

        let location = &state.device_definition.location;
        let location_text = format!("{}:{}", location.bus_id, location.device_address);
        let serial_text = state.device_definition.device_info.serial.clone();
        let version_text = state.device_definition.device_info.version.to_string();

        let info_row = |label: &'static str, value: String| {
            row![
                text(label).size(14).width(Length::Fixed(100.0)),
                text(value).size(14),
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        };

        let mut content = column![
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
        ]
        .spacing(8);

        // Studio Driverless / Port 2 compliancy mode
        if let Some(enabled) = state.headphones.studio_driverless {
            content = content.push(
                checkbox(enabled)
                    .label("Enable Port 2 Compliancy Mode")
                    .on_toggle(AboutMessage::SetStudioDriverless),
            );
        }

        // Mic compliancy mode
        if let Some(enabled) = state.headphones.mic_class_compliant {
            let checkbox = checkbox(enabled)
                .label("Enable Mic Compliancy Mode")
                .on_toggle(AboutMessage::SetMicClassCompliant);

            let info_button = svg_button_unstyled("info")
                .width(16)
                .height(16)
                .on_press(AboutMessage::OpenUrl(COMPLIANCE_MODE_INFO_URL.to_string()));

            content = content.push(
                row![checkbox, info_button]
                    .spacing(6)
                    .align_y(Alignment::Center),
            );

            let note = "Note: When changing this value, the Beacn Mic will reboot.";
            content = content.push(text(note).size(13));
        }

        let content = Element::from(content.spacing(8).padding(20));
        content.map(PageMessage::AboutPage).into()
    }
}
