use iced::advanced::svg::Handle;
use std::collections::HashMap;
use std::sync::LazyLock;

pub mod app;
mod page;
pub mod pages;
pub mod widgets;

pub static SVG: LazyLock<HashMap<&'static str, Handle>> = LazyLock::new(|| {
    let mut map = HashMap::default();
    map.insert(
        "mic",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/microphone.svg")),
    );
    map.insert(
        "headphones",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/headphones.svg")),
    );
    map.insert(
        "bulb",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/lightbulb.svg")),
    );
    map.insert(
        "gear",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/gear.svg")),
    );
    map.insert(
        "left_right",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/left-right.svg")),
    );
    map.insert(
        "error",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/error.svg")),
    );
    map.insert(
        "info",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/info.svg")),
    );

    // EQ Modes
    map.insert(
        "eq_bell",
        Handle::from_memory(include_bytes!("../../resources/ui/eq/bell.svg")),
    );
    map.insert(
        "eq_high_pass",
        Handle::from_memory(include_bytes!("../../resources/ui/eq/high_pass.svg")),
    );
    map.insert(
        "eq_high_shelf",
        Handle::from_memory(include_bytes!("../../resources/ui/eq/high_shelf.svg")),
    );
    map.insert(
        "eq_low_pass",
        Handle::from_memory(include_bytes!("../../resources/ui/eq/low_pass.svg")),
    );
    map.insert(
        "eq_low_shelf",
        Handle::from_memory(include_bytes!("../../resources/ui/eq/low_shelf.svg")),
    );
    map.insert(
        "eq_notch",
        Handle::from_memory(include_bytes!("../../resources/ui/eq/notch.svg")),
    );

    // Pipeweaver Logo
    map.insert(
        "pipeweaver",
        Handle::from_memory(include_bytes!("../../resources/ui/pipeweaver.svg")),
    );

    // Technically not SVGs, but I don't want a new struct..
    map.insert(
        "link",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/linked.png")),
    );
    map.insert(
        "unlink",
        Handle::from_memory(include_bytes!("../../resources/ui/icons/unlinked.png")),
    );

    map
});
