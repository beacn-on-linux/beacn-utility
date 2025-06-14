use crate::window_handle::{UserEvent, send_user_event};
use egui::{Context, Id, Ui};

pub(crate) fn settings_ui(ui: &mut Ui, context: &Context) {
    // Get the Auto-start state from the context
    let id = Id::new("autostart_enabled");
    let value: Option<Option<bool>> = context.memory(|mem| mem.data.get_temp::<Option<bool>>(id));
    if let Some(lookup) = value {
        if let Some(value) = lookup {
            let mut current = value;

            // Change AutoStart settings
            if ui.checkbox(&mut current, "Auto Start").changed() {
                send_user_event(context, UserEvent::SetAutoStart(current));
            }
        }
    } else {
        ui.label("Unable to Handle Auto-Start");
    }
}
