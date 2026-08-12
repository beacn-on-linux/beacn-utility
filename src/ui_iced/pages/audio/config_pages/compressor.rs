use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::audio::config_pages::{ChildMessage, ConfigPage, map_to_range};
use crate::ui_iced::widgets::helpers::buttons::toggle_button;
use crate::ui_iced::widgets::helpers::composite::{draw_horizontal_range, draw_range};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::compressor::{
    Compressor, CompressorMode, CompressorRatio, CompressorThreshold,
};
use beacn_lib::types::{HasRange, MakeUpGain, TimeFrame};
use iced::widget::{Space, checkbox, column, container, row};
use iced::{Center, Element, Length, Task};
use strum::IntoEnumIterator;

pub struct CompressorPage;

#[derive(Debug, Clone)]
pub(crate) enum CompressorMessage {
    SetEnabled(bool),
}

impl ConfigPage for CompressorPage {
    fn title(&self) -> &'static str {
        "Compressor"
    }

    fn update(&mut self, state: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
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

        let enabled = checkbox(values.enabled).on_toggle(|v| CompressorMessage::SetEnabled(v));
        let enabled = Element::from(enabled).map(ChildMessage::Compressor);
        let enabled = row![enabled, "Enabled"].spacing(6).align_y(Center);

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
        let threshold = draw_horizontal_range("Threshold", value, range, 1, "dB", move |v| {
            let msg = Compressor::Threshold(compressor_mode, CompressorThreshold(v as f32));
            let msg = Message::Compressor(msg);
            ChildMessage::State(msg)
        });

        // In simple mode, we just have an 'amount', but that internally maps to the Ratio
        let value = map_to_range(values.ratio, 1.0, 10.0, 0.0, 10.0).round() as u8;
        let amount = draw_horizontal_range("Amount", value, 0..=10, 1, "", |v| {
            let ratio = map_to_range(v as f32, 0.0, 10.0, 1.0, 10.0);
            let ratio = (ratio * 100.0).round() / 100.0;
            let ratio = CompressorRatio(ratio);
            let msg = Message::Compressor(Compressor::Ratio(CompressorMode::Simple, ratio));

            ChildMessage::State(msg)
        });

        // For everything else, there's mastercard
        let value = values.ratio;
        let range = CompressorRatio::range();
        let ratio = draw_horizontal_range("Ratio", value, range, 0.1, ":1", move |v| {
            let msg = Message::Compressor(Compressor::Ratio(compressor_mode, CompressorRatio(v)));
            ChildMessage::State(msg)
        });

        let value = values.attack;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let attack = draw_horizontal_range("Attack", value, range, 10, "ms", move |v| {
            let msg = Message::Compressor(Compressor::Attack(compressor_mode, TimeFrame(v as f32)));
            ChildMessage::State(msg)
        });

        let value = values.release;
        let range = TimeFrame::range();
        let range = (*range.start() as u16)..=(*range.end() as u16);
        let release = draw_horizontal_range("Release", value, range, 10, "ms", move |v| {
            let msg = Compressor::Release(compressor_mode, TimeFrame(v as f32));
            let msg = Message::Compressor(msg);
            ChildMessage::State(msg)
        });

        let value = values.makeup;
        let range = MakeUpGain::range();
        let makeup = draw_range("Make-Up Gain", value, range, 0.1, "dB", move |v| {
            let msg = Compressor::MakeupGain(compressor_mode, MakeUpGain(v));
            let msg = Message::Compressor(msg);
            ChildMessage::State(msg)
        });
        let makeup = container(makeup).padding(10);

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

        row![fields, makeup].spacing(10.0).into()
    }
}
