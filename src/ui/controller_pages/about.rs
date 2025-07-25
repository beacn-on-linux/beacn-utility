use crate::ui::controller_pages::ControllerPage;
use crate::ui::states::controller_state::BeacnControllerState;
use beacn_lib::manager::DeviceType;
use egui::{RichText, Slider, Ui};
use std::time::Duration;

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

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnControllerState) {
        match state.device_definition.device_type {
            DeviceType::BeacnMix => ui.heading("About Beacn Mix"),
            DeviceType::BeacnMixCreate => ui.heading("About Beacn Mix Create"),
            _ => ui.heading("ERROR"),
        };

        let serial_text = state.device_definition.device_info.serial.clone();
        let version_text = state.device_definition.device_info.version.to_string();
        let location_text = format!(
            "{}:{}",
            state.device_definition.location.bus_number, state.device_definition.location.address
        );

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
        ui.add_space(5.0);
        ui.separator();
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            ui.label("Display Brightness: ");

            let mut display_brightness = state.saved_settings.display_brightness;
            let slider = Slider::new(&mut display_brightness, 1..=100)
                .suffix("%")
                .trailing_fill(true);
            if ui.add(slider).changed() {
                let _ = state.set_display_brightness(display_brightness, true);
            }
        });

        // Button Brightness isn't a thing on the Beacn Mix, only the create
        if state.device_definition.device_type != DeviceType::BeacnMix {
            ui.horizontal(|ui| {
                ui.label("Button Brightness: ");
                let mut button_brightness = state.saved_settings.button_brightness;
                let slider = Slider::new(&mut button_brightness, 0..=10).trailing_fill(true);
                if ui.add(slider).changed() {
                    let _ = state.set_button_brightness(button_brightness, true);
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("Display Timeout: ");

            let mut display_timeout = state.saved_settings.display_dim.as_secs();
            let slider = Slider::new(&mut display_timeout, 30..=300)
                .suffix("s")
                .trailing_fill(true);
            if ui.add(slider).changed() {
                let _ = state.set_display_dim(Duration::from_secs(display_timeout), true);
            }
        });
    }
}
