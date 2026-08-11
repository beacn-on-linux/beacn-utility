use iced::widget::button;
use iced::{Background, Border, Theme};

pub fn render_tab(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let mut style = button::primary(theme, status);
    let base_pallet = theme.palette();
    let palette = theme.extended_palette();

    // Clear out any Border defaults
    style.border = Border {
        color: Default::default(),
        width: 0.0,
        radius: 0.0.into(),
    };

    if active {
        style.background = Some(Background::Color(base_pallet.primary));
        style.text_color = palette.primary.base.text;
    } else {
        style.background = Some(Background::Color(base_pallet.background));
        style.text_color = palette.background.base.text;
    }
    style
}
