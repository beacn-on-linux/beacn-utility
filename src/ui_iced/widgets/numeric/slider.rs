//! This is a port of egui's 'slider' balanced with iced's 'slider' with a tiny bit of Frosty's
//! 'slider', feature differences between this and the stock iced slider are:
//!
//! * Better snapping to 'round' values
//! * Logarithmic slider values
//! * Much easier Horizontal / Vertical Flip
//! * `trail_start` allowing the trail to start in the middle of the groove
//! * No forced clamping, allows the value to be outside the range without trying to fix

use std::ops::RangeInclusive;
use std::rc::Rc;

use crate::ui_iced::widgets::numeric::{clamp_to_range, to_num};
use emath::Numeric;
use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, mouse, renderer};
use iced::keyboard::{self, key};
use iced::widget::slider as iced_slider;
use iced::{Element, Event, Length, Rectangle, Size};

/// What to do when the incoming value is outside the defined range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clamping {
    /// Incoming values outside the range are never clamped.
    Never,

    /// Don't clamp initially, but if changed, clamp them
    Edits,

    /// Immediately clamp the incoming value
    #[default]
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

pub struct Slider<'a, Num, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Num: Numeric,
    Theme: iced_slider::Catalog,
    Renderer: renderer::Renderer,
{
    value: f64,
    range: RangeInclusive<f64>,
    clamping: Clamping,
    orientation: Orientation,
    step: Option<f64>,
    smart_aim: bool,
    logarithmic: bool,
    smallest_positive: f64,
    largest_finite: f64,
    aim_radius: f64,
    trail_start: Option<f64>,
    max_decimals: Option<usize>,
    length: Length,
    thickness: f32,
    on_change: Rc<dyn Fn(Num) -> Message + 'a>,
    class: <Theme as iced_slider::Catalog>::Class<'a>,
    _renderer: std::marker::PhantomData<Renderer>,
}

impl<'a, Num, Message, Theme, Renderer> Slider<'a, Num, Message, Theme, Renderer>
where
    Num: Numeric,
    Theme: iced_slider::Catalog,
    Renderer: renderer::Renderer,
{
    pub const DEFAULT_THICKNESS: f32 = 16.0;

    /// Creates a Slider
    pub fn new(
        range: RangeInclusive<Num>,
        value: Num,
        on_change: impl Fn(Num) -> Message + 'a,
    ) -> Self {
        let range = range.start().to_f64()..=range.end().to_f64();
        let value = value.to_f64();

        let slf = Self {
            value,
            range,
            clamping: Clamping::default(),
            orientation: Orientation::default(),
            step: None,
            smart_aim: true,
            logarithmic: false,
            smallest_positive: 1e-6,
            largest_finite: f64::INFINITY,
            aim_radius: 1.0,
            trail_start: None,
            max_decimals: None,
            length: Length::Fill,
            thickness: Self::DEFAULT_THICKNESS,
            on_change: Rc::new(on_change),
            class: <Theme as iced_slider::Catalog>::default(),
            _renderer: std::marker::PhantomData,
        };

        if Num::INTEGRAL {
            slf.smallest_positive(Num::from_f64(1.0))
                .step(Num::from_f64(1.0))
        } else {
            slf
        }
    }

    /// Sets the clamping behaviour
    pub fn clamping(mut self, clamping: Clamping) -> Self {
        self.clamping = clamping;
        self
    }

    /// Sets the orientation
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Shorthand for `.orientation(Orientation::Vertical)`
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Stepping to Clamp to
    pub fn step(mut self, step: Num) -> Self {
        self.step = Some(step.to_f64());
        self
    }

    /// Snap tighter to 'round' numbers
    pub fn smart_aim(mut self, smart_aim: bool) -> Self {
        self.smart_aim = smart_aim;
        self
    }

    /// Makes logarithmic
    pub fn logarithmic(mut self, logarithmic: bool) -> Self {
        self.logarithmic = logarithmic;
        self
    }

    /// Smallest positive non-zero value for logarithmic sliders
    pub fn smallest_positive(mut self, smallest_positive: Num) -> Self {
        self.smallest_positive = smallest_positive.to_f64();
        self
    }

    /// Largest finite value for logarithmic sliders.
    pub fn largest_finite(mut self, largest_finite: Num) -> Self {
        self.largest_finite = largest_finite.to_f64();
        self
    }

    /// Overrides the radius for smart aim
    pub fn aim_radius(mut self, aim_radius: f64) -> Self {
        self.aim_radius = aim_radius;
        self
    }

    /// At which value to show the trail from? (Allowing dual ended sliders)
    pub fn trail_start(mut self, value: Num) -> Self {
        self.trail_start = Some(value.to_f64());
        self
    }

    /// Maximum decimals shown/rounded to
    pub fn max_decimals(mut self, max_decimals: usize) -> Self {
        self.max_decimals = Some(max_decimals);
        self
    }

    /// Sets the widget Length
    pub fn length(mut self, length: impl Into<Length>) -> Self {
        self.length = length.into();
        self
    }

    /// How Thicc is the track
    pub fn thickness(mut self, thickness: impl Into<iced::Pixels>) -> Self {
        self.thickness = thickness.into().0;
        self
    }

    /// Styling (Uses iced's slider style)
    #[must_use]
    pub fn style(
        mut self,
        style: impl Fn(&Theme, iced_slider::Status) -> iced_slider::Style + 'a,
    ) -> Self
    where
        <Theme as iced_slider::Catalog>::Class<'a>: From<iced_slider::StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as iced_slider::StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the track.
    #[must_use]
    pub fn class(mut self, class: impl Into<<Theme as iced_slider::Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn log_spec(&self) -> LogSpec {
        LogSpec {
            logarithmic: self.logarithmic,
            smallest_positive: self.smallest_positive,
            largest_finite: self.largest_finite,
        }
    }

    /// The effenctive value of the slider (with clamping behaviour)
    fn effective_value(&self) -> f64 {
        if self.clamping == Clamping::Always {
            clamp_to_range(self.value, &self.range)
        } else {
            self.value
        }
    }

    /// Resolves value to the best value matching the configuration
    fn resolve_value(&self, mut value: f64) -> f64 {
        if let Some(step) = self.step {
            let start = *self.range.start();
            value = start + ((value - start) / step).round() * step;
        }

        if let Some(max_decimals) = self.max_decimals {
            value = emath::round_to_decimals(value, max_decimals);
        }

        if self.clamping != Clamping::Never {
            value = clamp_to_range(value, &self.range);
        }

        value
    }

    fn value_from_position(&self, position: f32, position_range: (f32, f32)) -> f64 {
        let normalized = f64::from(remap_clamp(position, position_range, (0.0, 1.0)));
        value_from_normalized(normalized, &self.range, &self.log_spec())
    }

    fn position_from_value(&self, value: f64, position_range: (f32, f32)) -> f32 {
        let normalized = normalized_from_value(value, &self.range, &self.log_spec());
        lerp(position_range, normalized as f32)
    }

    fn commit(&self, new_value: f64, shell: &mut Shell<'_, Message>) {
        let resolved = self.resolve_value(new_value);
        if resolved != self.effective_value() {
            shell.publish((self.on_change)(to_num(resolved)));
        }
    }

    fn status(&self, state: &State) -> iced_slider::Status {
        if state.is_dragging {
            iced_slider::Status::Dragged
        } else if state.is_hovered {
            iced_slider::Status::Hovered
        } else {
            iced_slider::Status::Active
        }
    }
}

/// Shorthand constructor, mirroring iced's `slider(...)`
pub fn slider<'a, Num, Message, Theme, Renderer>(
    range: RangeInclusive<Num>,
    value: Num,
    on_change: impl Fn(Num) -> Message + 'a,
) -> Slider<'a, Num, Message, Theme, Renderer>
where
    Num: Numeric,
    Theme: iced_slider::Catalog,
    Renderer: renderer::Renderer,
{
    Slider::new(range, value, on_change)
}

#[derive(Default)]
struct State {
    is_dragging: bool,
    is_focused: bool,
    is_hovered: bool,
    modifiers: keyboard::Modifiers,
}

impl<'a, Num, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Slider<'a, Num, Message, Theme, Renderer>
where
    Num: Numeric,
    Theme: iced_slider::Catalog,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        match self.orientation {
            Orientation::Horizontal => Size {
                width: self.length,
                height: Length::Fixed(self.thickness),
            },
            Orientation::Vertical => Size {
                width: Length::Fixed(self.thickness),
                height: self.length,
            },
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        match self.orientation {
            Orientation::Horizontal => layout::atomic(limits, self.length, self.thickness),
            Orientation::Vertical => layout::atomic(limits, self.thickness, self.length),
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        struct SliderFocusable<'s> {
            state: &'s mut State,
        }

        impl widget::operation::Focusable for SliderFocusable<'_> {
            fn is_focused(&self) -> bool {
                self.state.is_focused
            }

            fn focus(&mut self) {
                self.state.is_focused = true;
            }

            fn unfocus(&mut self) {
                self.state.is_focused = false;
            }
        }

        operation.focusable(None, bounds, &mut SliderFocusable { state });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<State>();

        let position_range = match self.orientation {
            Orientation::Horizontal => (bounds.x, bounds.x + bounds.width),
            Orientation::Vertical => (bounds.y + bounds.height, bounds.y),
        };
        let axis = |point: iced::Point| match self.orientation {
            Orientation::Horizontal => point.x,
            Orientation::Vertical => point.y,
        };
        let axis_sign: f32 = match self.orientation {
            Orientation::Horizontal => 1.0,
            Orientation::Vertical => -1.0,
        };

        if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
            let now_hovered = cursor.is_over(bounds);
            if now_hovered != state.is_hovered {
                state.is_hovered = now_hovered;
                shell.request_redraw();
            }
        }

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = *modifiers;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if let Some(position) = cursor.position_over(bounds) {
                    let new_value = self.locate_with_aim(axis(position), position_range);
                    self.commit(new_value, shell);
                    state.is_dragging = true;
                    state.is_focused = true;
                    shell.capture_event();
                } else if state.is_focused {
                    state.is_focused = false;
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(iced::touch::Event::FingerMoved { .. }) => {
                if state.is_dragging {
                    if let Some(position) = cursor.land().position() {
                        let new_value = self.locate_with_aim(axis(position), position_range);
                        self.commit(new_value, shell);
                    }
                    shell.capture_event();
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(iced::touch::Event::FingerLifted { .. })
            | Event::Touch(iced::touch::Event::FingerLost { .. }) => {
                if state.is_dragging {
                    state.is_dragging = false;
                    shell.capture_event();
                }
            }

            Event::Keyboard(keyboard::Event::KeyPressed {
                key: pressed_key, ..
            }) if state.is_focused => {
                let step_sign = match (self.orientation, pressed_key) {
                    (Orientation::Horizontal, keyboard::Key::Named(key::Named::ArrowRight)) => {
                        Some(1.0)
                    }
                    (Orientation::Horizontal, keyboard::Key::Named(key::Named::ArrowLeft)) => {
                        Some(-1.0)
                    }
                    (Orientation::Vertical, keyboard::Key::Named(key::Named::ArrowUp)) => Some(1.0),
                    (Orientation::Vertical, keyboard::Key::Named(key::Named::ArrowDown)) => {
                        Some(-1.0)
                    }
                    _ => None,
                };

                if let Some(kb_step) = step_sign {
                    let prev_value = self.effective_value();
                    let prev_position = self.position_from_value(prev_value, position_range);
                    let new_position = prev_position + kb_step as f32 * axis_sign;

                    let mut new_value = match self.step {
                        Some(step) => prev_value + kb_step * step,
                        None if self.smart_aim => {
                            let aim_delta = 0.49;
                            emath::smart_aim::best_in_range_f64(
                                self.value_from_position(new_position - aim_delta, position_range),
                                self.value_from_position(new_position + aim_delta, position_range),
                            )
                        }
                        _ => self.value_from_position(new_position, position_range),
                    };

                    if let Some(max_decimals) = self.max_decimals {
                        let min_increment = 1.0 / 10.0_f64.powi(max_decimals as i32);
                        new_value = if new_value > prev_value {
                            f64::max(new_value, prev_value + min_increment * 1.001)
                        } else if new_value < prev_value {
                            f64::min(new_value, prev_value - min_increment * 1.001)
                        } else {
                            new_value
                        };
                    }

                    self.commit(new_value, shell);
                    shell.capture_event();
                }
            }

            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();

        if state.is_dragging {
            if cfg!(target_os = "windows") {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Grabbing
            }
        } else if cursor.is_over(layout.bounds()) {
            if cfg!(target_os = "windows") {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Grab
            }
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        let style = <Theme as iced_slider::Catalog>::style(theme, &self.class, self.status(state));

        let value = self.effective_value();
        let normalized =
            (normalized_from_value(value, &self.range, &self.log_spec()) as f32).clamp(0.0, 1.0);

        match self.orientation {
            Orientation::Horizontal => {
                let (handle_main, handle_cross, handle_border_radius) = match style.handle.shape {
                    iced_slider::HandleShape::Circle { radius } => {
                        (radius * 2.0, radius * 2.0, radius.into())
                    }
                    iced_slider::HandleShape::Rectangle {
                        width,
                        border_radius,
                    } => (f32::from(width), bounds.height, border_radius),
                };

                let split =
                    |t: f32| (bounds.width - handle_main) * t.clamp(0.0, 1.0) + handle_main / 2.0;

                let split_at_value = split(normalized);
                let rail_y = bounds.y + bounds.height / 2.0;

                let rail_border = |left: bool, right: bool| iced::Border {
                    radius: iced::border::Radius {
                        top_left: if left {
                            style.rail.border.radius.top_left
                        } else {
                            0.0
                        },
                        bottom_left: if left {
                            style.rail.border.radius.bottom_left
                        } else {
                            0.0
                        },
                        top_right: if right {
                            style.rail.border.radius.top_right
                        } else {
                            0.0
                        },
                        bottom_right: if right {
                            style.rail.border.radius.bottom_right
                        } else {
                            0.0
                        },
                    },
                    ..style.rail.border
                };

                let rail_quad = |x: f32, width: f32, border: iced::Border| renderer::Quad {
                    bounds: Rectangle {
                        x,
                        y: rail_y - style.rail.width / 2.0,
                        width,
                        height: style.rail.width,
                    },
                    border,
                    ..renderer::Quad::default()
                };

                if let Some(trail_start) = self.trail_start {
                    let normalized_trail =
                        normalized_from_value(trail_start, &self.range, &self.log_spec()) as f32;
                    let split_at_trail = split(normalized_trail);

                    if split_at_trail == split_at_value {
                        renderer.fill_quad(
                            rail_quad(bounds.x, bounds.width, rail_border(true, true)),
                            style.rail.backgrounds.1,
                        );
                    } else {
                        let (lo, hi) = (
                            split_at_value.min(split_at_trail),
                            split_at_value.max(split_at_trail),
                        );
                        renderer.fill_quad(
                            rail_quad(bounds.x, lo, rail_border(true, false)),
                            style.rail.backgrounds.1,
                        );
                        renderer.fill_quad(
                            rail_quad(bounds.x + lo, hi - lo, rail_border(false, false)),
                            style.rail.backgrounds.0,
                        );
                        renderer.fill_quad(
                            rail_quad(bounds.x + hi, bounds.width - hi, rail_border(false, true)),
                            style.rail.backgrounds.1,
                        );
                    }
                } else {
                    renderer.fill_quad(
                        rail_quad(bounds.x, split_at_value, rail_border(true, false)),
                        style.rail.backgrounds.0,
                    );
                    renderer.fill_quad(
                        rail_quad(
                            bounds.x + split_at_value,
                            bounds.width - split_at_value,
                            rail_border(false, true),
                        ),
                        style.rail.backgrounds.1,
                    );
                }

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.x + split_at_value - handle_main / 2.0,
                            y: rail_y - handle_cross / 2.0,
                            width: handle_main,
                            height: handle_cross,
                        },
                        border: iced::Border {
                            radius: handle_border_radius,
                            width: style.handle.border_width,
                            color: style.handle.border_color,
                        },
                        ..renderer::Quad::default()
                    },
                    style.handle.background,
                );
            }

            Orientation::Vertical => {
                let (handle_main, handle_cross, handle_border_radius) = match style.handle.shape {
                    iced_slider::HandleShape::Circle { radius } => {
                        (radius * 2.0, radius * 2.0, radius.into())
                    }
                    iced_slider::HandleShape::Rectangle {
                        width,
                        border_radius,
                    } => (f32::from(width), bounds.width, border_radius),
                };

                let split_from_top = |t: f32| {
                    (bounds.height - handle_main) * (1.0 - t.clamp(0.0, 1.0)) + handle_main / 2.0
                };

                let split_at_value = split_from_top(normalized);
                let rail_x = bounds.x + bounds.width / 2.0;

                let rail_border = |top: bool, bottom: bool| iced::Border {
                    radius: iced::border::Radius {
                        top_left: if top {
                            style.rail.border.radius.top_left
                        } else {
                            0.0
                        },
                        top_right: if top {
                            style.rail.border.radius.top_right
                        } else {
                            0.0
                        },
                        bottom_left: if bottom {
                            style.rail.border.radius.bottom_left
                        } else {
                            0.0
                        },
                        bottom_right: if bottom {
                            style.rail.border.radius.bottom_right
                        } else {
                            0.0
                        },
                    },
                    ..style.rail.border
                };

                let rail_quad = |y: f32, height: f32, border: iced::Border| renderer::Quad {
                    bounds: Rectangle {
                        x: rail_x - style.rail.width / 2.0,
                        y,
                        width: style.rail.width,
                        height,
                    },
                    border,
                    ..renderer::Quad::default()
                };

                if let Some(trail_start) = self.trail_start {
                    let normalized_trail =
                        normalized_from_value(trail_start, &self.range, &self.log_spec()) as f32;
                    let split_at_trail = split_from_top(normalized_trail);

                    if split_at_trail == split_at_value {
                        renderer.fill_quad(
                            rail_quad(bounds.y, bounds.height, rail_border(true, true)),
                            style.rail.backgrounds.1,
                        );
                    } else {
                        let (lo, hi) = (
                            split_at_value.min(split_at_trail),
                            split_at_value.max(split_at_trail),
                        );
                        renderer.fill_quad(
                            rail_quad(bounds.y, lo, rail_border(true, false)),
                            style.rail.backgrounds.1,
                        );
                        renderer.fill_quad(
                            rail_quad(bounds.y + lo, hi - lo, rail_border(false, false)),
                            style.rail.backgrounds.0,
                        );
                        renderer.fill_quad(
                            rail_quad(bounds.y + hi, bounds.height - hi, rail_border(false, true)),
                            style.rail.backgrounds.1,
                        );
                    }
                } else {
                    // Unfilled: from the top down to the handle.
                    renderer.fill_quad(
                        rail_quad(bounds.y, split_at_value, rail_border(true, false)),
                        style.rail.backgrounds.1,
                    );

                    // Filled: from the handle down to the bottom.
                    renderer.fill_quad(
                        rail_quad(
                            bounds.y + split_at_value,
                            bounds.height - split_at_value,
                            rail_border(false, true),
                        ),
                        style.rail.backgrounds.0,
                    );
                }

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: rail_x - handle_cross / 2.0,
                            y: bounds.y + split_at_value - handle_main / 2.0,
                            width: handle_cross,
                            height: handle_main,
                        },
                        border: iced::Border {
                            radius: handle_border_radius,
                            width: style.handle.border_width,
                            color: style.handle.border_color,
                        },
                        ..renderer::Quad::default()
                    },
                    style.handle.background,
                );
            }
        }
    }
}

impl<'a, Num, Message, Theme, Renderer> Slider<'a, Num, Message, Theme, Renderer>
where
    Num: Numeric,
    Theme: iced_slider::Catalog,
    Renderer: renderer::Renderer,
{
    fn locate_with_aim(&self, x: f32, position_range: (f32, f32)) -> f64 {
        if self.smart_aim {
            let aim_radius = self.aim_radius as f32;
            emath::smart_aim::best_in_range_f64(
                self.value_from_position(x - aim_radius, position_range),
                self.value_from_position(x + aim_radius, position_range),
            )
        } else {
            self.value_from_position(x, position_range)
        }
    }
}

impl<'a, Num, Message, Theme, Renderer> From<Slider<'a, Num, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Num: Numeric,
    Message: 'a,
    Theme: iced_slider::Catalog + 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(slider: Slider<'a, Num, Message, Theme, Renderer>) -> Self {
        Element::new(slider)
    }
}

// Coordinate mapping (ported from `egui::widgets::slider` and `emath`)
struct LogSpec {
    logarithmic: bool,
    smallest_positive: f64,
    largest_finite: f64,
}

const INF_RANGE_MAGNITUDE: f64 = 10.0;

fn lerp(range: (f32, f32), t: f32) -> f32 {
    emath::lerp(range.0..=range.1, t)
}

fn remap_clamp(x: f32, from: (f32, f32), to: (f32, f32)) -> f32 {
    emath::remap_clamp(x, from.0..=from.1, to.0..=to.1)
}

fn remap64(x: f64, from: (f64, f64), to: (f64, f64)) -> f64 {
    emath::remap(x, from.0..=from.1, to.0..=to.1)
}

/// Direct port of `egui`'s `value_from_normalized`.
fn value_from_normalized(normalized: f64, range: &RangeInclusive<f64>, spec: &LogSpec) -> f64 {
    let (min, max) = (*range.start(), *range.end());

    if min.is_nan() || max.is_nan() {
        f64::NAN
    } else if min == max {
        min
    } else if min > max {
        value_from_normalized(1.0 - normalized, &(max..=min), spec)
    } else if normalized <= 0.0 {
        min
    } else if normalized >= 1.0 {
        max
    } else if spec.logarithmic {
        if max <= 0.0 {
            -value_from_normalized(normalized, &(-min..=-max), spec)
        } else if 0.0 <= min {
            let (min_log, max_log) = range_log10(min, max, spec);
            let log = (1.0 - normalized) * min_log + normalized * max_log;
            10.0_f64.powf(log)
        } else {
            let zero_cutoff = logarithmic_zero_cutoff(min, max);
            if normalized < zero_cutoff {
                value_from_normalized(
                    remap64(normalized, (0.0, zero_cutoff), (0.0, 1.0)),
                    &(min..=0.0),
                    spec,
                )
            } else {
                value_from_normalized(
                    remap64(normalized, (zero_cutoff, 1.0), (0.0, 1.0)),
                    &(0.0..=max),
                    spec,
                )
            }
        }
    } else {
        (1.0 - normalized.clamp(0.0, 1.0)) * min + normalized.clamp(0.0, 1.0) * max
    }
}

/// Direct port of `egui`'s `normalized_from_value`.
fn normalized_from_value(value: f64, range: &RangeInclusive<f64>, spec: &LogSpec) -> f64 {
    let (min, max) = (*range.start(), *range.end());

    if min.is_nan() || max.is_nan() {
        f64::NAN
    } else if min == max {
        0.5
    } else if min > max {
        1.0 - normalized_from_value(value, &(max..=min), spec)
    } else if value <= min {
        0.0
    } else if value >= max {
        1.0
    } else if spec.logarithmic {
        if max <= 0.0 {
            normalized_from_value(-value, &(-min..=-max), spec)
        } else if 0.0 <= min {
            let (min_log, max_log) = range_log10(min, max, spec);
            let value_log = value.log10();
            remap_clamp64(value_log, (min_log, max_log), (0.0, 1.0))
        } else {
            let zero_cutoff = logarithmic_zero_cutoff(min, max);
            if value < 0.0 {
                remap64(
                    normalized_from_value(value, &(min..=0.0), spec),
                    (0.0, 1.0),
                    (0.0, zero_cutoff),
                )
            } else {
                remap64(
                    normalized_from_value(value, &(0.0..=max), spec),
                    (0.0, 1.0),
                    (zero_cutoff, 1.0),
                )
            }
        }
    } else {
        remap_clamp64(value, (min, max), (0.0, 1.0))
    }
}

fn remap_clamp64(x: f64, from: (f64, f64), to: (f64, f64)) -> f64 {
    emath::remap_clamp(x, from.0..=from.1, to.0..=to.1)
}

/// Direct port of `egui`'s `range_log10`.
fn range_log10(min: f64, max: f64, spec: &LogSpec) -> (f64, f64) {
    if min == 0.0 && max == f64::INFINITY {
        (spec.smallest_positive.log10(), INF_RANGE_MAGNITUDE)
    } else if min == 0.0 {
        if spec.smallest_positive < max {
            (spec.smallest_positive.log10(), max.log10())
        } else {
            (max.log10() - INF_RANGE_MAGNITUDE, max.log10())
        }
    } else if max == f64::INFINITY {
        if min < spec.largest_finite {
            (min.log10(), spec.largest_finite.log10())
        } else {
            (min.log10(), min.log10() + INF_RANGE_MAGNITUDE)
        }
    } else {
        (min.log10(), max.log10())
    }
}

/// Direct port of `egui`'s `logarithmic_zero_cutoff`.
fn logarithmic_zero_cutoff(min: f64, max: f64) -> f64 {
    let min_magnitude = if min == f64::NEG_INFINITY {
        INF_RANGE_MAGNITUDE
    } else {
        min.abs().log10().abs()
    };
    let max_magnitude = if max == f64::INFINITY {
        INF_RANGE_MAGNITUDE
    } else {
        max.log10().abs()
    };

    min_magnitude / (min_magnitude + max_magnitude)
}
