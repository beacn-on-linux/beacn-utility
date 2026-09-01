use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage, map_to_range};
use crate::ui::widgets::helpers::buttons::toggle_button;
use crate::ui::widgets::helpers::composite::{draw_horizontal_range, draw_range};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::compressor::{
    Compressor, CompressorMode, CompressorRatio, CompressorThreshold,
};
use beacn_lib::types::{HasRange, MakeUpGain, TimeFrame};
use iced::widget::canvas::{Frame, Geometry};
use iced::widget::{Canvas, Space, canvas, checkbox, column, container, row};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Task, Theme, mouse};
use strum::IntoEnumIterator;

pub struct CompressorPage {
    input_amount: f32,
    attenuation: f32,
    output_amount: f32,
}

#[derive(Debug, Clone)]
pub(crate) enum CompressorMessage {
    SetEnabled(bool),
}

impl ConfigPage for CompressorPage {
    fn title(&self) -> &'static str {
        "Compressor"
    }

    fn update(&mut self, state: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
        if let ChildMessage::Meters(meters) = message {
            self.input_amount = meters.pre_compressor;
            self.output_amount = meters.post_compressor;
            self.attenuation = meters.compressor_attenuation;

            return Task::none();
        }

        let ChildMessage::Compressor(message) = message else {
            return Task::none();
        };

        match message {
            // Custom message here, the enabled button applies to both modes to prevent confusion!
            CompressorMessage::SetEnabled(enabled) => {
                for mode in CompressorMode::iter() {
                    let msg = Compressor::Enabled(mode, enabled);
                    let msg = Message::Compressor(msg);
                    state.handle_message(msg).expect("Failed");
                }
            }
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, ChildMessage> {
        let compressor_mode = state.compressor.mode;

        let compressor = &state.compressor;
        let values = &compressor.values[compressor_mode];

        let enabled = checkbox(values.enabled)
            .label("Enabled")
            .on_toggle(CompressorMessage::SetEnabled);
        let enabled = Element::from(enabled).map(ChildMessage::Compressor);

        let enabled = column![enabled, Space::new().height(10.0)];

        let is_simple = compressor_mode == CompressorMode::Simple;
        let simple = toggle_button("Simple", is_simple).on_press_with(|| {
            let msg = Message::Compressor(Compressor::Mode(CompressorMode::Simple));
            ChildMessage::State(msg)
        });

        let is_advanced = compressor_mode == CompressorMode::Advanced;
        let advanced = toggle_button("Advanced", is_advanced).on_press_with(|| {
            let msg = Message::Compressor(Compressor::Mode(CompressorMode::Advanced));
            ChildMessage::State(msg)
        });

        let mode = row![simple, advanced].spacing(8.0).height(20);

        // Threshold always appears
        let value = values.threshold;
        let range = CompressorThreshold::range();
        let range = (*range.start() as i8)..=(*range.end() as i8);
        let threshold = draw_horizontal_range("Threshold", value, range, "dB", move |v| {
            let msg = Compressor::Threshold(compressor_mode, CompressorThreshold(v as f32));
            let msg = Message::Compressor(msg);
            ChildMessage::State(msg)
        });

        // In simple mode, we just have an 'amount', but that internally maps to the Ratio
        let value = map_to_range(values.ratio, 1.0, 10.0, 0.0, 10.0).round() as u8;
        let amount = draw_horizontal_range("Amount", value, 0..=10, "", |v| {
            let ratio = map_to_range(v as f32, 0.0, 10.0, 1.0, 10.0);
            let ratio = (ratio * 100.0).round() / 100.0;
            let ratio = CompressorRatio(ratio);
            let msg = Message::Compressor(Compressor::Ratio(CompressorMode::Simple, ratio));

            ChildMessage::State(msg)
        });

        // For everything else, there's mastercard
        let value = values.ratio;
        let range = CompressorRatio::range();
        let ratio = draw_horizontal_range("Ratio", value, range, ":1", move |v| {
            let msg = Message::Compressor(Compressor::Ratio(compressor_mode, CompressorRatio(v)));
            ChildMessage::State(msg)
        });

        let value = values.attack;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let attack = draw_horizontal_range("Attack", value, range, "ms", move |v| {
            let msg = Message::Compressor(Compressor::Attack(compressor_mode, TimeFrame(v as f32)));
            ChildMessage::State(msg)
        });

        let value = values.release;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let release = draw_horizontal_range("Release", value, range, "ms", move |v| {
            let msg = Compressor::Release(compressor_mode, TimeFrame(v as f32));
            let msg = Message::Compressor(msg);
            ChildMessage::State(msg)
        });

        let value = values.makeup;
        let range = MakeUpGain::range();
        let makeup = draw_range("Make-Up Gain", value, range, "dB", move |v| {
            let msg = Compressor::MakeupGain(compressor_mode, MakeUpGain(v));
            let msg = Message::Compressor(msg);
            ChildMessage::State(msg)
        });
        let makeup = container(makeup).padding(7);

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

        // Ok, lets render the 'stuff'..
        let input_meter = Canvas::new(Meter {
            db: self.input_amount,
            range_db: (-50.0, 0.0),
            anchor: MeterAnchor::Bottom,
            fill_color: Color::from_rgb8(0, 92, 128),
        })
        .width(Length::Fixed(8.0))
        .height(Length::Fill);

        let attenuation_meter = Canvas::new(Meter {
            db: self.attenuation,
            range_db: (-20.0, 0.0),
            anchor: MeterAnchor::Top,
            fill_color: Color::from_rgb(0.95, 0.35, 0.3),
        })
        .width(Length::Fixed(8.0));

        let attenuation_column = column![
            attenuation_meter.height(Length::FillPortion(20)),
            Space::new().height(Length::FillPortion(30)),
        ];

        let output_meter = Canvas::new(Meter {
            db: self.output_amount,
            range_db: (-50.0, 0.0),
            anchor: MeterAnchor::Bottom,
            fill_color: Color::from_rgb8(0, 92, 128),
        })
        .width(Length::Fixed(8.0))
        .height(Length::Fill);

        let meters = row![input_meter, attenuation_column, output_meter]
            .padding(7.0)
            .spacing(5.0);

        row![fields, meters, makeup].spacing(10.0).into()
    }
}

impl CompressorPage {
    pub(crate) fn new() -> Self {
        Self {
            input_amount: 0.0,
            attenuation: 0.0,
            output_amount: 0.0,
        }
    }
}

enum MeterAnchor {
    Bottom,
    Top,
}

// TODO: This might need to be moved
// With that said, it's currently only relevant for the compressor, the only other
// linear meter is the mic amplitude meter, but that has different drawing.
struct Meter {
    db: f32,

    range_db: (f32, f32),
    anchor: MeterAnchor,
    fill_color: Color,
}

impl Meter {
    /// Fraction in [0,1] of the track height that a dB value covers.
    fn magnitude_fraction(&self, db: f32) -> f32 {
        let (min_db, max_db) = self.range_db;
        let span = (max_db - min_db).abs().max(f32::EPSILON);
        match self.anchor {
            MeterAnchor::Bottom => ((db - min_db) / span).clamp(0.0, 1.0),
            MeterAnchor::Top => ((max_db - db) / span).clamp(0.0, 1.0),
        }
    }

    /// Vertical pixel position for a specific DB value
    fn y_for_db(&self, db: f32, track_top: f32, track_height: f32) -> f32 {
        match self.anchor {
            MeterAnchor::Bottom => track_top + track_height * (1.0 - self.magnitude_fraction(db)),
            MeterAnchor::Top => track_top + track_height * self.magnitude_fraction(db),
        }
    }
}

impl<Message> canvas::Program<Message> for Meter {
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

        // Track background
        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.size().width, bounds.size().height),
            Color::from_rgb8(60, 60, 60),
        );

        // Filled portion, growing from this meter's anchor edge toward the
        // current value.
        let anchor_y = match self.anchor {
            MeterAnchor::Bottom => bounds.size().height,
            MeterAnchor::Top => 0.0,
        };
        let value_y = self.y_for_db(self.db, 0.0, bounds.size().height);
        let (fill_y, fill_height) = if value_y <= anchor_y {
            (value_y, anchor_y - value_y)
        } else {
            (anchor_y, value_y - anchor_y)
        };
        frame.fill_rectangle(
            Point::new(0.0, fill_y),
            Size::new(bounds.size().width, fill_height),
            self.fill_color,
        );

        vec![frame.into_geometry()]
    }
}
