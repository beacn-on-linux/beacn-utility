use crate::ui::states::{ErrorMessage, LoadState};
use beacn_lib::manager::DeviceLocation;
use egui::{RichText, Ui};

pub fn display_errors(
    ui: &mut Ui,
    load_state: &LoadState,
    device_location: &DeviceLocation,
    errors: &Vec<ErrorMessage>,
) {
    ui.add_sized(
        [ui.available_width(), ui.available_height()],
        |ui: &mut Ui| {
            ui.vertical(|ui| {
                ui.heading("An error occurred while loading the device.");
                ui.label(format!("USB Location: {}:{}", device_location.bus_number, device_location.address));
                ui.add_space(10.);
                match load_state {
                    LoadState::PermissionDenied => {
                        ui.label("Permission Denied");
                        ui.label("The application does not have permission to access the connected device.");
                        ui.add_space(5.0);
                        ui.hyperlink_to("Please visit this wiki page for help.", "https://github.com/beacn-on-linux/beacn-permissions/wiki/Installing-Device-Permission");
                    }
                    LoadState::ResourceBusy => {
                        ui.label("Resource Busy");
                        ui.label("The connected device is currently in use by another application. Please close any other applications that may be using the device and try again.");
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Note:").strong());
                            ui.label("This problem may be caused by older firmware, please ensure your device is up-to-date");
                        });
                    }
                    LoadState::Error => {
                        ui.label("Device in Error State");
                        for message in errors {
                            ui.add_space(15.0);
                            if let Some(error) = &message.error_text {
                                ui.label(format!("Error: {error:?}"));
                            }
                            if let Some(message) = &message.failed_message {
                                ui.label(format!("Message: {message:?}"));
                            }
                        }
                    }
                    _ => {
                        ui.label("Shouldn't Happen?");
                    }
                }
            }).response
        });
}
