use crate::AUTO_START_KEY;
use crate::window_handle::{UserEvent, send_user_event};
use egui::{Color32, Id, RichText, Ui};

pub(crate) fn settings_ui(ui: &mut Ui) {
    // Get the Auto-start state from the context
    let id = Id::new(AUTO_START_KEY);
    let value: Option<Option<bool>> = ui.ctx().memory(|mem| mem.data.get_temp::<Option<bool>>(id));
    if let Some(lookup) = value {
        if let Some(value) = lookup {
            let mut current = value;

            // Change AutoStart settings
            if ui.checkbox(&mut current, "Auto Start").changed() {
                send_user_event(ui.ctx(), UserEvent::SetAutoStart(current));
            }
        }
    } else {
        ui.label("Unable to Handle Auto-Start");
    }
}

pub(crate) fn pipeweaver_ui(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.label("Pipeweaver Placeholder Page");
    });
    ui.add_space(20.0);

    ui.label("Pipeweaver is a standalone application for Linux that provides virtual audio channels, mixing (including Personal and Audience), and routing, in a similar way to the official Beacn App");
    ui.add_space(10.0);
    ui.label("When used alongside the Beacn Utility, Mix and Mix Create functionality will become available, allowing you to manage audio channels in the same way you would on Windows");
    ui.add_space(10.0);
    ui.label("Pipeweaver is currently in early development so requires a bit of know-how to compile and run, proper releases are coming soon!");
    ui.add_space(20.0);

    let info_button = ui.add(egui::Label::new(
        RichText::new("Grab Pipeweaver Here!")
            .color(Color32::LIGHT_BLUE)
            .underline(),
    ));
    if info_button.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if info_button.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(
            "https://github.com/pipeweaver/pipeweaver",
        ));
    }
}
