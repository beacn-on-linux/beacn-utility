use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage, map_to_range};
use crate::ui::widgets::helpers::buttons::toggle_button;
use crate::ui::widgets::helpers::composite::draw_horizontal_range;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::expander::{
    Expander, ExpanderMode, ExpanderRatio, ExpanderThreshold,
};
use beacn_lib::types::{HasRange, TimeFrame};
use iced::advanced::mouse;
use iced::widget::canvas::{Cache, LineCap, LineJoin, Path, Stroke, Style};
use iced::widget::{Space, canvas, checkbox, column, row, rule};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Task, Theme};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;

// Run this data sender at 20pps
const POINTS_PER_SECOND: u64 = 5;
const POINT_FREQUENCY_MILLIS: Duration = Duration::from_millis(1000 / POINTS_PER_SECOND);
const TOTAL_POINTS: u64 = POINTS_PER_SECOND * 3;

pub struct ExpanderPage {
    pub peak: Option<(f32, f32)>,
    pub last: Instant,

    pub graph: DbGraph,
}

#[derive(Debug, Clone)]
pub(crate) enum ExpanderMessage {
    SetEnabled(bool),
}

impl ConfigPage for ExpanderPage {
    fn title(&self) -> &'static str {
        "Expander"
    }

    fn update(&mut self, state: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
        if let ChildMessage::Meters(meters) = message {
            let input = meters.pre_expander;
            let output = meters.post_expander;

            let is_new_peak = match self.peak {
                Some((input_peak, _)) => input > input_peak,
                None => true,
            };
            if is_new_peak {
                self.peak.replace((input, output));
            }

            if self.last.elapsed() > POINT_FREQUENCY_MILLIS {
                self.last = Instant::now();
                let (input_peak, output_peak) = self.peak.take().unwrap();
                self.graph.add_sample(input_peak, output_peak);
            }

            // Request a redraw every frame
            self.graph.request_redraw();
            return Task::none();
        }

        let ChildMessage::Expander(message) = message else {
            return Task::none();
        };

        match message {
            // Custom message here, the enabled button applies to both modes to prevent confusion!
            ExpanderMessage::SetEnabled(enabled) => {
                for mode in ExpanderMode::iter() {
                    let exp_msg = Expander::Enabled(mode, enabled);
                    let message = Message::Expander(exp_msg);
                    state.handle_message(message).expect("Failed");
                }
            }
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, ChildMessage> {
        let expander_mode = state.expander.mode;

        let expander = &state.expander;
        let values = &expander.values[expander_mode];

        let enabled = checkbox(values.enabled)
            .label("Enabled")
            .on_toggle(ExpanderMessage::SetEnabled);

        let enabled = Element::from(enabled).map(ChildMessage::Expander);

        let enabled = column![enabled, Space::new().height(10.0)];

        let is_simple = expander_mode == ExpanderMode::Simple;
        let simple = toggle_button("Simple", is_simple).on_press_with(|| {
            let msg = Message::Expander(Expander::Mode(ExpanderMode::Simple));
            ChildMessage::State(msg)
        });

        let is_advanced = expander_mode == ExpanderMode::Advanced;
        let advanced = toggle_button("Advanced", is_advanced).on_press_with(|| {
            let msg = Message::Expander(Expander::Mode(ExpanderMode::Advanced));
            ChildMessage::State(msg)
        });

        let mode = row![simple, advanced].spacing(8.0).height(20);

        // Threshold always appears
        let value = values.threshold;
        let range = ExpanderThreshold::range();
        let range = (*range.start() as i8)..=(*range.end() as i8);
        let threshold = draw_horizontal_range("Threshold", value, range, "dB", move |v| {
            let msg = Expander::Threshold(expander_mode, ExpanderThreshold(v as f32));
            let msg = Message::Expander(msg);
            ChildMessage::State(msg)
        });

        // In simple mode, we just have an 'amount', but that internally maps to the Ratio
        let value = Self::ratio_to_precent(values.ratio);
        let amount = draw_horizontal_range("Amount", value, 0..=100, "%", |v| {
            let ratio = ExpanderRatio(Self::percent_to_ratio(v));
            let msg = Message::Expander(Expander::Ratio(ExpanderMode::Simple, ratio));

            ChildMessage::State(msg)
        });

        // For everything else, there's mastercard
        let value = values.ratio;
        let range = ExpanderRatio::range();
        let ratio = draw_horizontal_range("Ratio", value, range, ":1", move |v| {
            let msg = Message::Expander(Expander::Ratio(expander_mode, ExpanderRatio(v)));
            ChildMessage::State(msg)
        });

        let value = values.attack;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let attack = draw_horizontal_range("Attack", value, range, "ms", move |v| {
            let msg = Message::Expander(Expander::Attack(expander_mode, TimeFrame(v as f32)));
            ChildMessage::State(msg)
        });

        let value = values.release;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let release = draw_horizontal_range("Release", value, range, "ms", move |v| {
            let msg = Message::Expander(Expander::Release(expander_mode, TimeFrame(v as f32)));
            ChildMessage::State(msg)
        });

        let fields = if is_simple {
            column![threshold, amount]
        } else {
            column![threshold, ratio, attack, release]
        }
        .height(Length::Shrink)
        .spacing(10.0);

        let fields = column![enabled, mode, fields]
            .padding(10.0)
            .spacing(10.0)
            .width(330);

        let canvas = canvas::Canvas::new(&self.graph)
            .width(Length::Fill)
            .height(Length::Fill);

        row![fields, rule::vertical(2.0), canvas].into()
    }
}

impl ExpanderPage {
    pub fn new() -> Self {
        let graph = DbGraph::new(TOTAL_POINTS, (-70.0, 0.0), -45.0, POINT_FREQUENCY_MILLIS);
        Self {
            graph,
            peak: None,
            last: Instant::now(),
        }
    }

    fn percent_to_ratio(percent: u8) -> f32 {
        let ratio = if percent <= 50 {
            map_to_range(percent as f32, 0.0, 50.0, 1.0, 3.0)
        } else {
            map_to_range(percent as f32, 51.0, 100.0, 3.1, 10.0)
        };

        // Convert to 2 decimal places
        (ratio * 100.0).round() / 100.0
    }

    fn ratio_to_precent(ratio: f32) -> u8 {
        if ratio <= 3.0 {
            map_to_range(ratio, 1.0, 3.0, 0.0, 50.0)
        } else {
            map_to_range(ratio, 3.1, 10.0, 50.1, 100.0)
        }
        .round() as u8
    }
}

// TODO: Maybe Move this, we need something similar for Mic Setup..
pub struct DbGraph {
    // Input and Expanded levels, along with last received time
    input: VecDeque<f32>,
    output: VecDeque<f32>,
    last_sample_at: Instant,

    // These are used to animate the graph's scroll
    capacity: usize,
    interval: Duration,

    // Not sure if this is actually needed, this is the current threshold for the expander, we
    // can at least somewhat infer this by waiting until unity gain is reached between the input
    // and output samples. Long term, we can at least draw the threshold line.
    threshold: f32,

    // Graph Configuration, the range, width of line, and tension of the Catmull-Rom curve
    db_range: (f32, f32),

    // Used so we don't have to redraw if there's no change
    cache: Cache,
}

// We use red and green hues here because they're simpler to manage a gradient through yellow
// compared to having to manage multiple values in an RGB configuration.
const RED_HUE: f32 = 0.0;
const GREEN_HUE: f32 = 120.0;

// Number of gradient steps to draw between two points, more is smoother but more expensive.
const GRADIENT_STEPS: usize = 10;

//const LINE_TENSION: f32 = 0.0;
const LINE_WIDTH: f32 = 2.5;

impl DbGraph {
    pub fn new(capacity: u64, db_range: (f32, f32), threshold: f32, interval: Duration) -> Self {
        let capacity = capacity.max(2) as usize;
        Self {
            input: VecDeque::with_capacity(capacity + 1),
            output: VecDeque::with_capacity(capacity + 1),
            last_sample_at: Instant::now(),

            capacity,
            interval,

            threshold,
            db_range,

            cache: Cache::new(),
        }
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    // Adds a sample to the graph
    pub fn add_sample(&mut self, input_db: f32, output_db: f32) {
        self.input.push_back(input_db);
        self.output.push_back(output_db);

        while self.input.len() > self.capacity + 1 {
            self.input.pop_front();
            self.output.pop_front();
        }

        self.last_sample_at = Instant::now();
    }

    // Calls when a re-draw is needed (new value added, or a frame pace point)
    pub fn request_redraw(&self) {
        self.cache.clear();
    }

    // Hue for a single time-point, 0 (red) .. 120 (green).
    fn state_hue(&self, input_db: f32, output_db: f32) -> f32 {
        if input_db >= self.threshold {
            return GREEN_HUE;
        }

        let floor = self.db_range.0;
        let denom = (input_db - floor).max(f32::EPSILON);
        let passthrough = ((output_db - floor) / denom).clamp(0.0, 1.0);

        RED_HUE + (GREEN_HUE - RED_HUE) * passthrough
    }
}

impl<Message> canvas::Program<Message> for DbGraph {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let count = self.input.len().min(self.output.len());
            if count < 2 {
                return;
            }

            // Just some helpers
            let (db_min, db_max) = self.db_range;

            // Distance between the points in the graph
            let dx = bounds.width / (self.capacity as f32 - 1.0).max(1.0);

            // Work out how far we are between our current point, and our next point
            let progress = if self.interval.is_zero() {
                0.0
            } else {
                let last = self.last_sample_at.elapsed().as_secs_f32();
                let interval = self.interval.as_secs_f32();
                (last / interval).clamp(0.0, 1.0)
            };

            let scroll_offset = progress * dx;
            let range = (db_max - db_min).max(f32::EPSILON);

            // Work out the position of each point, noting the last and most recent points are
            // drawn off-screen so the values come in and out cleanly
            let points: Vec<Point> = self
                .input
                .iter()
                .take(count)
                .enumerate()
                .map(|(i, &db)| {
                    let steps_behind_target = (count as f32 - 2.0 - i as f32).max(-1.0);
                    let x = bounds.width - steps_behind_target * dx - scroll_offset;
                    let t = ((db - db_min) / range).clamp(0.0, 1.0);
                    Point::new(x, bounds.height - t * bounds.height)
                })
                .collect();

            // Work out the gradient target for each point
            let hues: Vec<f32> = self
                .input
                .iter()
                .take(count)
                .zip(self.output.iter().take(count))
                .map(|(&in_db, &out_db)| self.state_hue(in_db, out_db))
                .collect();

            // Ok, we need to draw a path between the points, this is a little complicated..
            for i in 0..count - 1 {
                // Get the Start and End points for this segment
                let start = points[i];
                let end = points[i + 1];

                // So we can smoothly curve the line, we also need the points before and after
                // this segment.
                let before = if i == 0 { points[0] } else { points[i - 1] };
                let after = if i + 2 < count {
                    points[i + 2]
                } else {
                    points[i + 1]
                };

                // Generate some control points for the bezier curve
                let pull = 1.0 / 6.0;
                let control_1 = Point::new(
                    start.x + (end.x - before.x) * pull,
                    start.y + (end.y - before.y) * pull,
                );
                let control_2 = Point::new(
                    end.x - (after.x - start.x) * pull,
                    end.y - (after.y - start.y) * pull,
                );

                let start_hue = hues[i];
                let end_hue = hues[i + 1];
                let mut previous_point = start;
                for step in 1..=GRADIENT_STEPS {
                    let t = step as f32 / GRADIENT_STEPS as f32;
                    let current_point = cubic_bezier(start, control_1, control_2, end, t);
                    let color = hsl_to_rgb(start_hue + (end_hue - start_hue) * t, 1.0, 0.45);

                    // Create the segment
                    let segment = Path::new(|builder| {
                        builder.move_to(previous_point);
                        builder.line_to(current_point);
                    });

                    // Draw the segment
                    frame.stroke(
                        &segment,
                        Stroke {
                            style: Style::Solid(color),
                            width: LINE_WIDTH,
                            line_cap: LineCap::Round,
                            line_join: LineJoin::Round,
                            ..Stroke::default()
                        },
                    );
                    previous_point = current_point;
                }
            }
        });

        vec![geometry]
    }
}

// We need our own for this, iced DOES have one for path drawing, but it's expecting to draw
// the entire path itself, where as we're drawing smaller segments of different colours
fn cubic_bezier(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
    let mt = 1.0 - t;

    let weight_p0 = mt * mt * mt; // (1-t)^3
    let weight_p1 = 3.0 * mt * mt * t; // 3(1-t)^2 * t
    let weight_p2 = 3.0 * mt * t * t; // 3(1-t) * t^2
    let weight_p3 = t * t * t; // t^3

    let x = weight_p0 * p0.x + weight_p1 * p1.x + weight_p2 * p2.x + weight_p3 * p3.x;
    let y = weight_p0 * p0.y + weight_p1 * p1.y + weight_p2 * p2.y + weight_p3 * p3.y;

    Point::new(x, y)
}

// Converts a HSL to an RGB colour
fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> Color {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let hp = hue / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());

    let (r1, g1, b1) = if hp < 1.0 { (c, x, 0.0) } else { (x, c, 0.0) };

    let m = lightness - c / 2.0;
    Color::from_rgb(r1 + m, g1 + m, b1 + m)
}
