use crate::ui::widgets::numeric::drag_value::DragValue;
use emath::Numeric;
use iced::widget::text_input;
use iced::{Alignment, Background, Color, Length};
use std::ops::RangeInclusive;

pub fn styled_drag_value<'a, Num, Message>(
    value: Num,
    range: RangeInclusive<Num>,
) -> DragValue<'a, Num, Message>
where
    Num: Numeric,
    Message: Clone,
{
    let drag_speed = drag_speed_from_range(&range, 150);
    let drag = DragValue::new(value)
        .style(|theme, status| {
            let mut style = text_input::default(theme, status);
            style.value = Color::from_rgb8(180, 180, 180);
            style.background = Background::Color(Color::from_rgb8(60, 60, 60));

            style
        })
        .range(range.clone())
        .speed(drag_speed)
        .padding(2.0)
        .width(Length::Fill)
        .align_x(Alignment::Center);

    if Num::INTEGRAL {
        drag
    } else {
        drag.fixed_decimals(1)
    }
}

fn drag_speed_from_range<T>(range: &RangeInclusive<T>, steps: usize) -> f64
where
    T: Numeric,
{
    // Calculate our base speed ((end - start) / steps)
    let span = (range.end().to_f64() - range.start().to_f64()).abs();
    let base_speed = span / steps as f64;

    // Make sure we still function on tiny ranges (ex 0 -> 0.0001, where the span would be 0)
    let minimum_speed = base_speed.max(10f64.powf(span.log10().floor() - 4.0));
    base_speed.max(minimum_speed).clamp(1e-10, 100.0)
}
