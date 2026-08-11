use crate::ui_iced::widgets::helpers::drag_value::styled_drag_value;
use crate::ui_iced::widgets::numeric::slider::Slider;
use emath::Numeric;
use iced::widget::slider::Status;
use iced::widget::{Space, column, container, slider, text};
use iced::{Alignment, Background, Color, Element, Length};
use std::ops::RangeInclusive;

pub fn draw_range<'a, T, V>(
    title: &'a str,
    value: V,
    range: RangeInclusive<V>,
    step: V,
    suffix: &'a str,
    on_change: impl Fn(V) -> T + Clone + 'a,
) -> Element<'a, T>
where
    T: Clone + 'a,
    V: Numeric,
{
    // Ok, lets build the components first.
    let title = text(title);
    let title_spacer = Space::new().height(8.0);

    let slider_on_change = on_change.clone();
    let mut slider = Slider::new(range.clone(), value, slider_on_change)
        .style(|theme, status| {
            let mut style = slider::default(theme, status);

            style.handle.shape = slider::HandleShape::Rectangle {
                width: 12,
                border_radius: 2.0.into(),
            };

            style.handle.border_width = match status {
                Status::Active => 1.0,
                Status::Hovered | Status::Dragged => 2.0,
            };
            style.handle.border_color = Color::from_rgb8(180, 180, 180);
            style.handle.background = Background::Color(Color::from_rgb8(60, 60, 60));

            style.rail.width = 8.0;
            style
        })
        .vertical()
        .step(step);

    // If we're a float, force it to 1 decimal place.
    if !V::INTEGRAL {
        slider = slider.max_decimals(1);
    }

    let slider_spacer = Space::new().height(10.0);
    let input = styled_drag_value(value, range)
        .suffix(suffix)
        .on_change(on_change);

    // Build the layout
    let layout = column![title, title_spacer, slider, slider_spacer, input]
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center);

    container(layout).width(80).height(Length::Fill).into()
}
