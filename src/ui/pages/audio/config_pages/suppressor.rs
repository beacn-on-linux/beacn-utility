use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config::ConfigMessage;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::pages::page::PageMessage;
use crate::ui::widgets::helpers::buttons::toggle_button;
use crate::ui::widgets::helpers::composite::draw_horizontal_range;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::suppressor::{
    Suppressor, SuppressorSensitivity, SuppressorStyle, SupressorAdaptTime,
};
use beacn_lib::types::{HasRange, Percent};
use iced::widget::{Space, checkbox, column, row};
use iced::{Element, Length, Task};
use log::debug;
use std::ops::RangeInclusive;
use std::time::Duration;
use tokio::time::sleep;

// This is awkwardly positioned, we need to prevent things like page changes while this is running.
#[derive(Debug, Clone)]
pub(crate) enum SuppressorMessage {
    Start,
    Step(u64),
    End,
}

pub struct SuppressorPage {
    snapshot_running: bool,
}

impl SuppressorPage {
    pub fn new() -> Self {
        Self {
            snapshot_running: false,
        }
    }
}

impl ConfigPage for SuppressorPage {
    fn title(&self) -> &'static str {
        "Noise Suppression"
    }

    fn update(&mut self, state: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
        let ChildMessage::Suppressor(message) = message else {
            return Task::none();
        };

        match message {
            SuppressorMessage::Start => {
                self.snapshot_running = true;
                debug!("Suppressor setup started");

                // Ok, initial messages for setup
                let msg = Message::Suppressor(Suppressor::Enabled(false));
                let _ = state.handle_message(msg);

                let msg = Message::Suppressor(Suppressor::Style(SuppressorStyle::Off));
                let _ = state.handle_message(msg);

                // No idea why this is set to 5%, but here we are.
                // let msg = Message::Suppressor(Suppressor::Amount(Percent(5.0)));
                // let _ = state.handle_message(msg);

                Task::perform(
                    async move {
                        sleep(Duration::from_millis(1500)).await;
                    },
                    |_| ChildMessage::Suppressor(SuppressorMessage::Step(100)),
                )
            }
            SuppressorMessage::Step(amount) => {
                debug!("Suppressor step: {}", amount);
                let msg =
                    Message::Suppressor(Suppressor::AdaptTime(SupressorAdaptTime(amount as f32)));
                let _ = state.handle_message(msg);

                let next = match amount {
                    100 => 1000,
                    1000 => 2000,
                    2000 => 5000,
                    5000 => 0,
                    _ => unreachable!(),
                };

                if next == 0 {
                    Task::perform(
                        async move {
                            sleep(Duration::from_millis(3000)).await;
                        },
                        |_| ChildMessage::Suppressor(SuppressorMessage::End),
                    )
                } else {
                    Task::perform(
                        async move {
                            sleep(Duration::from_millis(1500)).await;
                        },
                        move |_| ChildMessage::Suppressor(SuppressorMessage::Step(next)),
                    )
                }
            }
            SuppressorMessage::End => {
                debug!("Suppressor setup complete");
                let msg = Message::Suppressor(Suppressor::Enabled(true));
                let _ = state.handle_message(msg);

                let style = SuppressorStyle::Snapshot;
                let msg = Message::Suppressor(Suppressor::Style(style));
                let _ = state.handle_message(msg);

                let time = SupressorAdaptTime(1000.0);
                let msg = Message::Suppressor(Suppressor::AdaptTime(time));
                let _ = state.handle_message(msg);

                self.snapshot_running = false;
                Task::none()
            }
        }

        //Task::none()
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
        let text = if self.snapshot_running {
            "Snapshot in progress..."
        } else {
            "Run Snapshot"
        };
        let on_press = if self.snapshot_running {
            None
        } else {
            Some(ChildMessage::Suppressor(SuppressorMessage::Start))
        };

        let snap_button = toggle_button(text, false)
            .on_press_maybe(on_press)
            .height(20.0);

        let mut sliders = column![amount].height(Length::Shrink).spacing(10.0);
        if is_adaptive {
            sliders = sliders.push(sense);
        } else {
            sliders = sliders.push(snap_spacer);
            sliders = sliders.push(snap_button);
        }

        column![enabled, mode, sliders]
            .padding(10.0)
            .spacing(10.0)
            .width(330)
            .into()
    }
}
