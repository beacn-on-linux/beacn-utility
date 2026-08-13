use crate::ui_iced::app::{DeviceState, Message};
use crate::ui_iced::pages::control::about::ControlAboutMessage;
use crate::ui_iced::pages::info_row;
use crate::ui_iced::pages::page::{Page, PageMessage};
use crate::ui_iced::widgets::helpers::composite::draw_horizontal_range;
use crate::{HASH, VERSION};
use beacn_lib::manager::DeviceType;
use iced::widget::{Space, column, row, rule, text};
use iced::{Alignment, Element, Length};
use std::time::Duration;

struct AboutUtilityPage;

impl AboutUtilityPage {
    fn view(&self) -> Element<'_, Message> {
        let title = "About the GoXLR Utility";

        let version_text = format!("{} - Rev: {}", VERSION, HASH);

        column![
            text(title).size(24),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
            info_row("Version:", version_text),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
        ]
        .into()
    }
}
