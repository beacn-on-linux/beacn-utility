use crate::ui::states::{ErrorMessage, LoadState};
use egui::Ui;

pub fn display_errors(ui: &mut Ui, load_state: &LoadState, errors: &Vec<ErrorMessage>) {
    ui.add_sized(
        [ui.available_width(), ui.available_height()],
        |ui: &mut Ui| {
            match load_state {
                LoadState::PermissionDenied => {
                    ui.vertical(|ui| {
                        ui.label("Permission Denied");
                        ui.label("The application does not have permission to access the connected device.");
                        ui.add_space(5.0);
                        ui.hyperlink_to("Please visit this wiki page for help.", "https://github.com/beacn-on-linux/beacn-permissions/wiki/Installing-Device-Permission");
                    })
                    .response
                }
                LoadState::ResourceBusy => {
                    ui.vertical(|ui| {
                        ui.label("Resource Busy");
                        ui.label("The connected device is currently in use by another application. Please close any other applications that may be using the device and try again.");
                    })
                    .response
                }
                LoadState::Error => {
                    ui.vertical(|ui| {
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
                    })
                    .response
                }
                _ => {
                    ui.vertical(|ui| ui.label("WHAT THE HELL IS THIS?!"))
                        .response
                }
            }
        },
    );
}
