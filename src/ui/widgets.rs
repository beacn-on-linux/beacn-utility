use crate::ui::SVG;
use crate::ui::numbers::NumericType;
use egui::emath::Numeric;
use egui::{
    Align, Button, Color32, CornerRadius, DragValue, Image, Layout, Response, RichText, Slider, Ui,
    Visuals, vec2,
};

use std::fmt::Debug;
use std::ops::RangeInclusive;

pub fn round_nav_button(ui: &mut Ui, img: &str, active: bool) -> Response {
    let tint_colour = if active {
        Color32::WHITE
    } else {
        Color32::from_rgb(120, 120, 120)
    };

    // We might need to do caching here..
    let image = SVG.get(img).unwrap().clone();

    ui.scope(|ui| {
        ui.style_mut().spacing.button_padding = vec2(0.0, 0.0);
        ui.add_sized(
            [40.0, 40.0],
            Button::image(
                Image::new(image)
                    .tint(tint_colour)
                    .fit_to_exact_size(vec2(20., 20.)),
            )
            .corner_radius(CornerRadius::same(5))
            .selected(active),
        )
    })
    .inner
}

// So the pipeweaver button is the same as a basic button, but because it's already coloured
// we don't need to add a tint to it, and also because of it's size we need far less padding
pub fn pipeweaver_button(ui: &mut Ui, img: &str, active: bool) -> Response {
    // We might need to do caching here..
    let image = SVG.get(img).unwrap().clone();

    ui.scope(|ui| {
        ui.style_mut().spacing.button_padding = vec2(0.0, 0.0);
        ui.add_sized(
            [40.0, 40.0],
            Button::image(Image::new(image).fit_to_exact_size(vec2(35., 35.)))
                .corner_radius(CornerRadius::same(5))
                .selected(active),
        )
    })
    .inner
}

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
                ui.label(format!("{title}: "));
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

/// Create a slider which has a trail moving from a fixed position
pub fn zero_trail_slider(
    ui: &mut Ui,
    value: &mut i32,
    range: RangeInclusive<i32>,
    trail_origin: i32,
) -> Response {
    let min = *range.start();
    let max = *range.end();

    let desired_size = egui::vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);

    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let response = ui.put(rect, egui::Slider::new(value, range).show_value(false));

    let current_value = *value;

    // Use the slider response rect, not the allocation rect.
    let slider_rect = response.rect;

    let y = slider_rect.center().y;

    // Match egui handle sizing.
    let handle_radius = slider_rect.height() / 2.5;

    let visuals = if response.dragged() {
        &ui.visuals().widgets.active
    } else if response.hovered() {
        &ui.visuals().widgets.hovered
    } else {
        &ui.visuals().widgets.inactive
    };

    let handle_half_width = match ui.visuals().handle_shape {
        egui::style::HandleShape::Circle => handle_radius,

        egui::style::HandleShape::Rect { aspect_ratio } => handle_radius * aspect_ratio,
    } + visuals.expansion;

    let travel_left = slider_rect.left() + handle_half_width;
    let travel_right = slider_rect.right() - handle_half_width;

    let to_x = |v: i32| -> f32 {
        egui::remap(
            v as f32,
            min as f32..=max as f32,
            travel_left..=travel_right,
        )
    };

    let origin_x = to_x(trail_origin);
    let value_x = to_x(current_value);

    let track_height = ui.spacing().slider_rail_height;

    let trail_rect = egui::Rect::from_min_max(
        egui::pos2(origin_x.min(value_x), y - track_height * 0.5),
        egui::pos2(origin_x.max(value_x), y + track_height * 0.5),
    );

    let handle_left = value_x - handle_half_width;
    let handle_right = value_x + handle_half_width;

    let painter = ui.painter();
    let color = ui.visuals().selection.bg_fill;

    if current_value > trail_origin {
        let clip =
            egui::Rect::from_min_max(trail_rect.min, egui::pos2(handle_left, trail_rect.max.y));

        if clip.width() > 0.0 {
            painter
                .with_clip_rect(clip)
                .rect_filled(trail_rect, 0.0, color);
        }
    } else if current_value < trail_origin {
        let clip =
            egui::Rect::from_min_max(egui::pos2(handle_right, trail_rect.min.y), trail_rect.max);

        if clip.width() > 0.0 {
            painter
                .with_clip_rect(clip)
                .rect_filled(trail_rect, 0.0, color);
        }
    }

    response
}
