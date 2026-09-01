use crate::ui::SVG;
use iced::widget::{Button, Space, button, container, svg};
use iced::{Alignment, Border, Color, Element, Length, Theme};

pub fn svg_button<'a, T>(svg: &'static str, active: bool) -> Button<'a, T>
where
    T: Clone + 'a,
{
    let tint_color = if active {
        Color::WHITE
    } else {
        Color::from_rgb8(120, 120, 120)
    };

    let icon_content: Element<'a, T> = if let Some(svg_handle) = SVG.get(svg) {
        iced::widget::svg(svg_handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Contain)
            .style(move |_theme: &Theme, _status: svg::Status| svg::Style {
                color: Some(tint_color),
            })
            .into()
    } else {
        Space::new().into()
    };

    let centered_content = container(icon_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    // Assemble the button with your precise state colors mapped
    button(centered_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(4)
        .style(move |theme: &Theme, status: button::Status| svg_button_style(theme, status, active))
}

pub fn svg_button_style(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let palette = theme.palette();
    let bg_color = if active {
        palette.primary
    } else {
        match status {
            button::Status::Hovered | button::Status::Pressed => Color::from_rgb8(0x46, 0x46, 0x46),
            _ => Color::from_rgb8(0x3C, 0x3C, 0x3C),
        }
    };

    // Map your custom border stroke rule selection
    let border_style = match status {
        button::Status::Hovered if !active => Border {
            radius: 5.0.into(),
            width: 1.0,
            color: Color::from_rgb8(0x96, 0x96, 0x96),
        },
        _ => Border {
            radius: 5.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
    };

    button::Style {
        background: Some(bg_color.into()),
        text_color: Color::TRANSPARENT,
        border: border_style,
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

pub fn svg_button_unstyled<'a, T>(svg: &'static str) -> Button<'a, T>
where
    T: Clone + 'a,
{
    let icon_content: Element<'a, T> = if let Some(svg_handle) = SVG.get(svg) {
        iced::widget::svg(svg_handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |theme: &Theme, _status: svg::Status| svg::Style {
                color: Some(theme.palette().text),
            })
            .into()
    } else {
        Space::new().into()
    };

    let centered_content = container(icon_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    button(centered_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .style(move |_: &Theme, _: button::Status| button::Style {
            ..Default::default()
        })
}

pub fn svg_coloured_button_unstyled<'a, T>(svg: &'static str, svg_colour: Color) -> Button<'a, T>
where
    T: Clone + 'a,
{
    let icon_content: Element<'a, T> = if let Some(svg_handle) = SVG.get(svg) {
        iced::widget::svg(svg_handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme, _status: svg::Status| svg::Style {
                color: Some(svg_colour),
            })
            .into()
    } else {
        Space::new().into()
    };

    let centered_content = container(icon_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    button(centered_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .style(move |_: &Theme, _: button::Status| button::Style {
            ..Default::default()
        })
}
