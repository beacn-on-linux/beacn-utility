use iced::widget::{Button, Space, button, column, container, rule, svg};
use iced::{Alignment, Color, Element, Length, Theme};

use crate::ui_iced::SVG;
use crate::ui_iced::app::Message;
use crate::ui_iced::widgets::helpers::svg::svg_button_style;

pub(crate) fn pipeweaver_sidebar_item(active: bool) -> Element<'static, Message> {
    column![
        Space::new()
            .width(Length::Shrink)
            .height(Length::Fixed(2.0)),
        round_nav_button("pipeweaver", active).on_press(Message::ActivatePipeweaver),
        Space::new()
            .width(Length::Shrink)
            .height(Length::Fixed(11.0)),
        rule::horizontal(1),
    ]
    .align_x(Alignment::Center)
    .into()
}

pub(crate) fn settings_sidebar_item(active: bool) -> Element<'static, Message> {
    column![
        Space::new()
            .width(Length::Shrink)
            .height(Length::Fixed(1.0)),
        rule::horizontal(1),
        Space::new()
            .width(Length::Shrink)
            .height(Length::Fixed(5.0)),
        round_nav_button("gear", active).on_press(Message::ActivateSettings),
        Space::new()
            .width(Length::Shrink)
            .height(Length::Fixed(3.0)),
    ]
    .align_x(Alignment::Center)
    .into()
}

pub(crate) fn round_nav_button<'a>(img_key: &str, active: bool) -> Button<'a, Message> {
    let tint_color = if active {
        Color::WHITE
    } else {
        Color::from_rgb8(120, 120, 120)
    };

    let is_pipeweaver = img_key == "pipeweaver";
    let icon_size = if is_pipeweaver { 36.0 } else { 20.0 };

    let icon_content: Element<'a, Message> = if let Some(svg_handle) = SVG.get(img_key) {
        svg(svg_handle.clone())
            .width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size))
            .content_fit(iced::ContentFit::Contain)
            .style(move |_theme: &Theme, _status: svg::Status| svg::Style {
                color: if is_pipeweaver {
                    None
                } else {
                    Some(tint_color)
                },
            })
            .into()
    } else {
        Space::new().into()
    };

    let centered_content = container(icon_content)
        .width(Length::Fixed(40.0))
        .height(Length::Fixed(40.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    // Assemble the button with your precise state colors mapped
    button(centered_content)
        .width(Length::Fixed(40.0))
        .height(Length::Fixed(40.0))
        .padding(0)
        .style(move |theme: &Theme, status: button::Status| svg_button_style(theme, status, active))
}
