use crate::ui::audio_pages::AudioPage;
use crate::ui::states::audio_state::BeacnAudioState;
use beacn_lib::audio::LinkChannel;
use egui::{ComboBox, Ui};
use strum::IntoEnumIterator;

pub struct Linked {}

impl Linked {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for Linked {
    fn icon(&self) -> &'static str {
        "left_right"
    }

    fn is_link_page(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        ui.label("This page requires the PC2 USB port to be plugged into a Windows PC with the Beacn Link app running.");
        ui.add_space(10.);

        let mut changed_apps = Vec::new();
        if let Some(apps) = &mut state.linked {
            if apps.is_empty() {
                ui.label("No Apps playing audio detected");
            } else {
                for app in apps {
                    ComboBox::from_label(&app.name)
                        .selected_text(self.display_name(app.channel))
                        .show_ui(ui, |ui| {
                            for channel in LinkChannel::iter() {
                                ui.add_enabled_ui(channel != LinkChannel::System, |ui| {
                                    if ui
                                        .selectable_value(
                                            &mut app.channel,
                                            channel,
                                            self.display_name(channel),
                                        )
                                        .clicked()
                                    {
                                        changed_apps.push(app.clone());
                                    }
                                });
                            }
                        });
                }
            }
        } else {
            ui.label("Unable to communicate with the Beacn Link App");
        }
        for app in changed_apps {
            let _ = state.set_link(app);
        }

        if ui.button("Refresh").clicked() {
            let _ = state.get_linked();
        }
    }
}

impl Linked {
    fn display_name(&self, channel: LinkChannel) -> &'static str {
        match channel {
            LinkChannel::System => "System",
            LinkChannel::Link1 => "Link 1",
            LinkChannel::Link2 => "Link 2",
            LinkChannel::Link3 => "Link 3",
            LinkChannel::Link4 => "Link 4",
        }
    }
}
