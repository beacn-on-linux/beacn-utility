use crate::window_handle::{UserEvent, send_user_event};
use crate::{AUTO_START_KEY, VERSION};
use egui::{Id, RichText, Ui};

pub(crate) fn settings_ui(ui: &mut Ui) {
    ui.heading("About Beacn Utility");

    let version = RichText::new("Version: ").strong().size(14.0);
    let version_value = RichText::new(VERSION).size(14.0);

    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.label(version);
        ui.label(version_value)
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    let id = Id::new(AUTO_START_KEY);
    let value: Option<Option<bool>> = ui.ctx().memory(|mem| mem.data.get_temp::<Option<bool>>(id));
    if let Some(lookup) = value {
        if let Some(value) = lookup {
            const LABEL: &str = "Auto-Start the Beacn Utility on Login";
            let mut current = value;

            // Change AutoStart settings
            if ui.checkbox(&mut current, LABEL).changed() {
                send_user_event(ui.ctx(), UserEvent::SetAutoStart(current));
            }
        }
    } else {
        ui.label("Unable to Handle Auto-Start");
    }
}

pub(crate) fn pipeweaver_ui(ui: &mut Ui) {
    ui.label(
        RichText::new("Enhance your Beacn on Linux experience with Pipeweaver")
            .strong()
            .size(22.0),
    );
    ui.add_space(20.0);
    ui.label("Pipeweaver brings streaming-focused audio control to Linux, with mixing, routing, and separate personal and stream outputs.");
    ui.add_space(10.0);
    ui.label("If you have a Mix / Mix Create, the Beacn Utility will talk to Pipeweaver to bring volume and mix control to your devices, similar to how you've used them on Windows.");
    ui.add_space(10.0);
    ui.label("Pipeweaver isn’t running right now. If you’ve already installed it, just start it up. If not, hit the button below and give it an install!");
    ui.add_space(20.0);

    // CTA BUTTON (make it feel like a button, not a link)
    let btn = ui.add_sized(
        [160.0, 32.0],
        egui::Button::new(RichText::new("Get Pipeweaver").strong()),
    );

    if btn.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if btn.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(
            "https://github.com/pipeweaver/pipeweaver",
        ));
    }
}
