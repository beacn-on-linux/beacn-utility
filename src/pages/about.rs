use crate::pages::AudioPage;
use crate::states::audio_state::BeacnAudioState;
use beacn_lib::audio::messages::headphones::Headphones;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::BeacnAudioDevice;
use beacn_lib::manager::DeviceType;
use egui::{RichText, Ui};

#[allow(unused)]
pub struct About {}

impl About {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for About {
    fn icon(&self) -> &'static str {
        "gear"
    }

    fn show_on_error(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut Ui, mic: &Box<dyn BeacnAudioDevice>, state: &mut BeacnAudioState) {
        match state.device_type {
            DeviceType::BeacnMic => ui.heading("About Beacn Mic"),
            DeviceType::BeacnStudio => ui.heading("About Beacn Studio"),
            _ => ui.heading("ERROR"),
        };

        let serial = RichText::new("Serial: ").strong().size(14.0);
        let version = RichText::new("Version: ").strong().size(14.0);

        let serial_value = RichText::new(mic.get_serial()).size(14.0);
        let version_value = RichText::new(mic.get_version().to_string()).size(14.0);

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label(serial);
            ui.label(serial_value)
        });
        ui.horizontal(|ui| {
            ui.label(version);
            ui.label(version_value)
        });

        if state.device_type == DeviceType::BeacnStudio {
            ui.add_space(20.0);

            if ui
                .checkbox(
                    &mut state.headphones.studio_driverless,
                    "Enable PC2 4 Link Mode",
                )
                .changed()
            {
                // Send the command that we're at least aware of
                let message = Message::Headphones(Headphones::StudioDriverless(
                    state.headphones.studio_driverless,
                ));
                mic.set_value(message).expect("Failed!");
            }
        }
    }
}
