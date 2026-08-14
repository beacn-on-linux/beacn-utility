use iced::widget::{Row, row, text};
use iced::{Alignment, Length};

pub mod app;
pub mod audio;
pub mod common;
pub mod control;
pub mod page;

pub(crate) fn info_row<T>(label: &str, value: String) -> Row<'_, T> {
    row![
        text(label).size(14).width(Length::Fixed(100.0)),
        text(value).size(14),
    ]
    .spacing(5)
    .align_y(Alignment::Center)
}
