use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::widgets::helpers::buttons::toggle_button;
use crate::ui::widgets::helpers::composite::draw_horizontal_range;
use beacn_lib::audio::data::{BulkMessage, SuppressionResponse};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::suppressor::{Suppressor, SuppressorSensitivity, SuppressorStyle};
use beacn_lib::types::{HasRange, Percent};
use iced::border::Radius;
use iced::widget::canvas::{Frame, Geometry};
use iced::widget::{
    Canvas, Space, button, canvas, checkbox, column, container, progress_bar, row, rule, stack,
    text,
};
use iced::{
    Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Task, Theme, mouse,
};
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub(crate) enum SuppressorMessage {
    Start,
    //Step,
    End,
}

pub struct SuppressorPage {
    // Used for the live suppression response
    current: SuppressionResponse,
    baseline: SuppressionResponse,

    // This is for state restore after a snapshot completes.
    suppressor_enabled: bool,

    // Is the snapshot itself currently running?
    snapshot_running: bool,

    snapshot_started: Instant,
}

impl SuppressorPage {
    pub(crate) fn new() -> Self {
        Self {
            current: Default::default(),
            baseline: Default::default(),

            suppressor_enabled: false,
            snapshot_running: false,

            snapshot_started: Instant::now(),
        }
    }
}

impl SuppressorPage {
    fn stop_snapshot(&mut self, state: &mut AudioState) {
        if !self.snapshot_running {
            return;
        }

        let msg = Message::Suppressor(Suppressor::Enabled(self.suppressor_enabled));
        let _ = state.handle_message(msg);

        let style = SuppressorStyle::Snapshot;
        let msg = Message::Suppressor(Suppressor::Style(style));
        let _ = state.handle_message(msg);

        self.snapshot_running = false;
    }
}

impl ConfigPage for SuppressorPage {
    fn title(&self) -> &'static str {
        "Noise Suppression"
    }

    fn update(&mut self, state: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
        if matches!(message, ChildMessage::OnTick) {
            let msg = BulkMessage::GetSuppressionBase;
            if let Ok(BulkMessage::SuppressionBase(response)) = state.handle_bulk_message(msg) {
                self.baseline = response;
            }

            let msg = BulkMessage::GetSuppressionCurrent;
            if let Ok(BulkMessage::SuppressionCurrent(response)) = state.handle_bulk_message(msg) {
                self.current = response;
            }
        }

        let ChildMessage::Suppressor(message) = message else {
            return Task::none();
        };

        match message {
            SuppressorMessage::Start => {
                self.suppressor_enabled = state.suppressor.enabled;
                self.snapshot_running = true;
                self.snapshot_started = Instant::now();

                let message = Message::Suppressor(Suppressor::Enabled(false));
                let _ = state.handle_message(message);

                let message = Message::Suppressor(Suppressor::Style(SuppressorStyle::Instant));
                let _ = state.handle_message(message);

                Task::perform(
                    async move {
                        sleep(Duration::from_millis(5000)).await;
                    },
                    |_| ChildMessage::Suppressor(SuppressorMessage::End),
                )
            }
            SuppressorMessage::End => {
                self.stop_snapshot(state);
                Task::none()
            }
        }
    }

    fn on_close(&mut self, state: &mut AudioState) {
        self.stop_snapshot(state);
    }

    fn view(&self, state: &AudioState) -> Element<'_, ChildMessage> {
        let suppressor = state.suppressor;
        let enabled = checkbox(suppressor.enabled)
            .label("Enabled")
            .on_toggle(move |v| {
                let msg = Message::Suppressor(Suppressor::Enabled(v));
                ChildMessage::State(msg)
            });

        let enabled = column![enabled, Space::new().height(10.0)];

        let is_adaptive = suppressor.style == SuppressorStyle::Adaptive;
        let adaptive = toggle_button("Adaptive", is_adaptive).on_press_with(|| {
            let msg = Message::Suppressor(Suppressor::Style(SuppressorStyle::Adaptive));
            ChildMessage::State(msg)
        });

        let is_snapshot = suppressor.style == SuppressorStyle::Snapshot;
        let snapshot = toggle_button("Snapshot", is_snapshot).on_press_with(|| {
            let msg = Message::Suppressor(Suppressor::Style(SuppressorStyle::Snapshot));
            ChildMessage::State(msg)
        });

        let mode = row![adaptive, snapshot].spacing(8.0).height(20);

        let value = suppressor.amount;
        let range = Percent::range();
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let amount = draw_horizontal_range("Amount", value, range, "%", |v| {
            let msg = Message::Suppressor(Suppressor::Amount(Percent(v as f32)));
            ChildMessage::State(msg)
        });

        let value = suppressor.sense;
        let sense = draw_horizontal_range("Sensitivity", value, 0..=100, "%", |v| {
            let value = -120.0 + (60.0 * (v as f32 / 100.0));

            let msg = Message::Suppressor(Suppressor::Sensitivity(SuppressorSensitivity(value)));
            ChildMessage::State(msg)
        });

        let snap_spacer = Space::new().height(10.0);
        let snapshot = if !self.snapshot_running {
            Element::from(
                button(
                    text("Run Snapshot")
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_y(Alignment::Center)
                        .align_x(Alignment::Center),
                )
                .style(|t: &Theme, s| {
                    let mut style = button::primary(t, s);
                    style.border.radius = Radius::from(5.0);

                    style
                })
                .on_press(ChildMessage::Suppressor(SuppressorMessage::Start))
                .height(20.0)
                .width(Length::Fill),
            )
        } else {
            let timer = Instant::now()
                .duration_since(self.snapshot_started)
                .as_secs_f32();
            Element::from(
                stack![
                    progress_bar(0.0..=5.0, timer),
                    text("Snapshot in progress, stay quiet!")
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_y(Alignment::Center)
                        .align_x(Alignment::Center),
                ]
                .height(20.0)
                .width(Length::Fill),
            )
        };

        let mut sliders = column![amount].height(Length::Shrink).spacing(10.0);
        if is_adaptive {
            sliders = sliders.push(sense);
        } else {
            sliders = sliders.push(snap_spacer);
            sliders = sliders.push(snapshot);
        }

        let controls = column![enabled, mode, sliders]
            .padding(10.0)
            .spacing(10.0)
            .width(330);

        let canvas = Canvas::new(Suppression {
            base: self.baseline,
            live: self.current,
            range_db: (-200.0, 100.0),
        })
        .width(Length::Fill)
        .height(Length::Fill);
        let canvas = container(canvas).width(Length::Fill).height(Length::Fill);

        row![
            controls,
            Space::new().width(10.0),
            rule::vertical(3),
            canvas
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

struct Suppression {
    base: SuppressionResponse,
    live: SuppressionResponse,
    range_db: (f32, f32),
}

impl Suppression {
    fn magnitude_fraction(&self, db: f32) -> f32 {
        let (min_db, max_db) = self.range_db;
        let span = (max_db - min_db).abs().max(f32::EPSILON);

        ((db - min_db) / span).clamp(0.0, 1.0)
    }

    /// Vertical pixel position for a specific DB value
    fn y_for_db(&self, db: f32, height: f32) -> f32 {
        height * (1.0 - self.magnitude_fraction(db))
    }
}

impl<Message> canvas::Program<Message> for Suppression {
    type State = ();

    fn draw(
        &self,
        _: &Self::State,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let distance = bounds.width / (self.base.values.len() as f32 - 1.0);
        let mut start = 0.0;

        for (base, live) in self.base.values.iter().zip(self.live.values.iter()) {
            let base_y = self.y_for_db(*base, bounds.size().height);
            frame.fill_rectangle(
                Point::new(start - 2.0, base_y - 2.0),
                Size::new(4.0, 4.0),
                Color::from_rgb8(0, 255, 0),
            );

            let live_y = self.y_for_db(*live, bounds.size().height);
            frame.fill_rectangle(
                Point::new(start - 2.0, live_y - 2.0),
                Size::new(4.0, 4.0),
                Color::from_rgb8(0, 255, 255),
            );
            start += distance;
        }
        vec![frame.into_geometry()]
    }
}
