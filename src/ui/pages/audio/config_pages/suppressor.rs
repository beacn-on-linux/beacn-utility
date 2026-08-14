use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::widgets::helpers::buttons::toggle_button;
use crate::ui::widgets::helpers::composite::draw_horizontal_range;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::suppressor::{Suppressor, SuppressorSensitivity, SuppressorStyle};
use beacn_lib::types::{HasRange, Percent};
use iced::widget::{Space, checkbox, column, row};
use iced::{Element, Length, Task};
use std::ops::RangeInclusive;

pub struct SuppressorPage;

impl ConfigPage for SuppressorPage {
    fn title(&self) -> &'static str {
        "Noise Suppression"
    }

    fn update(&mut self, _state: &mut AudioState, _message: ChildMessage) -> Task<ChildMessage> {
        Task::none()
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
        let snap_button = toggle_button("Snapshot Not Supported", false).height(20.0);

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
