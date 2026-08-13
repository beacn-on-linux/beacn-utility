use crate::ui::widgets::helpers::drag_value::styled_drag_value;
use crate::ui::widgets::helpers::slider::themed_slider;
use emath::Numeric;
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Element, Length};
use std::ops::RangeInclusive;

pub fn draw_range<'a, T, V>(
    title: &'a str,
    value: V,
    range: RangeInclusive<V>,
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
    let mut slider = themed_slider(range.clone(), value, slider_on_change).vertical();

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

    container(layout).width(85).height(Length::Fill).into()
}

pub fn draw_horizontal_range<'a, T, V>(
    title: &'a str,
    value: V,
    range: RangeInclusive<V>,
    suffix: &'a str,
    on_change: impl Fn(V) -> T + Clone + 'a,
) -> Element<'a, T>
where
    T: Clone + 'a,
    V: Numeric,
{
    // Ok, lets build the components first.
    let (title, title_spacer) = if title.is_empty() {
        (text(""), Space::new().width(0.0))
    } else {
        (
            text(format!("{title}:")).align_x(Alignment::End).width(60),
            Space::new().width(10.0),
        )
    };

    let slider_on_change = on_change.clone();
    let mut slider = themed_slider(range.clone(), value, slider_on_change);

    // If we're a float, force it to 1 decimal place.
    if !V::INTEGRAL {
        slider = slider.max_decimals(1);
    }

    let slider_spacer = Space::new().width(10.0);
    let input = styled_drag_value(value, range)
        .width(Length::Fixed(60.0))
        .suffix(suffix)
        .on_change(on_change);

    // Build the layout
    let layout = row![title, title_spacer, slider, slider_spacer, input]
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center);

    container(layout)
        .width(Length::Fill)
        .height(Length::Shrink)
        .into()
}

pub fn draw_lighting_range<'a, T, V>(
    title: &'a str,
    value: V,
    range: RangeInclusive<V>,
    suffix: &'a str,
    title_width: Length,
    on_change: impl Fn(V) -> T + Clone + 'a,
) -> Element<'a, T>
where
    T: Clone + 'a,
    V: Numeric,
{
    // We just pass through to a titleless version of the slider
    let title = text(format!("{title}:")).width(title_width);
    let title_spacer = Space::new().width(10.0);

    let slider = draw_horizontal_range("", value, range, suffix, on_change);

    row![title, title_spacer, slider]
        .align_y(Alignment::Center)
        .into()
}
