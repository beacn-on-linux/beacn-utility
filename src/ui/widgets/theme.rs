use iced::theme::Palette;
use iced::theme::palette::Extended;
use iced::{Color, Theme};

pub fn build_beacn_theme() -> Theme {
    let base_palette = Palette {
        background: Color::from_rgb8(0x1B, 0x1B, 0x1B), // #1B1B1B Main App BG
        text: Color::from_rgb8(138, 138, 138),
        primary: Color::from_rgb8(0x00, 0x5C, 0x80), // #005C80 Active highlight accent
        success: Color::from_rgb8(40, 167, 69),
        warning: Color::from_rgb8(255, 193, 7), // FIX: Added missing warning color field parameter
        danger: Color::from_rgb8(220, 53, 69),
    };

    Theme::custom_with_fn("Beacn Utility Theme".to_string(), base_palette, |palette| {
        // Generate the default extended palette template from the base colors
        let mut extended = Extended::generate(palette);

        // Override the exact background subfield that naked rules pull from by default
        extended.background.strong.color = Color::from_rgb8(0x3C, 0x3C, 0x3C);
        extended.background.base.text = palette.text;

        extended
    })
}
