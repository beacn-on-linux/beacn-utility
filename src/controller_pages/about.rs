use crate::controller_pages::ControllerPage;
use crate::states::controller_state::ControlState;
use beacn_lib::controller::BeacnControlDevice;
use beacn_lib::manager::DeviceType;
use egui::{RichText, Ui};

#[allow(unused)]
pub struct About {}

impl About {
    pub fn new() -> Self {
        Self {}
    }
}

impl ControllerPage for About {
    fn icon(&self) -> &'static str {
        "gear"
    }

    fn show_on_error(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut Ui, dev: &Box<dyn BeacnControlDevice>, state: &mut ControlState) {
        match state.device_type {
            DeviceType::BeacnMix => ui.heading("About Beacn Mix"),
            DeviceType::BeacnMixCreate => ui.heading("About Beacn Mix Create"),
            _ => ui.heading("ERROR"),
        };

        let serial = RichText::new("Serial: ").strong().size(14.0);
        let version = RichText::new("Version: ").strong().size(14.0);

        let serial_value = RichText::new(dev.get_serial()).size(14.0);
        let version_value = RichText::new(dev.get_version().to_string()).size(14.0);

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label(serial);
            ui.label(serial_value)
        });
        ui.horizontal(|ui| {
            ui.label(version);
            ui.label(version_value)
        });
    }
}
