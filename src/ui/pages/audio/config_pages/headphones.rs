use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::widgets::helpers::composite::draw_range;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::eq_headphones_legacy::{EQHPLegacy, HPEQType, HPEQValue};
use beacn_lib::audio::messages::headphones::HeadphoneTypes::{
    HighImpedance, InEarMonitors, LineLevel, NormalPower,
};
use beacn_lib::audio::messages::headphones::{
    HPLevel, HPMicMonitorLevel, HeadphoneTypes, Headphones,
};
use beacn_lib::audio::messages::subwoofer::{Subwoofer, SubwooferAmount};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::HasRange;
use iced::widget::{checkbox, container, radio};
use iced::widget::{column, row, rule, text};
use iced::{Alignment, Element, Length, Padding, Task};
use std::ops::RangeInclusive;

pub struct HeadphonesPage;

#[derive(Debug, Clone)]
pub(crate) enum HeadphonesMessage {
    EQEnabled(bool),
    SubwooferAmount(u8),
    HeadphoneType(HeadphoneTypes),
}

impl ConfigPage for HeadphonesPage {
    fn title(&self) -> &'static str {
        "Headphones"
    }

    fn update(&mut self, state: &mut AudioState, message: ChildMessage) -> Task<ChildMessage> {
        let ChildMessage::Headphones(message) = message else {
            return Task::none();
        };

        match message {
            // Custom message here, need to mass enable / disable
            HeadphonesMessage::EQEnabled(enabled) => {
                let messages = vec![
                    Message::Headphones(Headphones::FXEnabled(enabled)),
                    Message::EQHPLegacy(EQHPLegacy::Enabled(HPEQType::Bass, enabled)),
                    Message::EQHPLegacy(EQHPLegacy::Enabled(HPEQType::Mids, enabled)),
                    Message::EQHPLegacy(EQHPLegacy::Enabled(HPEQType::Treble, enabled)),
                    Message::Subwoofer(Subwoofer::Enabled(enabled)),
                ];
                for message in messages {
                    let _ = state.handle_message(message);
                }
            }

            HeadphonesMessage::SubwooferAmount(amount) => {
                let version = state.device_definition.device_info.version;
                let messages = Subwoofer::get_amount_messages(amount, version);
                for message in messages {
                    let _ = state.handle_message(message);
                }
            }

            HeadphonesMessage::HeadphoneType(headphone_type) => {
                let message = Message::Headphones(Headphones::HeadphoneType(headphone_type));
                let _ = state.handle_message(message);
            }
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, ChildMessage> {
        let device_version = state.device_definition.device_info.version;

        let device_type = state.device_definition.device_type;
        let value = state.headphones.mic_monitor;
        let range = HPMicMonitorLevel::range();
        let mic_monitor = draw_range("Mic Monitor", value, range, "dB", move |v| {
            let command = match device_type {
                DeviceType::BeacnMic => Headphones::MicMonitor(HPMicMonitorLevel(v)),
                DeviceType::BeacnStudio => Headphones::StudioMicMonitor(HPMicMonitorLevel(v)),
                _ => unreachable!(),
            };

            let msg = Message::Headphones(command);
            ChildMessage::State(msg)
        });

        let value = state.headphones.linked;
        let linked = checkbox(value).on_toggle(move |v| {
            let command = match device_type {
                DeviceType::BeacnMic => Headphones::MicChannelsLinked(v),
                DeviceType::BeacnStudio => Headphones::StudioChannelsLinked(v),
                _ => unreachable!(),
            };

            let msg = Message::Headphones(command);
            ChildMessage::State(msg)
        });
        let linked = container(linked)
            .height(Length::Fill)
            .align_y(Alignment::Center);

        let value = state.headphones.level;
        let range = HPLevel::range();
        let headphones = draw_range("Headphones", value, range, "dB", |v| {
            let msg = Message::Headphones(Headphones::HeadphoneLevel(HPLevel(v)));
            ChildMessage::State(msg)
        });

        let levels = row![mic_monitor, linked, headphones].spacing(10.0);
        let levels = column![text("Level Controls"), rule::horizontal(1.0), levels]
            .spacing(7.0)
            .width(Length::Shrink)
            .align_x(Alignment::Center);

        // Headphones EQ
        let value = state.headphones.fx_enabled;
        let enabled = checkbox(value)
            .label("Equalizer")
            .on_toggle(HeadphonesMessage::EQEnabled);
        let enabled = Element::from(enabled).map(ChildMessage::Headphones);

        let value = state.eq_hp_legacy.eq[HPEQType::Bass].amount;
        let range = HPEQValue::range();
        let bass = draw_range("Bass", value, range, "dB", |v| {
            let msg = Message::EQHPLegacy(EQHPLegacy::Amount(HPEQType::Bass, HPEQValue(v)));
            ChildMessage::State(msg)
        });

        let value = state.eq_hp_legacy.eq[HPEQType::Mids].amount;
        let range = HPEQValue::range();
        let mids = draw_range("Mids", value, range, "dB", |v| {
            let msg = Message::EQHPLegacy(EQHPLegacy::Amount(HPEQType::Mids, HPEQValue(v)));
            ChildMessage::State(msg)
        });

        let value = state.eq_hp_legacy.eq[HPEQType::Treble].amount;
        let range = HPEQValue::range();
        let treble = draw_range("Treble", value, range, "dB", |v| {
            let msg = Message::EQHPLegacy(EQHPLegacy::Amount(HPEQType::Treble, HPEQValue(v)));
            ChildMessage::State(msg)
        });

        let value = state.subwoofer.amount;
        let range = SubwooferAmount::range();
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let woofer = draw_range("Subwoofer", value, range, "", |v| {
            let message = HeadphonesMessage::SubwooferAmount(v);
            ChildMessage::Headphones(message)
        });

        let eq = row![bass, mids, treble, woofer].spacing(10.0);
        let eq = column![enabled, rule::horizontal(1.0), eq]
            .spacing(7.0)
            .width(Length::Shrink)
            .align_x(Alignment::Center);

        let value = Some(state.headphones.headphone_type);
        let message = |v| ChildMessage::Headphones(HeadphonesMessage::HeadphoneType(v));
        let amp_power = column![
            radio("In Ear Monitors", InEarMonitors, value, message),
            radio("Line Level", LineLevel, value, message),
            radio("Normal Power", NormalPower, value, message),
            radio("High Impedance Mode", HighImpedance, value, message),
        ]
        .spacing(5.0);

        let amp_power = column![text("Amp Power"), rule::horizontal(1.0), amp_power]
            .spacing(7.0)
            .width(Length::Shrink)
            .align_x(Alignment::Start);

        if device_version > beacn_lib::EQ_HEADPHONES_VERSION {
            row![levels, rule::vertical(1.0), amp_power]
                .spacing(15)
                .padding(Padding {
                    top: 7.0,
                    bottom: 7.0,
                    left: 20.0,
                    right: 00.0,
                })
                .into()
        } else {
            row![
                levels,
                rule::vertical(1.0),
                eq,
                rule::vertical(1.0),
                amp_power
            ]
            .spacing(15)
            .padding(Padding {
                top: 7.0,
                bottom: 7.0,
                left: 20.0,
                right: 00.0,
            })
            .into()
        }
    }
}
