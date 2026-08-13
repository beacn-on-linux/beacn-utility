use crate::devices::states::audio::AudioState;
use crate::ui_iced::pages::page::{AudioPage, PageMessage};
use crate::ui_iced::widgets::helpers::composite::draw_lighting_range;
use crate::ui_iced::widgets::helpers::drag_value::styled_drag_value;
use crate::ui_iced::widgets::helpers::slider::themed_slider;
use crate::ui_iced::widgets::render::pop_over::Popover;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::lighting::{
    Lighting, LightingBrightness, LightingMeterSensitivty, LightingMeterSource, LightingMode,
    LightingMuteMode, LightingSpeed, LightingSuspendBrightness, LightingSuspendMode,
    StudioLightingMode,
};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::{HasRange, RGBA};
use iced::border::Radius;
use iced::widget::button::Status;
use iced::widget::{Space, button, column, container, pick_list, radio, row, rule, text};
use iced::{Alignment, Border, Color, Element, Length, Task, Theme};
use iced_color_picker::{ColorPicker, Hsv, Spectrum};
use iced_futures::core::Background;
use std::cmp::PartialEq;
use std::fmt::{Display, Formatter};

#[derive(Debug, Copy, Clone)]
pub(crate) enum LightingMessage {
    State(Message),
    TogglePicker(Picker),
    ClosePicker,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Picker {
    SolidColour,

    GradientPrimary,
    GradientSecondary,

    ReactivePrimary,
    ReactiveSecondary,

    SparklePrimary,
    SparkleSecondary,

    MuteColour,
}

pub struct LightingPage {
    active_picker: Option<Picker>,
}

impl LightingPage {
    pub fn new() -> Self {
        Self {
            active_picker: None,
        }
    }
}

impl AudioPage for LightingPage {
    fn icon(&self) -> &'static str {
        "bulb"
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        let PageMessage::AudioLightingPage(message) = message else {
            return Task::none();
        };

        match message {
            LightingMessage::State(msg) => {
                let _ = state.handle_message(msg);
            }
            LightingMessage::TogglePicker(picker) => {
                if self.active_picker == Some(picker) {
                    self.active_picker = None;
                }
                self.active_picker = Some(picker);
            }
            LightingMessage::ClosePicker => {
                self.active_picker = None;
            }
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, PageMessage> {
        let styles = self.lighting_styles(state);
        //
        let top = row![
            container(
                column![
                    text("Lighting Style").color(Color::WHITE),
                    row![Space::new().width(20), column(styles).spacing(4)],
                ]
                .spacing(10)
            )
            .width(Length::Fixed(140.0)),
            rule::vertical(1),
            container(self.lighting_controls(state)).width(Length::Fill),
        ]
        .spacing(15)
        .align_y(Alignment::Start);

        let bottom = row![
            self.mute_options(state),
            rule::vertical(1),
            self.suspend_options(state),
        ]
        .height(130)
        .spacing(15)
        .align_y(Alignment::Start);

        let ele = Element::from(
            column![
                top,
                rule::horizontal(1),
                text("Other Lighting Options (note, this does not work cleanly under Linux)")
                    .color(Color::from_rgb8(255, 255, 255)),
                rule::horizontal(1),
                bottom,
            ]
            .spacing(10)
            .padding(10),
        );

        // Everything returns state messages, so we'll just map them directly back.
        ele.map(PageMessage::AudioLightingPage)
    }
}

impl LightingPage {
    fn lighting_styles(&self, state: &AudioState) -> Vec<Element<'_, LightingMessage>> {
        match state.device_definition.device_type {
            DeviceType::BeacnMic => vec![
                self.style_radio(
                    "Solid Colour",
                    state.lighting.mic_mode == LightingMode::Solid,
                    Lighting::Mode(LightingMode::Solid),
                ),
                self.style_radio(
                    "Gradient",
                    state.lighting.mic_mode == LightingMode::Gradient,
                    Lighting::Mode(LightingMode::Gradient),
                ),
                self.style_radio(
                    "Reactive Meter",
                    matches!(
                        state.lighting.mic_mode,
                        LightingMode::ReactiveRing
                            | LightingMode::ReactiveMeterUp
                            | LightingMode::ReactiveMeterDown
                    ),
                    Lighting::Mode(LightingMode::ReactiveRing),
                ),
                self.style_radio(
                    "Sparkle",
                    matches!(
                        state.lighting.mic_mode,
                        LightingMode::SparkleMeter | LightingMode::SparkleRandom
                    ),
                    Lighting::Mode(LightingMode::SparkleRandom),
                ),
                self.style_radio(
                    "Spectrum Cycle",
                    state.lighting.mic_mode == LightingMode::Spectrum,
                    Lighting::Mode(LightingMode::Spectrum),
                ),
            ],

            DeviceType::BeacnStudio => vec![
                self.style_radio(
                    "Solid Colour",
                    state.lighting.studio_mode == StudioLightingMode::Solid,
                    Lighting::StudioMode(StudioLightingMode::Solid),
                ),
                self.style_radio(
                    "Peak Meter",
                    state.lighting.studio_mode == StudioLightingMode::PeakMeter,
                    Lighting::StudioMode(StudioLightingMode::PeakMeter),
                ),
                self.style_radio(
                    "Solid Spectrum",
                    state.lighting.studio_mode == StudioLightingMode::SolidSpectrum,
                    Lighting::StudioMode(StudioLightingMode::SolidSpectrum),
                ),
            ],

            _ => unreachable!(),
        }
    }

    fn style_radio<'a>(
        &self,
        label: &'a str,
        active: bool,
        msg: Lighting,
    ) -> Element<'a, LightingMessage> {
        let msg = LightingMessage::State(Message::Lighting(msg));

        // Assemble the button with your precise state colors mapped
        button(label)
            .style(move |theme: &Theme, status: Status| {
                let palette = theme.palette();

                let bg_color = if active {
                    palette.primary
                } else {
                    match status {
                        // Hovered state background: #464646
                        Status::Hovered | Status::Pressed => Color::from_rgb8(0x46, 0x46, 0x46),
                        _ => Color::TRANSPARENT,
                    }
                };

                let text_colour = match active {
                    true => Color::WHITE,
                    false => match status {
                        Status::Hovered => Color::WHITE,
                        Status::Active | Status::Pressed | Status::Disabled => {
                            Color::from_rgb8(120, 120, 120)
                        }
                    },
                };

                let border_style = match status {
                    Status::Hovered if !active => Border {
                        radius: 5.0.into(),
                        width: 1.0,
                        color: Color::from_rgb8(0x96, 0x96, 0x96),
                    },
                    _ => Border {
                        radius: 5.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                };

                button::Style {
                    background: Some(bg_color.into()),
                    text_color: text_colour.into(),
                    border: border_style,
                    shadow: iced::Shadow::default(),
                    snap: false,
                }
            })
            .width(Length::Fill)
            .on_press(msg)
            .into()
    }

    fn lighting_controls(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        match state.device_definition.device_type {
            DeviceType::BeacnMic => match state.lighting.mic_mode {
                LightingMode::Solid => self.solid_controls(state),
                LightingMode::Gradient => self.gradient_controls(state),
                LightingMode::Spectrum => self.spectrum_controls(state),

                LightingMode::ReactiveRing
                | LightingMode::ReactiveMeterUp
                | LightingMode::ReactiveMeterDown => self.reactive_controls(state),

                LightingMode::SparkleRandom | LightingMode::SparkleMeter => {
                    self.sparkle_controls(state)
                }
            },

            DeviceType::BeacnStudio => match state.lighting.studio_mode {
                StudioLightingMode::Solid => self.solid_controls(state),
                StudioLightingMode::PeakMeter => self.reactive_controls(state),
                StudioLightingMode::SolidSpectrum => self.spectrum_controls(state),
            },

            _ => text("Unknown device").into(),
        }
    }

    fn solid_controls(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        column![
            self.colour(
                "Primary Colour",
                &state.lighting.colour1,
                Picker::SolidColour,
                Lighting::Colour1
            ),
            self.brightness(state, Length::Shrink),
        ]
        .spacing(15)
        .into()
    }

    fn gradient_controls(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        column![
            self.colour(
                "Primary Colour",
                &state.lighting.colour1,
                Picker::GradientPrimary,
                Lighting::Colour1
            ),
            self.colour(
                "Secondary Colour",
                &state.lighting.colour2,
                Picker::GradientSecondary,
                Lighting::Colour2
            ),
            Space::new().height(10),
            self.speed(state, Length::Fixed(130.0)),
            self.brightness(state, Length::Fixed(130.0)),
        ]
        .spacing(8)
        .into()
    }

    fn spectrum_controls(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        column![
            self.speed(state, Length::Fixed(130.0)),
            self.brightness(state, Length::Fixed(130.0)),
        ]
        .spacing(8)
        .into()
    }

    fn reactive_controls(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        let behaviour = if state.device_definition.device_type == DeviceType::BeacnMic {
            column![
                text("Behaviour"),
                radio(
                    "Whole Ring Meter",
                    LightingMode::ReactiveRing,
                    Some(state.lighting.mic_mode),
                    Lighting::Mode
                ),
                radio(
                    "Bar Meter Up",
                    LightingMode::ReactiveMeterUp,
                    Some(state.lighting.mic_mode),
                    Lighting::Mode
                ),
                radio(
                    "Bar Meter Down",
                    LightingMode::ReactiveMeterDown,
                    Some(state.lighting.mic_mode),
                    Lighting::Mode
                ),
            ]
            .spacing(4)
        } else {
            column![]
        };

        let behaviour = Element::from(behaviour);
        let behaviour = behaviour.map(Message::Lighting).map(LightingMessage::State);

        column![
            behaviour,
            Space::new().height(10),
            self.colour(
                "Primary Colour",
                &state.lighting.colour1,
                Picker::ReactivePrimary,
                Lighting::Colour1,
            ),
            self.colour(
                "Secondary Colour",
                &state.lighting.colour2,
                Picker::ReactiveSecondary,
                Lighting::Colour2,
            ),
            Space::new().height(10),
            self.sensitivity(state, Length::Fixed(110.0)),
            self.brightness(state, Length::Fixed(110.0)),
            Space::new().height(10),
            self.meter_source(state, Length::Fixed(110.0)),
        ]
        .spacing(8)
        .into()
    }

    fn sparkle_controls(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        let selector = column![
            text("Behaviour"),
            radio(
                "Sparkle Random",
                LightingMode::SparkleRandom,
                Some(state.lighting.mic_mode),
                Lighting::Mode,
            ),
            radio(
                "Sparkle Meter",
                LightingMode::SparkleMeter,
                Some(state.lighting.mic_mode),
                Lighting::Mode,
            ),
        ]
        .spacing(4);

        let selector = Element::from(selector);
        let selector = selector.map(Message::Lighting).map(LightingMessage::State);

        column![
            selector,
            Space::new().height(10),
            self.colour(
                "Primary Colour",
                &state.lighting.colour1,
                Picker::SparklePrimary,
                Lighting::Colour1,
            ),
            self.colour(
                "Secondary Colour",
                &state.lighting.colour2,
                Picker::SparkleSecondary,
                Lighting::Colour2,
            ),
            Space::new().height(10),
            self.sensitivity(state, Length::Fixed(130.0)),
            self.speed(state, Length::Fixed(130.0)),
            self.brightness(state, Length::Fixed(130.0)),
            Space::new().height(10),
            self.meter_source(state, Length::Fixed(130.0)),
        ]
        .spacing(8)
        .into()
    }

    fn labeled_slider<'a>(
        label: &'a str,
        value: i32,
        range: std::ops::RangeInclusive<i32>,
        width: Length,
        on_change: impl Fn(i32) -> Lighting + Clone + 'a,
    ) -> Element<'a, LightingMessage> {
        let on_change = move |value| LightingMessage::State(Message::Lighting(on_change(value)));
        draw_lighting_range(label, value, range, "", width, on_change).into()
    }

    fn mute_options<'a>(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        let mode = state.lighting.mute_mode;

        let mut content = column![
            text("When Muted").color(Color::from_rgb8(255, 255, 255)),
            Space::new().height(4),
            radio(
                "Do Nothing",
                LightingMuteMode::Nothing,
                Some(mode),
                |mode| LightingMessage::State(Message::Lighting(Lighting::MuteMode(mode)))
            ),
            radio(
                "Turn off LED ring",
                LightingMuteMode::Off,
                Some(mode),
                |mode| LightingMessage::State(Message::Lighting(Lighting::MuteMode(mode)))
            ),
            radio(
                "Turn LED ring to a solid colour",
                LightingMuteMode::Solid,
                Some(mode),
                |mode| LightingMessage::State(Message::Lighting(Lighting::MuteMode(mode)))
            ),
        ]
        .spacing(6);

        if mode == LightingMuteMode::Solid {
            content = content.push(Space::new().height(3));
            content = content.push(self.colour(
                "Mute Colour",
                &state.lighting.mute_colour,
                Picker::MuteColour,
                Lighting::MuteColour,
            ));
        }

        content.into()
    }

    fn suspend_options<'a>(&self, state: &AudioState) -> Element<'_, LightingMessage> {
        let mode = state.lighting.suspend_mode;

        let mut content = column![
            text("When USB is Suspended").color(Color::from_rgb8(255, 255, 255)),
            Space::new().height(4),
            radio(
                "Do Nothing",
                LightingSuspendMode::Nothing,
                Some(mode),
                Lighting::SuspendMode
            ),
            radio(
                "Turn off LED ring",
                LightingSuspendMode::Off,
                Some(mode),
                Lighting::SuspendMode
            ),
            radio(
                "Change the brightness",
                LightingSuspendMode::Brightness,
                Some(mode),
                Lighting::SuspendMode
            ),
        ]
        .spacing(6);

        if mode == LightingSuspendMode::Brightness {
            content = content.push(Space::new().height(2));

            let value = state.lighting.suspend_brightness;
            let range = LightingSuspendBrightness::range();

            let brightness = draw_lighting_range(
                "Suspend Brightness",
                value,
                range,
                "",
                Length::Fixed(140.0),
                |v| Lighting::SuspendBrightness(LightingSuspendBrightness(v)),
            );
            content = content.push(brightness);
        }

        let content = Element::from(content);
        content
            .map(Message::Lighting)
            .map(LightingMessage::State)
            .into()
    }

    fn colour<'a>(
        &self,
        label: &'a str,
        colour: &[u8; 3],
        picker: Picker,
        on_change: impl Fn(RGBA) -> Lighting + Clone + 'a,
    ) -> Element<'a, LightingMessage> {
        let on_change = move |value| LightingMessage::State(Message::Lighting(on_change(value)));
        let colour = Color::from_rgb8(colour[0], colour[1], colour[2]);

        row![
            self.colour_swatch(colour, picker, on_change),
            text(label).height(Length::Shrink),
        ]
        .spacing(5)
        .align_y(Alignment::Center)
        .into()
    }

    fn brightness(&self, state: &AudioState, width: Length) -> Element<'_, LightingMessage> {
        let value = state.lighting.brightness;
        let range = LightingBrightness::range();

        Self::labeled_slider("Ring Brightness", value, range, width, |value| {
            Lighting::Brightness(LightingBrightness(value))
        })
        .into()
    }

    fn speed(&self, state: &AudioState, width: Length) -> Element<'_, LightingMessage> {
        // Dis one is special :D
        let value = state.lighting.speed;
        let range = LightingSpeed::range();

        let title = text("Speed and Direction:").width(width);
        let title_spacer = Space::new().width(10.0);

        let on_change = |value| {
            LightingMessage::State(Message::Lighting(Lighting::Speed(LightingSpeed(value))))
        };

        let slider = themed_slider(range.clone(), value, on_change).trail_start(0);
        let slider_spacer = Space::new().width(10.0);

        let input = styled_drag_value(value, range)
            .width(Length::Fixed(40.0))
            .on_change(on_change);

        // Build the layout
        let layout = row![title, title_spacer, slider, slider_spacer, input]
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center);

        container(layout)
            .width(Length::Fill)
            .height(Length::Shrink)
            .into()
    }

    fn sensitivity(&self, state: &AudioState, width: Length) -> Element<'_, LightingMessage> {
        let value = state.lighting.sensitivity;
        let range = LightingMeterSensitivty::range();

        draw_lighting_range("Meter Sensitivity", value, range, "", width, |v| {
            Lighting::MeterSensitivity(LightingMeterSensitivty(v))
        })
        .map(Message::Lighting)
        .map(LightingMessage::State)
        .into()
    }

    fn meter_source(&self, state: &AudioState, width: Length) -> Element<'_, LightingMessage> {
        let options: &[MeterSource] = &[
            MeterSource(LightingMeterSource::Microphone),
            MeterSource(LightingMeterSource::Headphones),
        ];

        let content = row![
            text("Meter Source:").width(width),
            pick_list(
                options,
                Some(MeterSource(state.lighting.source)),
                |option| Lighting::MeterSource(option.0)
            )
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let content = Element::from(content);
        content
            .map(Message::Lighting)
            .map(LightingMessage::State)
            .into()
    }

    fn colour_swatch<'a>(
        &self,
        colour: Color,
        picker: Picker,
        on_change: impl Fn(RGBA) -> LightingMessage + Clone + 'a,
    ) -> Element<'a, LightingMessage> {
        let title = text(format!("R: {}, G: {}, B: {}", colour.r, colour.g, colour.b));

        let hsv = Hsv::from(colour);
        let to_rgba = |hsv: Hsv| RGBA::from(hsv.to_rgba8());

        let active = container("")
            .style(move |_s| container::Style {
                text_color: None,
                background: Some(Background::Color(colour)),
                border: Default::default(),
                shadow: Default::default(),
                snap: false,
            })
            .width(Length::Fill)
            .height(18);

        let on_change_horizontal = on_change.clone();

        let content = column![
            title,
            active,
            ColorPicker::new(hsv, move |v| on_change(to_rgba(v)))
                .width(Length::Fill)
                .height(Length::Fill),
            ColorPicker::new(hsv, move |v| on_change_horizontal(to_rgba(v)))
                .spectrum(Spectrum::HueHorizontal)
                .width(Length::Fill)
                .height(18),
        ]
        .spacing(4);

        let show_button = button("")
            .style(move |_, _| button::Style {
                background: Some(Background::Color(colour)),
                text_color: Color::TRANSPARENT,
                border: Border {
                    width: 1.0,
                    color: Color::from_rgb8(60, 60, 60),
                    radius: Radius::from(2.0),
                },
                shadow: Default::default(),
                snap: false,
            })
            .width(38)
            .height(18)
            .on_press(LightingMessage::TogglePicker(picker));

        Popover::new(
            show_button,
            container(content)
                .style(move |_s| container::Style {
                    text_color: None,
                    background: Some(Background::Color(Color::from_rgb8(27, 27, 27))),
                    border: Border {
                        width: 1.0,
                        color: Color::from_rgb8(60, 60, 60),
                        radius: Radius::from(4.0),
                    },
                    shadow: Default::default(),
                    snap: false,
                })
                .width(290)
                .height(350)
                .padding(8),
            self.active_picker == Some(picker),
        )
        .on_close(LightingMessage::ClosePicker)
        .into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeterSource(pub LightingMeterSource);
impl Display for MeterSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
