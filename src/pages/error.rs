use beacn_mic_lib::device::BeacnMic;
use egui::Ui;
use crate::pages::MicPage;
use crate::state::BeacnMicState;

pub struct ErrorPage {}

impl ErrorPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl MicPage for ErrorPage {
    fn icon(&self) -> &'static str {
        "error"
    }

    fn show_on_error(&self) -> bool {
        true
    }


    fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState) {
        ui.add_sized([ui.available_width(), ui.available_height()], |ui: &mut Ui| {
            ui.add_sized(ui.available_size(), |ui: &mut Ui| {
                ui.vertical(|ui| {
                    ui.label("Device in Error State");
                    for message in &state.device_state.errors {
                        ui.add_space(15.0);
                        if let Some(error) = &message.error_text {
                            ui.label(format!("Error: {:?}", error));
                        }
                        if let Some(message) = &message.failed_message {
                            ui.label(format!("Message: {:?}", message));
                        }
                    }
                }).response
            })
        });
    }
}