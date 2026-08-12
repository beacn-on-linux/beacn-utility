use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::audio::config_pages::{ChildMessage, ConfigPage, map_to_range};
use crate::ui_iced::widgets::helpers::buttons::toggle_button;
use crate::ui_iced::widgets::helpers::composite::draw_horizontal_range;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::expander::{
    Expander, ExpanderMode, ExpanderRatio, ExpanderThreshold,
};
use beacn_lib::types::{HasRange, TimeFrame};
use iced::widget::{Space, checkbox, column, row};
use iced::{Center, Element, Length, Task};
use strum::IntoEnumIterator;

pub struct ExpanderPage;

#[derive(Debug, Clone)]
pub(crate) enum ExpanderMessage {
    SetEnabled(bool),
}

impl ConfigPage for ExpanderPage {
    fn title(&self) -> &'static str {
        "Expander"
    }

    fn update(&mut self, state: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
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

        let enabled = checkbox(values.enabled).on_toggle(|v| ExpanderMessage::SetEnabled(v));
        let enabled = Element::from(enabled).map(ChildMessage::Expander);
        let enabled = row![enabled, "Enabled"].spacing(6).align_y(Center);

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
        let threshold = draw_horizontal_range("Threshold", value, range, 1, "dB", move |v| {
            let msg = Expander::Threshold(expander_mode, ExpanderThreshold(v as f32));
            let msg = Message::Expander(msg);
            ChildMessage::State(msg)
        });

        // In simple mode, we just have an 'amount', but that internally maps to the Ratio
        let value = Self::ratio_to_precent(values.ratio);
        let amount = draw_horizontal_range("Amount", value, 0..=100, 1, "%", |v| {
            let ratio = ExpanderRatio(Self::percent_to_ratio(v));
            let msg = Message::Expander(Expander::Ratio(ExpanderMode::Simple, ratio));

            ChildMessage::State(msg)
        });

        // For everything else, there's mastercard
        let value = values.ratio;
        let range = ExpanderRatio::range();
        let ratio = draw_horizontal_range("Ratio", value, range, 0.1, ":1", move |v| {
            let msg = Message::Expander(Expander::Ratio(expander_mode, ExpanderRatio(v)));
            ChildMessage::State(msg)
        });

        let value = values.attack;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let attack = draw_horizontal_range("Attack", value, range, 10, "ms", move |v| {
            let msg = Message::Expander(Expander::Attack(expander_mode, TimeFrame(v as f32)));
            ChildMessage::State(msg)
        });

        let value = values.release;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let release = draw_horizontal_range("Release", value, range, 10, "ms", move |v| {
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

        column![enabled, mode, fields]
            .padding(10.0)
            .spacing(10.0)
            .width(330)
            .into()
    }
}

impl ExpanderPage {
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
