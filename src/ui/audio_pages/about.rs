use crate::ui::audio_pages::AudioPage;
use crate::ui::states::audio_state::BeacnAudioState;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphones::Headphones;
use beacn_lib::manager::DeviceType;
use egui::{RichText, Ui};
use crate::ui::SVG;

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

        let location = &state.device_definition.location;
        let location_text = format!("{}:{}", location.bus_number, location.address);

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

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        if let Some(inner) = &state.headphones.studio_driverless {
            let mut inner = *inner;
            const LABEL: &str = "Enable PC2 Compliancy Mode";
            if ui.checkbox(&mut inner, LABEL).changed() {
                state.headphones.studio_driverless = Some(inner);

                let message = Message::Headphones(Headphones::StudioDriverless(inner));
                state.handle_message(message).expect("Failed!");
            }
        }

        if let Some(inner) = &state.headphones.mic_class_compliant {
            let mut inner = *inner;
            const LABEL: &str = "Enable Mic Compliancy Mode";
            ui.horizontal(|ui| {
                if ui.checkbox(&mut inner, LABEL).changed() {
                    state.headphones.mic_class_compliant = Some(inner);

                    let message = Message::Headphones(Headphones::MicClassCompliant(inner));
                    state.handle_message(message).expect("Failed!");
                }

                // Add clickable info icon
                if let Some(info_icon) = SVG.get("info") {
                    let info_button = ui.add(
                        egui::ImageButton::new(egui::Image::new(info_icon.clone())
                            .fit_to_exact_size(egui::vec2(16.0, 16.0)))
                            .frame(false)
                    );

                    if info_button.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if info_button.clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(
                            "https://github.com/beacn-on-linux/beacn-utility/wiki/Beacn-Mic-Compliancy-Mode"
                        ));
                    }

                    info_button.on_hover_text("Learn more about Mic Compliancy Mode");
                }
            });
            ui.add_space(5.0);
            ui.label("Note: When changing this value, the Beacn Mic will reboot.");
        }
    }
}
