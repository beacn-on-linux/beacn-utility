use crate::devices::manager::DefinitionState;
use crate::devices::states::{ErrorMessage, LoadState, State};
use crate::ui_iced::app::DeviceState;
use crate::ui_iced::pages::page::{Page, PageMessage};
use beacn_lib::manager::{DeviceLocation, DeviceType};
use iced::Task;
use iced::font::Weight;
use iced::widget::{Column, Space, button, column, container, row, scrollable, text};
use iced::{Element, Font, Length};

#[derive(Debug, Clone)]
pub(crate) enum ErrorPageMessages {
    OpenUrl(String),
}

pub(crate) struct ErrorPage;
impl ErrorPage {
    pub fn new() -> Self {
        Self
    }
}

impl Page for ErrorPage {
    fn should_show_fn(&self, device: &DeviceState) -> bool {
        matches!(device.definition().state, DefinitionState::Error(_))
    }

    fn update_fn(&mut self, _: &mut DeviceState, message: PageMessage) -> Task<PageMessage> {
        match message {
            PageMessage::ErrorPage(url) => match url {
                ErrorPageMessages::OpenUrl(url) => {
                    let _ = open::that_detached(url);
                }
            },
            _ => {}
        }

        Task::none()
    }

    fn view_fn(&self, device: &DeviceState) -> Element<'_, PageMessage> {
        let state = match device {
            DeviceState::Audio(state) => &state.device_state,
            DeviceState::Control(state) => &state.device_state,
        };

        let is_mix = matches!(
            device.definition().device_type,
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate
        );
        let location = device.location();
        display_errors(&state.state, is_mix, location, &state.errors)
    }
}

fn heading_font() -> Font {
    Font {
        weight: Weight::Bold,
        ..Font::default()
    }
}

fn subheading_font() -> Font {
    Font {
        weight: Weight::Semibold,
        ..Font::default()
    }
}

pub fn display_errors(
    load_state: &LoadState,
    is_mix: bool,
    device_location: &DeviceLocation,
    errors: &[ErrorMessage],
) -> Element<'static, PageMessage> {
    let header = column![
        text("An error occurred while loading the device.")
            .size(24)
            .font(heading_font()),
        text(format!(
            "USB Location: {}:{}",
            device_location.bus_id, device_location.device_address
        ))
        .size(14),
    ]
    .spacing(4);

    let content = match load_state {
        LoadState::PermissionDenied => permission_denied(),
        LoadState::ResourceBusy => resource_busy(is_mix),
        LoadState::Error => error_details(errors),

        _ => column![text("Shouldn't Happen?")],
    };

    let body = column![header, Space::new().height(20), content]
        .spacing(0)
        .width(Length::Fill);

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(30)
        .into()
}

fn permission_denied() -> Column<'static, PageMessage> {
    let url =
        "https://github.com/beacn-on-linux/beacn-permissions/wiki/Installing-Device-Permission";
    let message = PageMessage::ErrorPage(ErrorPageMessages::OpenUrl(url.into()));

    column![
        text("Permission Denied").size(18).font(subheading_font()),
        text(
            "The application does not have permission \
             to access the connected device."
        ),
        Space::new().height(5),
        button(text("Please visit this wiki page for help."))
            .style(button::text)
            .padding(0)
            .on_press(message),
    ]
    .spacing(6)
}

fn resource_busy(show_firmware_note: bool) -> Column<'static, PageMessage> {
    let mut content = column![
        text("Resource Busy").size(18).font(subheading_font()),
        text(
            "The connected device is currently in use by another \
             application. Please close any other applications that \
             may be using the device and try again."
        ),
    ]
    .spacing(6);

    if show_firmware_note {
        content = content.push(Space::new().height(10)).push(
            row![
                text("Note:").font(subheading_font()),
                text(
                    "This problem may be caused by older firmware. \
                     Please ensure your device is up-to-date."
                ),
            ]
            .spacing(6),
        );
    }

    content
}

fn error_details(errors: &[ErrorMessage]) -> Column<'static, PageMessage> {
    let mut list = column![].spacing(15);

    for message in errors {
        let mut entry = column![].spacing(4);

        if let Some(error) = &message.error_text {
            entry = entry.push(text(format!("Error: {error:?}")));
        }

        if let Some(failed_message) = &message.failed_message {
            entry = entry.push(text(format!("Message: {failed_message:?}")));
        }

        list = list.push(entry);
    }

    column![
        text("Device in Error State")
            .size(18)
            .font(subheading_font()),
        Space::new().height(10),
        scrollable(list).height(Length::Shrink),
    ]
    .spacing(0)
}
