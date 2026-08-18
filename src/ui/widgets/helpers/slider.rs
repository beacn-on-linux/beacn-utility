use crate::ui::widgets::numeric::slider::Slider;
use emath::Numeric;
use iced::widget::slider;
use iced::widget::slider::{Status, Style};
use iced::{Background, Color, Theme};
use std::ops::RangeInclusive;

pub fn themed_slider<'a, T, Num>(
    range: RangeInclusive<Num>,
    value: Num,
    on_change: impl Fn(Num) -> T + Clone + 'a,
) -> Slider<'a, Num, T>
where
    T: Clone + 'a,
    Num: Numeric,
{
    Slider::new(range.clone(), value, on_change).style(slider_theme)
}

// Exported so custom sliders can base their theme on this
// https://www.youtube.com/watch?v=f64nXt1z4XU
pub fn slider_theme(theme: &Theme, status: Status) -> Style {
    let mut style = slider::default(theme, status);

    style.handle.shape = slider::HandleShape::Rectangle {
        width: 12,
        border_radius: 2.5.into(),
    };

    style.handle.border_width = match status {
        Status::Active => 1.0,
        Status::Hovered | Status::Dragged => 2.0,
    };
    style.handle.border_color = Color::from_rgb8(180, 180, 180);
    style.handle.background = Background::Color(Color::from_rgb8(60, 60, 60));

    style.rail.width = 8.0;
    style
}
