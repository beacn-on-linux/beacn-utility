//! This is a re-implementation of the egui DragValue widget ported to iced as closely as possible.
//!
//! I currently use emath for certain parts, generally because I couldn't find the equivalent
//! iced version, or the iced versions were lacking. emath is just a math library though and
//! while made for egui, it's useful anywhere.
//!
//! We attempt to copy the behaviour of text_input in as many places as possible, but due to a
//! widget not being able to process events on a child widget we can't just do some magic switcharoo
//!
use std::ops::RangeInclusive;

use crate::ui::widgets::numeric::{clamp_to_range, to_num};
use emath::Numeric;
use iced::advanced::graphics::core::touch;
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse::{self, Click, click};
use iced::advanced::text::{self, Paragraph as _, Text, paragraph};
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::advanced::{clipboard, renderer};
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::time::Instant;
use iced::widget::text_input::{self, Value as TextValue};
use iced::{Alignment, Element, Event, Length, Padding, Pixels, Point, Rectangle, Size, alignment};
use unicode_segmentation::UnicodeSegmentation;

const MAX_CLICK_DIST: f32 = 6.0;
const MAX_CLICK_DURATION: f64 = 0.8;
const CURSOR_BLINK_INTERVAL_MILLIS: u128 = 500;

type CursorState = text_input::cursor::State;

// So, "Why are you using emath::Numeric instead of num_traits"? This has a relatively simple
// answer: emath's Numeric provides a LOT of helpers that allow us to move back and forth between
// various types, without detailed checking. It provides:
// Num::INTEGRAL - Is this an integral type?
// Num::MIN/MAX - The minimum/maximum value of this type.
// Num::to_f64/from_f64 - Convert to/from f64.
//
// In num_traits, these are split across Bounded, ToPrimitive, NumCast, and these all return
// Options, rather than infallible results. In addition, there's NOTHING for INTEGRAL, there are
// PrimInt and PrimFloat, but they're mutually exclusive marker traits, so the only solution
// would be to hand-roll our own stuff, or just use emath.

/// A Drag Value
pub struct DragValue<'a, Num, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Num: Numeric,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    // Value management, we accept a Num then internally convert to f64.
    value: f64,
    speed: f64,
    range: RangeInclusive<f64>,

    // General Display Settings
    clamp_existing_to_range: bool,
    min_decimals: usize,
    max_decimals: Option<usize>,
    prefix: String,
    suffix: String,

    // Iced specific display settings
    width: Length,
    padding: Padding,
    font: Option<Renderer::Font>,
    text_size: Option<Pixels>,
    align_x: alignment::Horizontal,
    aim_radius: f64,

    // Update Management
    update_while_editing: bool,
    on_change: Option<Box<dyn Fn(Num) -> Message + 'a>>,

    // Theming and Styling
    class: Theme::Class<'a>,
}

impl<'a, Num, Message, Theme, Renderer> DragValue<'a, Num, Message, Theme, Renderer>
where
    Num: Numeric,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new [`DragValue`] displaying (and controlling) `value`.
    pub fn new(value: Num) -> Self {
        let slf = Self {
            value: value.to_f64(),
            speed: 1.0,
            range: f64::NEG_INFINITY..=f64::INFINITY,
            clamp_existing_to_range: true,
            min_decimals: 0,
            max_decimals: None,
            prefix: String::new(),
            suffix: String::new(),
            width: Length::Shrink,
            padding: text_input::DEFAULT_PADDING,
            font: None,
            text_size: None,
            align_x: alignment::Horizontal::Left,
            aim_radius: 1.0,
            update_while_editing: true,
            on_change: None,
            class: Theme::default(),
        };

        if Num::INTEGRAL {
            slf.max_decimals(0).range(Num::MIN..=Num::MAX).speed(0.25)
        } else {
            slf
        }
    }

    /// How much the value changes per logical pixel dragged.
    pub fn speed(mut self, speed: impl Into<f64>) -> Self {
        self.speed = speed.into();
        self
    }

    /// The valid range for the value.
    pub fn range(mut self, range: RangeInclusive<Num>) -> Self {
        self.range = range.start().to_f64()..=range.end().to_f64();
        self
    }

    /// If `true` values coming from outside (i.e. not from a  drag or a text edit) are
    /// also clamped to [`Self::range`].
    pub fn clamp_existing_to_range(mut self, clamp: bool) -> Self {
        self.clamp_existing_to_range = clamp;
        self
    }

    /// Minimum number of decimals to display. Default: `0`.
    pub fn min_decimals(mut self, min_decimals: usize) -> Self {
        self.min_decimals = min_decimals;
        self
    }

    /// Maximum number of decimals to display (and to round to).
    pub fn max_decimals(mut self, max_decimals: usize) -> Self {
        self.max_decimals = Some(max_decimals);
        self
    }

    /// Shorthand for setting both [`Self::min_decimals`] and
    /// [`Self::max_decimals`] to the same value.
    pub fn fixed_decimals(mut self, decimals: usize) -> Self {
        self.min_decimals = decimals;
        self.max_decimals = Some(decimals);
        self
    }

    /// Text shown before the number (only while not editing), e.g. `"x: "`.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Text shown after the number (only while not editing), e.g. `" px"`.
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// Sets the width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the font.
    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Sets the text size.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the horizontal text alignment.
    pub fn align_x(mut self, align_x: impl Into<alignment::Horizontal>) -> Self {
        self.align_x = align_x.into();
        self
    }

    /// Overrides the assumed pointer imprecision (in logical points) used
    /// for "smart aim" rounding and auto-decimals. egui derives this from
    /// the display scale factor (`1 / pixels_per_point`); iced's widget API
    /// doesn't expose that here, so it defaults to a fixed `1.0`.
    pub fn aim_radius(mut self, aim_radius: f64) -> Self {
        self.aim_radius = aim_radius;
        self
    }

    /// Send a value update on every keystroke
    pub fn update_while_editing(mut self, update: bool) -> Self {
        self.update_while_editing = update;
        self
    }

    /// Message to send when the value changes
    pub fn on_change(mut self, on_change: impl Fn(Num) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }

    /// Optional Message to send when the value changes
    pub fn on_change_maybe(mut self, on_change: Option<impl Fn(Num) -> Message + 'a>) -> Self {
        self.on_change = on_change.map(|f| Box::new(f) as _);
        self
    }

    /// Sets the style.
    #[must_use]
    pub fn style(
        mut self,
        style: impl Fn(&Theme, text_input::Status) -> text_input::Style + 'a,
    ) -> Self
    where
        Theme::Class<'a>: From<text_input::StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as text_input::StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn decimals(&self, is_slow_speed: bool) -> (usize, usize) {
        let speed = self.speed.abs().max(f64::MIN_POSITIVE);
        let auto = (self.aim_radius / speed).log10().ceil().clamp(0.0, 15.0) as usize;
        let auto = auto + usize::from(is_slow_speed);
        let max_decimals = self.max_decimals.unwrap_or(auto + 2).max(self.min_decimals);
        let auto = auto.clamp(self.min_decimals, max_decimals);
        (auto, max_decimals)
    }

    /// The value this widget treats as "current" for display and as the drag/edit starting point.
    fn effective_value(&self) -> f64 {
        if self.clamp_existing_to_range {
            clamp_to_range(self.value, &self.range)
        } else {
            self.value
        }
    }

    fn formatted_number(&self) -> String {
        let (auto, max) = self.decimals(false);
        emath::format_with_decimals_in_range(self.effective_value(), auto..=max)
    }

    fn display_text(&self) -> String {
        format!("{}{}{}", self.prefix, self.formatted_number(), self.suffix)
    }

    fn status(&self, state: &State<Renderer::Paragraph>) -> text_input::Status {
        if self.on_change.is_none() {
            text_input::Status::Disabled
        } else if state.focus.is_some() {
            text_input::Status::Focused {
                is_hovered: state.is_hovered,
            }
        } else if state.is_hovered {
            text_input::Status::Hovered
        } else {
            text_input::Status::Active
        }
    }

    /// Commits the current edit buffer.
    fn apply_edit_buffer(
        &self,
        state: &mut State<Renderer::Paragraph>,
        shell: &mut Shell<'_, Message>,
    ) {
        let Some(on_change) = &self.on_change else {
            return;
        };

        let text = state.edit_value.to_string();
        if let Some(parsed) = default_parse(&text) {
            let clamped = clamp_to_range(parsed, &self.range);
            let baseline = state.last_published.unwrap_or(self.value);
            if clamped != baseline {
                state.last_published = Some(clamped);
                shell.publish(on_change(to_num(clamped)));
            }
        }
    }

    // Checks whether a change will result in a valid number
    fn valid_edit(
        &self,
        value: &TextValue,
        selection: Option<(usize, usize)>,
        cursor: CursorState,
        inserted: &str,
    ) -> bool {
        let len = value.len();
        let (before, after) = if let Some((s, e)) = selection {
            (value.until(s).to_string(), value.select(e, len).to_string())
        } else {
            let index = cursor_index(len, cursor);
            (
                value.until(index).to_string(),
                value.select(index, len).to_string(),
            )
        };

        let candidate = format!("{before}{inserted}{after}");

        let negative_count = candidate.chars().filter(|&c| c == '-').count();
        let decimal_count = candidate.chars().filter(|&c| c == '.').count();

        let characters = |c: char| c.is_ascii_digit() || c == '-' || (!Num::INTEGRAL && c == '.');
        let valid_chars = candidate.chars().all(characters);

        let valid_negative = negative_count <= 1 && candidate.find('-').is_none_or(|i| i == 0);
        let valid_decimal = decimal_count <= 1;

        !inserted.is_empty() && valid_chars && valid_negative && valid_decimal
    }

    // Updates the Paragraph text and rendering
    fn update_paragraph(&self, state: &mut State<Renderer::Paragraph>, renderer: &Renderer) {
        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
        let line_height = text::LineHeight::default();
        let height = line_height.to_absolute(text_size);

        let content = match state.focus {
            Some(_) => state.edit_value.to_string(),
            None => self.display_text(),
        };

        let _ = state.paragraph.update(Text {
            content: &content,
            bounds: Size::new(f32::INFINITY, height.into()),
            size: text_size,
            line_height,
            font,
            align_x: match self.align_x {
                alignment::Horizontal::Left => text::Alignment::Left,
                alignment::Horizontal::Center => text::Alignment::Center,
                alignment::Horizontal::Right => text::Alignment::Right,
            },
            align_y: alignment::Vertical::Center,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::default(),
        });
    }
}

struct Focus {
    updated_at: Instant,
}

#[derive(Clone, Copy)]
enum Interaction {
    Idle,

    Pressed {
        origin: Point,
        last_pos: Point,
        started_at: Instant,
        decided_drag: bool,
        precise_value: f64,
    },
}

struct State<P: text::Paragraph> {
    paragraph: paragraph::Plain<P>,
    interaction: Interaction,
    focus: Option<Focus>,
    edit_value: TextValue,
    cursor: CursorState,
    is_selecting: bool,
    drag_anchor: usize,
    last_click: Option<mouse::Click>,
    is_hovered: bool,
    modifiers: keyboard::Modifiers,

    // Last value published to on_change
    last_published: Option<f64>,
}

impl<P: text::Paragraph> Default for State<P> {
    fn default() -> Self {
        Self {
            paragraph: paragraph::Plain::default(),
            interaction: Interaction::Idle,
            focus: None,
            edit_value: TextValue::new(""),
            cursor: CursorState::Index(0),
            is_selecting: false,
            drag_anchor: 0,
            last_click: None,
            is_hovered: false,
            modifiers: keyboard::Modifiers::default(),

            last_published: None,
        }
    }
}

fn cursor_index(edit_len: usize, cursor: CursorState) -> usize {
    match cursor {
        CursorState::Index(i) => i.min(edit_len),
        CursorState::Selection { end, .. } => end.min(edit_len),
    }
}

fn selection_range(edit_len: usize, cursor: CursorState) -> Option<(usize, usize)> {
    match cursor {
        CursorState::Selection { start, end } => {
            let s = start.min(end).min(edit_len);
            let e = start.max(end).min(edit_len);
            if s == e { None } else { Some((s, e)) }
        }
        CursorState::Index(_) => None,
    }
}

impl<'a, Num, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DragValue<'a, Num, Message, Theme, Renderer>
where
    Num: Numeric,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        self.update_paragraph(state, renderer);

        let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
        let padding = self.padding.fit(Size::ZERO, limits.max());
        let line_height = text::LineHeight::default();
        let height = line_height.to_absolute(text_size);
        let intrinsic = Size::new(state.paragraph.min_width(), height.into());
        let limits = limits.width(self.width).shrink(padding);
        let text_bounds = limits.resolve(self.width, height, intrinsic);

        let point = Point::new(padding.left, padding.top);
        let text_node = layout::Node::new(text_bounds).move_to(point);

        layout::Node::with_children(text_bounds.expand(padding), vec![text_node])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();
        let text_bounds = layout
            .children()
            .next()
            .map(|l| l.bounds())
            .unwrap_or(bounds);

        let status = self.status(state);
        let style = theme.style(&self.class, status);

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let content = state.paragraph.content();
        let is_empty = content.is_empty();

        let paragraph = state.paragraph.raw();

        let para_min = paragraph.min_bounds();
        let text_origin = text_bounds.anchor(para_min, self.align_x, Alignment::Center);

        if let Some(focus) = &state.focus {
            let len = state.edit_value.len();

            match state.cursor {
                CursorState::Index(index) => {
                    let index = index.min(len);
                    let x = measure_x(paragraph, index);

                    let last_blink = Instant::now().saturating_duration_since(focus.updated_at);
                    let blink_phase = last_blink.as_millis() / CURSOR_BLINK_INTERVAL_MILLIS;
                    let is_visible = blink_phase.is_multiple_of(2);

                    if is_visible {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle {
                                    x: (text_origin.x + x).floor(),
                                    y: text_bounds.y,
                                    width: 1.0,
                                    height: text_bounds.height,
                                },
                                ..renderer::Quad::default()
                            },
                            style.value,
                        );
                    }
                }
                CursorState::Selection { start, end } => {
                    let s = start.min(end).min(len);
                    let e = start.max(end).min(len);
                    let xs = measure_x(paragraph, s);
                    let xe = measure_x(paragraph, e);

                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle {
                                x: text_origin.x + xs,
                                y: text_bounds.y,
                                width: xe - xs,
                                height: text_bounds.height,
                            },
                            ..renderer::Quad::default()
                        },
                        style.selection,
                    );
                }
            }
        }

        renderer.fill_paragraph(
            paragraph,
            text_origin,
            if is_empty {
                style.placeholder
            } else {
                style.value
            },
            *viewport,
        );
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::default())
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children.clear();

        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        if self.on_change.is_none() {
            state.focus = None;
            state.interaction = Interaction::Idle;
            state.is_selecting = false;
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();
        let is_focused = state.focus.is_some();

        struct DragValueFocusable<'s, P: text::Paragraph> {
            state: &'s mut State<P>,
        }

        impl<P: text::Paragraph> widget::operation::Focusable for DragValueFocusable<'_, P> {
            fn is_focused(&self) -> bool {
                self.state.focus.is_some()
            }

            fn focus(&mut self) {
                self.state.focus = Some(Focus {
                    updated_at: Instant::now(),
                });
                self.state.last_published = None;
            }

            fn unfocus(&mut self) {
                self.state.focus = None;
                self.state.is_selecting = false;
            }
        }

        let _ = is_focused;
        operation.focusable(None, bounds, &mut DragValueFocusable { state });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        // Ok, don't do anything at all if we're disabled.
        let Some(on_change) = self.on_change.as_ref() else {
            return;
        };

        // So this is where the fun shit happens
        let bounds = layout.bounds();
        let text_bounds = layout
            .children()
            .next()
            .map(|l| l.bounds())
            .unwrap_or(bounds);
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        // Hover State handle, we could do this later but it's isolated and mouse movement
        // otherwise is a WHOLE thing, so we can keep this out the way :D
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
            // General Mouse stuff first.
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                // If we're already focused, we're in text edit most, treat this as a text input
                if state.focus.is_some() {
                    if let Some(position) = cursor.position_over(bounds) {
                        let content = state.edit_value.to_string();
                        let text_origin_x = text_bounds
                            .anchor(
                                state.paragraph.raw().min_bounds(),
                                self.align_x,
                                Alignment::Center,
                            )
                            .x;
                        let x = position.x - text_origin_x;
                        let index = hit_test_index(state.paragraph.raw(), &content, text_bounds, x);

                        let click = Click::new(position, mouse::Button::Left, state.last_click);
                        state.last_click = Some(click);

                        match click.kind() {
                            click::Kind::Single => {
                                if state.modifiers.shift() {
                                    let anchor = cursor_index(state.edit_value.len(), state.cursor);
                                    state.cursor = if anchor == index {
                                        CursorState::Index(index)
                                    } else {
                                        CursorState::Selection {
                                            start: anchor,
                                            end: index,
                                        }
                                    };
                                    state.drag_anchor = anchor;
                                } else {
                                    state.cursor = CursorState::Index(index);
                                    state.drag_anchor = index;
                                }
                                state.is_selecting = true;
                            }
                            click::Kind::Double | click::Kind::Triple => {
                                let len = state.edit_value.len();
                                state.cursor = if len == 0 {
                                    CursorState::Index(0)
                                } else {
                                    CursorState::Selection { start: 0, end: len }
                                };
                                state.is_selecting = false;
                            }
                        }

                        shell.capture_event();
                    } else {
                        // Outside click, finish up.
                        self.apply_edit_buffer(state, shell);
                        state.focus = None;
                        state.is_selecting = false;
                    }
                } else if let Some(position) = cursor.position_over(bounds) {
                    // Store this, so we can work out intent on move and release
                    state.interaction = Interaction::Pressed {
                        origin: position,
                        last_pos: position,
                        started_at: Instant::now(),
                        decided_drag: false,
                        precise_value: self.effective_value(),
                    };
                    shell.capture_event();
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                // Again, if we're in text mode, handle text input stuff (text selection)
                if state.focus.is_some() && state.is_selecting {
                    if let Some(position) = cursor.position() {
                        let content = state.edit_value.to_string();
                        let text_origin_x = text_bounds
                            .anchor(
                                state.paragraph.raw().min_bounds(),
                                self.align_x,
                                Alignment::Center,
                            )
                            .x;
                        let x = position.x - text_origin_x;
                        let index = hit_test_index(state.paragraph.raw(), &content, text_bounds, x);
                        state.cursor = if index == state.drag_anchor {
                            CursorState::Index(index)
                        } else {
                            CursorState::Selection {
                                start: state.drag_anchor,
                                end: index,
                            }
                        };
                    }
                } else if let Interaction::Pressed {
                    origin,
                    last_pos,
                    started_at,
                    decided_drag,
                    precise_value,
                } = &mut state.interaction
                {
                    // Not in text mode, lets try and work out intent..
                    let now_over = cursor.is_over(bounds);

                    // There are three identifiers to find out if we're dragging:
                    // 1. Has the mouse left the boundary of the input?
                    // 2. How far the mouse has moved from its starting point
                    // 3. How long the user has been holding the mouse button
                    if !*decided_drag {
                        let moved_too_much = origin.distance(*position) > MAX_CLICK_DIST;
                        let held_too_long = started_at.elapsed().as_secs_f64() > MAX_CLICK_DURATION;

                        if moved_too_much || held_too_long || !now_over {
                            *decided_drag = true;
                            *last_pos = *position;
                            *precise_value = self.effective_value();
                        }
                    }

                    // If we're in drag mode, update the value based on the mouse movement
                    if *decided_drag {
                        let is_slow_speed = state.modifiers == keyboard::Modifiers::SHIFT;
                        let (auto_decimals, _) = self.decimals(is_slow_speed);
                        let speed = if is_slow_speed {
                            self.speed / 10.0
                        } else {
                            self.speed
                        };

                        // "Increase to the right and up", per egui.
                        let delta_points = (position.x - last_pos.x) - (position.y - last_pos.y);
                        let delta_value = f64::from(delta_points) * speed;
                        *last_pos = *position;

                        if delta_value != 0.0 {
                            let new_precise = *precise_value + delta_value;
                            let aim_delta = self.aim_radius * speed;
                            let rounded = emath::smart_aim::best_in_range_f64(
                                new_precise - aim_delta,
                                new_precise + aim_delta,
                            );
                            let rounded = emath::round_to_decimals(rounded, auto_decimals);
                            let rounded = clamp_to_range(rounded, &self.range);

                            *precise_value = new_precise;

                            if rounded != self.effective_value() {
                                shell.publish(on_change(to_num(rounded)));
                            }
                        }
                    }
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
            | Event::Touch(touch::Event::FingerLost { .. }) => {
                state.is_selecting = false;

                // If we've not hit any of the drag conditions above, and the mouse has been
                // released, we should switch to text edit mode and position the cursor.
                if let Interaction::Pressed { decided_drag, .. } = state.interaction {
                    state.interaction = Interaction::Idle;

                    if !decided_drag && cursor.is_over(bounds) {
                        state.edit_value = TextValue::new(&self.formatted_number());
                        let len = state.edit_value.len();
                        state.cursor = if len == 0 {
                            CursorState::Index(0)
                        } else {
                            CursorState::Selection { start: 0, end: len }
                        };
                        state.focus = Some(Focus {
                            updated_at: Instant::now(),
                        });
                        state.last_published = None;
                        shell.capture_event();
                    }
                }
            }

            Event::Keyboard(keyboard::Event::KeyPressed {
                // Note, as `pressed_key` because `key` shadows `keyboard::key`
                key: pressed_key,
                physical_key,
                modifiers,
                text,
                ..
            }) if state.focus.is_some() => {
                // Select All, Cut, Copy, and Paste
                let command = modifiers.command();
                if command {
                    match pressed_key.to_latin(*physical_key) {
                        Some('a') => {
                            let len = state.edit_value.len();
                            state.cursor = if len == 0 {
                                CursorState::Index(0)
                            } else {
                                CursorState::Selection { start: 0, end: len }
                            };
                            shell.capture_event();
                            shell.request_redraw();
                            return;
                        }
                        Some('c') => {
                            let range = selection_range(state.edit_value.len(), state.cursor);
                            if let Some((s, e)) = range {
                                let selected = state.edit_value.select(s, e).to_string();
                                clipboard.write(clipboard::Kind::Standard, selected);
                            }
                            shell.capture_event();
                            return;
                        }
                        Some('x') => {
                            let range = selection_range(state.edit_value.len(), state.cursor);
                            if let Some((s, e)) = range {
                                let selected = state.edit_value.select(s, e).to_string();
                                clipboard.write(clipboard::Kind::Standard, selected);

                                // For Cut, we need to remove it from the edit buffer.
                                state.edit_value.remove_many(s, e);
                                state.cursor = CursorState::Index(s);
                                if self.update_while_editing {
                                    self.apply_edit_buffer(state, shell);
                                }

                                self.update_paragraph(state, renderer);
                            }
                            shell.capture_event();
                            shell.request_redraw();
                            return;
                        }
                        Some('v') => {
                            // We're gonna do some special for the paste, rather than just pasting it, we'll
                            // try to validate the incoming data.
                            if let Some(pasted) = clipboard.read(clipboard::Kind::Standard) {
                                let sel = selection_range(state.edit_value.len(), state.cursor);
                                let current = &state.edit_value;

                                if self.valid_edit(current, sel, state.cursor, &pasted) {
                                    let insert_at = if let Some((s, e)) = sel {
                                        state.edit_value.remove_many(s, e);
                                        s
                                    } else {
                                        cursor_index(state.edit_value.len(), state.cursor)
                                    };

                                    let inserted = TextValue::new(&pasted);
                                    let inserted_len = inserted.len();

                                    state.edit_value.insert_many(insert_at, inserted);
                                    state.cursor = CursorState::Index(insert_at + inserted_len);

                                    self.update_paragraph(state, renderer);

                                    if self.update_while_editing {
                                        self.apply_edit_buffer(state, shell);
                                    }
                                }
                            }
                            shell.capture_event();
                            shell.request_redraw();
                            return;
                        }
                        _ => {}
                    }
                }

                if let Some(text) = text
                    && let Some(c) = text.chars().next().filter(|c| !c.is_control())
                {
                    // Character input only gets here.

                    let sel = selection_range(state.edit_value.len(), state.cursor);
                    let current = &state.edit_value;
                    let inserted = c.to_string();

                    if self.valid_edit(current, sel, state.cursor, &inserted) {
                        let insert_at = if let Some((s, e)) = sel {
                            state.edit_value.remove_many(s, e);
                            s
                        } else {
                            cursor_index(state.edit_value.len(), state.cursor)
                        };

                        state.edit_value.insert(insert_at, c);
                        state.cursor = CursorState::Index(insert_at + 1);

                        self.update_paragraph(state, renderer);

                        if self.update_while_editing {
                            self.apply_edit_buffer(state, shell);
                        }

                        shell.request_redraw();
                    }

                    shell.capture_event();
                    return;
                }

                // Any other keys that may be useful
                let mut changed = false;
                let mut commit_and_unfocus = false;
                let mut cancel_and_unfocus = false;

                match pressed_key.as_ref() {
                    Key::Named(Named::ArrowUp) | Key::Named(Named::ArrowDown) => {
                        let sign = if matches!(pressed_key, Key::Named(Named::ArrowUp)) {
                            1.0
                        } else {
                            -1.0
                        };
                        let is_slow_speed = modifiers.shift();
                        let (auto_decimals, _) = self.decimals(is_slow_speed);
                        let baseline = state
                            .last_published
                            .unwrap_or_else(|| self.effective_value());
                        let stepped =
                            emath::round_to_decimals(baseline + self.speed * sign, auto_decimals);
                        let stepped = clamp_to_range(stepped, &self.range);

                        if stepped != baseline {
                            state.last_published = Some(stepped);
                            shell.publish(on_change(to_num(stepped)));
                        }

                        state.edit_value = TextValue::new(&self.formatted_number());
                        state.cursor = CursorState::Index(state.edit_value.len());

                        self.update_paragraph(state, renderer);
                    }

                    Key::Named(Named::ArrowLeft) => {
                        let len = state.edit_value.len();
                        if modifiers.shift() {
                            let anchor = match state.cursor {
                                CursorState::Selection { start, .. } => start,
                                CursorState::Index(i) => i,
                            };
                            let end = cursor_index(state.edit_value.len(), state.cursor);
                            let new_end = end.saturating_sub(1);
                            state.cursor = if anchor == new_end {
                                CursorState::Index(new_end)
                            } else {
                                CursorState::Selection {
                                    start: anchor,
                                    end: new_end,
                                }
                            };
                        } else if let Some((s, _)) = selection_range(len, state.cursor) {
                            state.cursor = CursorState::Index(s);
                        } else {
                            let i = cursor_index(state.edit_value.len(), state.cursor);
                            state.cursor = CursorState::Index(i.saturating_sub(1));
                        }
                    }

                    Key::Named(Named::ArrowRight) => {
                        let len = state.edit_value.len();
                        if modifiers.shift() {
                            let anchor = match state.cursor {
                                CursorState::Selection { start, .. } => start,
                                CursorState::Index(i) => i,
                            };
                            let end = cursor_index(len, state.cursor);
                            let new_end = (end + 1).min(len);
                            state.cursor = if anchor == new_end {
                                CursorState::Index(new_end)
                            } else {
                                CursorState::Selection {
                                    start: anchor,
                                    end: new_end,
                                }
                            };
                        } else if let Some((_, e)) = selection_range(len, state.cursor) {
                            state.cursor = CursorState::Index(e);
                        } else {
                            let i = cursor_index(len, state.cursor);
                            state.cursor = CursorState::Index((i + 1).min(len));
                        }
                    }

                    Key::Named(Named::Home) => {
                        if modifiers.shift() {
                            let anchor = match state.cursor {
                                CursorState::Selection { start, .. } => start,
                                CursorState::Index(i) => i,
                            };
                            state.cursor = CursorState::Selection {
                                start: anchor,
                                end: 0,
                            };
                        } else {
                            state.cursor = CursorState::Index(0);
                        }
                    }

                    Key::Named(Named::End) => {
                        let len = state.edit_value.len();
                        if modifiers.shift() {
                            let anchor = match state.cursor {
                                CursorState::Selection { start, .. } => start,
                                CursorState::Index(i) => i,
                            };
                            state.cursor = CursorState::Selection {
                                start: anchor,
                                end: len,
                            };
                        } else {
                            state.cursor = CursorState::Index(len);
                        }
                    }

                    Key::Named(Named::Backspace) => {
                        if let Some((s, e)) = selection_range(state.edit_value.len(), state.cursor)
                        {
                            state.edit_value.remove_many(s, e);
                            state.cursor = CursorState::Index(s);
                        } else {
                            let i = cursor_index(state.edit_value.len(), state.cursor);
                            if i > 0 {
                                state.edit_value.remove(i - 1);
                                state.cursor = CursorState::Index(i - 1);
                            }
                        }
                        changed = true;
                    }

                    Key::Named(Named::Delete) => {
                        if let Some((s, e)) = selection_range(state.edit_value.len(), state.cursor)
                        {
                            state.edit_value.remove_many(s, e);
                            state.cursor = CursorState::Index(s);
                        } else {
                            let i = cursor_index(state.edit_value.len(), state.cursor);
                            if i < state.edit_value.len() {
                                state.edit_value.remove(i);
                            }
                        }
                        changed = true;
                    }

                    Key::Named(Named::Enter) | Key::Named(Named::Tab) => {
                        commit_and_unfocus = true;
                    }

                    Key::Named(Named::Escape) => {
                        cancel_and_unfocus = true;
                    }

                    _ => {}
                }

                // If anything's changed, redraw the text.
                if changed {
                    self.update_paragraph(state, renderer);
                }

                if changed && self.update_while_editing {
                    self.apply_edit_buffer(state, shell);
                }

                if commit_and_unfocus {
                    self.apply_edit_buffer(state, shell);
                    state.focus = None;
                    state.is_selecting = false;
                } else if cancel_and_unfocus {
                    // Escape skips the *final* commit-from-text step, but
                    // (as in egui) does not roll back changes that
                    // `update_while_editing` already applied live.
                    state.focus = None;
                    state.is_selecting = false;
                }

                shell.capture_event();
                shell.request_redraw();
            }

            _ => {}
        }

        let _ = renderer;
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.on_change.is_none() {
            return mouse::Interaction::None;
        }

        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();

        if !cursor.is_over(bounds) {
            return mouse::Interaction::None;
        }

        if state.focus.is_some() {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::ResizingHorizontally
        }
    }
}

impl<'a, Num, Message, Theme, Renderer> From<DragValue<'a, Num, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Num: Numeric,
    Message: 'a,
    Theme: text_input::Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(value: DragValue<'a, Num, Message, Theme, Renderer>) -> Self {
        Self::new(value)
    }
}

// Text measurement helpers
fn measure_x<P: text::Paragraph>(paragraph: &P, grapheme_index: usize) -> f32 {
    paragraph
        .grapheme_position(0, grapheme_index)
        .map(|point| point.x)
        .unwrap_or(0.0)
}

fn hit_test_index<P: text::Paragraph>(
    paragraph: &P,
    content: &str,
    text_bounds: Rectangle,
    x: f32,
) -> usize {
    let Some(hit) = paragraph.hit_test(Point::new(x, text_bounds.height / 2.0)) else {
        return 0;
    };

    let byte_offset = hit.cursor().min(content.len());
    UnicodeSegmentation::graphemes(&content[..byte_offset], true).count()
}

fn default_parse(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == '\u{2212}' { '-' } else { c })
        .collect();

    cleaned.parse().ok()
}
