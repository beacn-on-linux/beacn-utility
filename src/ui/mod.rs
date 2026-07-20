use egui::{ImageSource, include_image};
use std::collections::HashMap;
use std::sync::LazyLock;

pub(crate) mod app;
mod audio_pages;
mod controller_pages;
mod numbers;
mod pages;
mod shared_pages;
mod states;
mod widgets;

// SVG Images
pub static SVG: LazyLock<HashMap<&'static str, ImageSource>> = LazyLock::new(|| {
    let mut map = HashMap::default();
    map.insert(
        "mic",
        include_image!("../../resources/ui/icons/microphone.svg"),
    );
    map.insert(
        "headphones",
        include_image!("../../resources/ui/icons/headphones.svg"),
    );
    map.insert(
        "bulb",
        include_image!("../../resources/ui/icons/lightbulb.svg"),
    );
    map.insert("gear", include_image!("../../resources/ui/icons/gear.svg"));
    map.insert(
        "left_right",
        include_image!("../../resources/ui/icons/left-right.svg"),
    );
    map.insert(
        "error",
        include_image!("../../resources/ui/icons/error.svg"),
    );
    map.insert("info", include_image!("../../resources/ui/icons/info.svg"));

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

    // Pipeweaver Logo
    map.insert(
        "pipeweaver",
        include_image!("../../resources/ui/pipeweaver.svg"),
    );

    // Technically not SVGs, but I don't want a new struct..
    map.insert(
        "link",
        include_image!("../../resources/ui/icons/linked.png"),
    );
    map.insert(
        "unlink",
        include_image!("../../resources/ui/icons/unlinked.png"),
    );

    map
});
