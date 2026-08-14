use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::widgets::helpers::buttons::toggle_button;
use crate::ui::widgets::helpers::composite::draw_range;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::bass_enhancement::{BassAmount, BassEnhancement, BassPreset};
use beacn_lib::audio::messages::deesser::DeEsser;
use beacn_lib::audio::messages::exciter::{Exciter, ExciterFreq};
use beacn_lib::audio::messages::mic_setup::{MicGain, MicSetup, StudioMicGain};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::{HasRange, Percent};
use iced::widget::{Space, column, container, row};
use iced::widget::{rule, text};
use iced::{Alignment, Element, Length, Padding};
use std::ops::RangeInclusive;

pub struct MicrophoneSetup;

impl ConfigPage for MicrophoneSetup {
    fn title(&self) -> &'static str {
        "Mic Setup"
    }

    fn view(&self, device: &AudioState) -> Element<'_, ChildMessage> {
        let range = match device.device_definition.device_type {
            DeviceType::BeacnMic => MicGain::range(),
            DeviceType::BeacnStudio => StudioMicGain::range(),
            _ => unreachable!(),
        };
        let on_change = match device.device_definition.device_type {
            DeviceType::BeacnMic => |v: u8| {
                let msg = Message::MicSetup(MicSetup::MicGain(MicGain(v as u32)));
                ChildMessage::State(msg)
            },
            DeviceType::BeacnStudio => |v: u8| {
                let msg = Message::MicSetup(MicSetup::StudioMicGain(StudioMicGain(v as u32)));
                ChildMessage::State(msg)
            },
            _ => unreachable!(),
        };

        let value = device.mic_setup.gain;
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let mic_gain = draw_range("Mic Gain", value, range, "dB", on_change);

        // Integer percent, messaged as a float.
        let value = device.de_esser.amount;
        let range = Percent::range();
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let deesser = draw_range("De-Esser", value, range, "dB", |v| {
            ChildMessage::State(Message::DeEsser(DeEsser::Amount(Percent(v as f32))))
        });

        let value = device.bass_enhancement.amount;
        let range = BassAmount::range();
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let bass_amnt = draw_range("Amount", value, range, "dB", |v| {
            ChildMessage::State(Message::BassEnhancement(BassEnhancement::Amount(
                BassAmount(v as f32),
            )))
        });

        let value = device.exciter.amount;
        let range = Percent::range();
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let exc_amnt = draw_range("Amount", value, range, "dB", |v| {
            ChildMessage::State(Message::Exciter(Exciter::Amount(Percent(v as f32))))
        });

        let value = device.exciter.freq;
        let range = ExciterFreq::range();
        let range: RangeInclusive<u16> = (*range.start() as u16)..=(*range.end() as u16);
        let exc_freq = draw_range("Frequency", value, range, "dB", |v| {
            ChildMessage::State(Message::Exciter(Exciter::Frequency(ExciterFreq(v as f32))))
        });

        let current = device.bass_enhancement.preset;
        let p1 = toggle_button("1", current == BassPreset::Preset1)
            .on_press(ChildMessage::State(Message::BassEnhancement(
                BassEnhancement::Preset(BassPreset::Preset1),
            )))
            .width(Length::Fixed(35.0))
            .height(Length::Fixed(35.0));
        let p2 = toggle_button("2", current == BassPreset::Preset2)
            .on_press(ChildMessage::State(Message::BassEnhancement(
                BassEnhancement::Preset(BassPreset::Preset2),
            )))
            .width(Length::Fixed(35.0))
            .height(Length::Fixed(35.0));
        let p3 = toggle_button("3", current == BassPreset::Preset3)
            .on_press(ChildMessage::State(Message::BassEnhancement(
                BassEnhancement::Preset(BassPreset::Preset3),
            )))
            .width(Length::Fixed(35.0))
            .height(Length::Fixed(35.0));
        let p4 = toggle_button("4", current == BassPreset::Preset4)
            .on_press(ChildMessage::State(Message::BassEnhancement(
                BassEnhancement::Preset(BassPreset::Preset4),
            )))
            .width(Length::Fixed(35.0))
            .height(Length::Fixed(35.0));

        // 2x2 grid to put them in
        let presets = column![row![p1, p2].spacing(8.0), row![p3, p4].spacing(8.0)].spacing(8.0);
        let preset_layout =
            column![text("Style"), Space::new().height(8.0), presets].align_x(Alignment::Center);

        let bass_bottom = row![preset_layout, bass_amnt].spacing(10.0);
        let bass_layout = column![text("Bass Enhancer"), rule::horizontal(1.0), bass_bottom]
            .spacing(7.0)
            .width(Length::Shrink)
            .align_x(Alignment::Center);

        let exciter_sliders = row![exc_amnt, exc_freq].spacing(10.0);
        let exciter_layout = column![text("Exciter"), rule::horizontal(1.0), exciter_sliders]
            .spacing(7.0)
            .width(Length::Shrink)
            .align_x(Alignment::Center);

        let layout = row![
            mic_gain,
            rule::vertical(1.0),
            deesser,
            rule::vertical(1.0),
            bass_layout,
            rule::vertical(1.0),
            exciter_layout,
            rule::vertical(1.0)
        ]
        .spacing(20.0)
        .padding(Padding {
            top: 7.0,
            bottom: 7.0,
            left: 20.0,
            right: 00.0,
        });

        // let output: Element<'_, MicrophoneSetupMessage> = container(layout).into();
        // output.map(ChildMessage::MicrophoneSetup)
        container(layout).into()
    }
}
