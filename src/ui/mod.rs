use egui::ahash::HashMap;
use egui::{ImageSource, include_image};
use once_cell::sync::Lazy;

pub(crate) mod app;
mod app_settings;
mod audio_pages;
mod controller_pages;
mod numbers;
mod states;
mod widgets;

// SVG Images
pub static SVG: Lazy<HashMap<&'static str, ImageSource>> = Lazy::new(|| {
    let mut map = HashMap::default();
    map.insert(
        "mic",
        include_image!("../../resources/ui/icons/microphone.svg"),
    );
    map.insert(
        "bulb",
        include_image!("../../resources/ui/icons/lightbulb.svg"),
    );
    map.insert("gear", include_image!("../../resources/ui/icons/gear.svg"));
    map.insert(
        "error",
        include_image!("../../resources/ui/icons/error.svg"),
    );

    // EQ Modes
    map.insert("eq_bell", include_image!("../../resources/ui/eq/bell.svg"));
    map.insert(
        "eq_high_pass",
        include_image!("../../resources/ui/eq/high_pass.svg"),
    );
    map.insert(
        "eq_high_shelf",
        include_image!("../../resources/ui/eq/high_shelf.svg"),
    );
    map.insert(
        "eq_low_pass",
        include_image!("../../resources/ui/eq/low_pass.svg"),
    );
    map.insert(
        "eq_low_shelf",
        include_image!("../../resources/ui/eq/low_shelf.svg"),
    );
    map.insert(
        "eq_notch",
        include_image!("../../resources/ui/eq/notch.svg"),
    );
    map
});
