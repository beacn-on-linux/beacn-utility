use iced::widget::button::Status;
use iced::widget::{Button, button, text};
use iced::{Alignment, Border, Color, Length, Theme};

pub fn toggle_button<T>(value: &str, active: bool) -> Button<'_, T> {
    let text_colour = if active {
        Color::WHITE
    } else {
        Color::from_rgb8(120, 120, 120)
    };

    let text = text(value)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .color(text_colour)
        .width(Length::Fill)
        .height(Length::Fill);

    button(text).style(move |theme: &Theme, status: Status| {
        let palette = theme.palette();
        let bg_color = if active {
            palette.primary
        } else {
            match status {
                Status::Hovered | Status::Pressed => Color::from_rgb8(0x46, 0x46, 0x46),
                _ => Color::from_rgb8(0x3C, 0x3C, 0x3C),
            }
        };

        let border_style = match status {
            Status::Hovered if !active => Border {
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
    })
}
