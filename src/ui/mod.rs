use egui::ahash::HashMap;
use egui::{ImageSource, include_image};
use once_cell::sync::Lazy;

pub(crate) mod app;
mod audio_pages;
mod controller_pages;
mod states;

// Main Window Icon
static ICON: &[u8] = include_bytes!("../../resources/com.github.beacn-on-linux.png");

// SVG Images
pub static SVG: Lazy<HashMap<&'static str, ImageSource>> = Lazy::new(|| {
    let mut map = HashMap::default();
    map.insert(
        "mic",
        include_image!("../../resources/icons/microphone.svg"),
    );
    map.insert(
        "bulb",
        include_image!("../../resources/icons/lightbulb.svg"),
    );
    map.insert("gear", include_image!("../../resources/icons/gear.svg"));
    map.insert("error", include_image!("../../resources/icons/error.svg"));

    // EQ Modes
    map.insert("eq_bell", include_image!("../../resources/eq/bell.svg"));
    map.insert(
        "eq_high_pass",
        include_image!("../../resources/eq/high_pass.svg"),
    );
    map.insert(
        "eq_high_shelf",
        include_image!("../../resources/eq/high_shelf.svg"),
    );
    map.insert(
        "eq_low_pass",
        include_image!("../../resources/eq/low_pass.svg"),
    );
    map.insert(
        "eq_low_shelf",
        include_image!("../../resources/eq/low_shelf.svg"),
    );
    map.insert("eq_notch", include_image!("../../resources/eq/notch.svg"));
    map
});
