use crate::ui::audio_pages::config_pages::ConfigPage;
use crate::ui::states::audio_state::BeacnAudioState;
use crate::widgets::draw_range;
use beacn_lib::audio::BeacnAudioDevice;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphone_equaliser::HPEQType::{Bass, Mids, Treble};
use beacn_lib::audio::messages::headphone_equaliser::{HPEQValue, HeadphoneEQ};
use beacn_lib::audio::messages::headphones::HeadphoneTypes::{
    HighImpedance, InEarMonitors, LineLevel, NormalPower,
};
use beacn_lib::audio::messages::headphones::{HPLevel, HPMicMonitorLevel, Headphones};
use beacn_lib::audio::messages::subwoofer::Subwoofer;
use beacn_lib::manager::DeviceType;
use egui::Ui;
use log::debug;

pub struct HeadphonesPage;

impl ConfigPage for HeadphonesPage {
    fn title(&self) -> &'static str {
        "Headphones"
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        let device_type = state.device_definition.device_type;

        let spacing = 10.0;

        ui.horizontal_centered(|ui| {
            let mut hp = state.headphones;
            ui.add_space(spacing);
            if draw_range(ui, &mut hp.mic_monitor, -100.0..=6.0, "Mic Monitor", "dB") {
                let value = HPMicMonitorLevel(hp.mic_monitor);
                let message = match device_type {
                    DeviceType::BeacnMic => Message::Headphones(Headphones::MicMonitor(value)),
                    DeviceType::BeacnStudio => {
                        Message::Headphones(Headphones::StudioMicMonitor(value))
                    }
                    _ => panic!("This shouldn't happen."),
                };
                state.send_message(message).expect("Failed to Send Message");
                debug!("Mic Monitor Change: {:?}", hp.mic_monitor);
            }
            if ui.checkbox(&mut hp.linked, "").changed() {
                let message = match device_type {
                    DeviceType::BeacnMic => {
                        Message::Headphones(Headphones::MicChannelsLinked(hp.linked))
                    }
                    DeviceType::BeacnStudio => {
                        Message::Headphones(Headphones::StudioChannelsLinked(hp.linked))
                    }
                    _ => panic!("This shouldn't happen"),
                };
                state.send_message(message).expect("Failed to Send Message");
            }
            if draw_range(ui, &mut hp.level, -70.0..=0.0, "Headphones", "dB") {
                debug!("HP Level Change: {:?}", hp.level);
                let message = Message::Headphones(Headphones::HeadphoneLevel(HPLevel(hp.level)));
                state.send_message(message).expect("Failed to Send Message");
            }

            ui.add_space(spacing);
            ui.separator();
            ui.add_space(spacing);

            // When this changes, we also need to send disabled for all the EQ settings
            if ui.checkbox(&mut hp.fx_enabled, "").changed() {
                let messages = vec![
                    Message::Headphones(Headphones::FXEnabled(hp.fx_enabled)),
                    Message::HeadphoneEQ(HeadphoneEQ::Enabled(Bass, hp.fx_enabled)),
                    Message::HeadphoneEQ(HeadphoneEQ::Enabled(Mids, hp.fx_enabled)),
                    Message::HeadphoneEQ(HeadphoneEQ::Enabled(Treble, hp.fx_enabled)),
                    Message::Subwoofer(Subwoofer::Enabled(hp.fx_enabled)),
                ];
                for message in messages {
                    state.send_message(message).expect("Failed to Send Message");
                }
            };

            let mut eq = state.headphone_eq;
            if draw_range(ui, &mut eq.eq[Bass].amount, -12.0..=12.0, "Bass", "") {
                let value = HPEQValue(eq.eq[Bass].amount);
                let message = Message::HeadphoneEQ(HeadphoneEQ::Amount(Bass, value));
                state.send_message(message).expect("Failed to Send Message");
            }
            if draw_range(ui, &mut eq.eq[Mids].amount, -12.0..=12.0, "Mids", "") {
                let value = HPEQValue(eq.eq[Mids].amount);
                let message = Message::HeadphoneEQ(HeadphoneEQ::Amount(Mids, value));
                state.send_message(message).expect("Failed to Send Message");
            }
            if draw_range(ui, &mut eq.eq[Treble].amount, -12.0..=12.0, "Treble", "") {
                let value = HPEQValue(eq.eq[Treble].amount);
                let message = Message::HeadphoneEQ(HeadphoneEQ::Amount(Treble, value));
                state.send_message(message).expect("Failed to Send Message");
            }

            let sub = &mut state.subwoofer;
            if draw_range(ui, &mut sub.amount, 0..=10, "Subwoofer", "") {
                // Fetch the messages needed for this change
                let messages = Subwoofer::get_amount_messages(sub.amount);
                for message in messages {
                    state.send_message(message).expect("Failed to Send Message");
                }
            }

            ui.add_space(spacing);
            ui.separator();
            ui.add_space(spacing);

            ui.vertical(|ui| {
                let hp = &mut state.headphones;
                // The easiest way to handle this is to monitor the previous and see if it's
                // changed, rather than having .click or .change on each radio
                let previous = hp.headphone_type;

                ui.label("Amp Power");
                ui.add_space(10.);
                ui.radio_value(&mut hp.headphone_type, InEarMonitors, "In Ear Monitors");
                ui.radio_value(&mut hp.headphone_type, LineLevel, "Line Level");
                ui.radio_value(&mut hp.headphone_type, NormalPower, "Normal Power");
                ui.radio_value(&mut hp.headphone_type, HighImpedance, "High Impedance Mode");

                if hp.headphone_type != previous {
                    let message = Message::Headphones(Headphones::HeadphoneType(hp.headphone_type));
                    state.send_message(message).expect("Failed to Send Message");
                }
            })
        });
    }
}
