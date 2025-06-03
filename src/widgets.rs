use crate::numbers::NumericType;
use egui::emath::Numeric;
use egui::{Align, DragValue, Layout, Response, RichText, Slider, Ui, Visuals};
use std::fmt::Debug;
use std::ops::RangeInclusive;

pub fn draw_range<T>(
    ui: &mut Ui,
    value: &mut T,
    range: RangeInclusive<T>,
    title: &str,
    suffix: &str,
) -> bool
where
    T: Copy + Numeric + Debug + NumericType,
{
    // The component ID (we'll use the title, at least for now!)
    //let id = ui.id().with(title);

    // Prepare the output
    let mut changed = false;
    ui.add_sized([80.0, ui.available_height()], |ui: &mut egui::Ui| {
        ui.vertical_centered(|ui| {
            // Title above the field
            ui.label(title);
            ui.add_space(5.0);

            let slider_response = ui
                .scope(|ui| {
                    ui.style_mut().spacing.slider_width = ui.available_height() - 32.0;

                    let mut slider = Slider::new(value, range.clone())
                        .vertical()
                        .suffix(suffix)
                        .trailing_fill(true)
                        .show_value(false);

                    if T::IS_FLOAT {
                        slider = slider.fixed_decimals(1);
                    }

                    ui.add_sized([20.0, ui.available_height()], slider)
                })
                .inner;
            if slider_response.changed() {
                changed = true;
            }

            ui.add_space(10.0);

            let drag_speed = drag_speed_from_range(&range, 150);
            let mut drag = DragValue::new(value)
                .range(range.clone())
                .speed(drag_speed)
                .suffix(suffix);

            if T::IS_FLOAT {
                drag = drag.fixed_decimals(1);
            }

            let drag_response = ui.add_sized([ui.available_width(), 0.0], drag);
            if drag_response.changed() {
                changed = true;
            }
        })
        .response
    });

    changed
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

pub fn toggle_button<'a>(ui: &mut Ui, active: bool, label: &str) -> egui::Button<'a> {
    let visuals: &Visuals = &ui.style().visuals;

    let bg_color = if active {
        visuals.selection.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };

    let text_color = if active {
        visuals.selection.stroke.color
    } else {
        visuals.widgets.inactive.fg_stroke.color
    };

    egui::Button::new(RichText::new(label).color(text_color)).fill(bg_color)
}

pub fn draw_draggable<'a, T>(
    value: &'a mut T,
    range: RangeInclusive<T>,
    suffix: &str,
) -> DragValue<'a>
where
    T: Copy + Numeric + Debug + NumericType,
{
    let drag_speed = drag_speed_from_range(&range, 150);
    let mut drag = DragValue::new(value)
        .range(range.clone())
        .speed(drag_speed)
        .suffix(suffix);

    if T::IS_FLOAT {
        drag = drag.fixed_decimals(1);
    }

    // let drag_response = ui.add_sized([75.0, 20.0], drag);
    // drag_response.changed()
    drag
}

pub fn get_slider<T>(
    ui: &mut Ui,
    title: &str,
    suffix: &str,
    value: &mut T,
    range: RangeInclusive<T>,
) -> Response
where
    T: Numeric + NumericType,
{
    ui.horizontal_centered(|ui| {
        ui.add_sized([60.0, 0.], |ui: &mut Ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui: &mut Ui| {
                ui.label(format!("{}: ", title));
            })
            .response
        });
        let mut slider = Slider::new(value, range).suffix(suffix).trailing_fill(true);
        if T::IS_FLOAT {
            slider = slider.fixed_decimals(1);
        }
        ui.add(slider)
    })
    .inner
}
