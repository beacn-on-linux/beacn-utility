use crate::ui::audio_pages::AudioPage;
use crate::ui::states::audio_state::BeacnAudioState;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphones::Headphones;
use beacn_lib::manager::DeviceType;
use egui::{RichText, Ui};

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

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        let device_type = state.device_definition.device_type;
        let serial_text = state.device_definition.device_info.serial.clone();
        let version_text = state.device_definition.device_info.version.to_string();
        let location_text = format!(
            "{}:{}",
            state.device_definition.location.bus_number, state.device_definition.location.address
        );

        match device_type {
            DeviceType::BeacnMic => ui.heading("About Beacn Mic"),
            DeviceType::BeacnStudio => ui.heading("About Beacn Studio"),
            _ => ui.heading("ERROR"),
        };

        let location = RichText::new("USB Location: ").strong().size(14.0);
        let serial = RichText::new("Serial: ").strong().size(14.0);
        let version = RichText::new("Version: ").strong().size(14.0);

        let location_value = RichText::new(location_text).size(14.0);
        let serial_value = RichText::new(serial_text).size(14.0);
        let version_value = RichText::new(version_text).size(14.0);

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label(location);
            ui.label(location_value)
        });
        ui.horizontal(|ui| {
            ui.label(serial);
            ui.label(serial_value)
        });
        ui.horizontal(|ui| {
            ui.label(version);
            ui.label(version_value)
        });

        if device_type == DeviceType::BeacnStudio {
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
                state.handle_message(message).expect("Failed!");
            }
        }
    }
}
