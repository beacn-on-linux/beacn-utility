use crate::pages::MicPage;
use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use beacn_mic_lib::manager::DeviceType;
use beacn_mic_lib::messages::Message;
use beacn_mic_lib::messages::headphones::Headphones;
use egui::{RichText, Ui};

#[allow(unused)]
pub struct About {}

impl About {
    pub fn new() -> Self {
        Self {}
    }
}

impl MicPage for About {
    fn icon(&self) -> &'static str {
        "gear"
    }

    fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState) {
        match state.device_type {
            DeviceType::BeacnMic => ui.heading("About Beacn Mic"),
            DeviceType::BeacnStudio => ui.heading("About Beacn Studio"),
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
