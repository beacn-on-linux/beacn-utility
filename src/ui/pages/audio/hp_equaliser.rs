use crate::devices::states::audio::{AudioState, EqualiserBand};
use crate::ui::pages::audio::hp_equaliser::HPEQMessage::{
    Balance, Equaliser, Stereo, SubWoofer, ToggleLinked,
};
use crate::ui::pages::page::{AudioPage, PageMessage};
use crate::ui::widgets::equaliser::eq_common::Bands;
use crate::ui::widgets::equaliser::eq_drawer::{EQDrawView, EQMouseEvent};
use crate::ui::widgets::helpers::slider::{slider_theme, themed_slider};
use crate::ui::widgets::helpers::svg::{svg_button_style, svg_button_unstyled};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::equaliser::EQBand;
use beacn_lib::audio::messages::headphones::{HPLevel, HPMicMonitorLevel, Headphones};
use beacn_lib::audio::messages::subwoofer::{Subwoofer, SubwooferAmount};
use beacn_lib::manager::DeviceType;
use beacn_lib::types::HasRange;
use enum_map::{Enum, EnumMap};
use iced::border::Radius;
use iced::font::Weight;
use iced::mouse::ScrollDelta;
use iced::widget::{Canvas, Space, checkbox, column, container, row, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Point, Task};
use std::ops::RangeInclusive;
use std::time::Instant;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Enum, EnumIter)]
enum Channel {
    Left,
    Right,
}
impl Channel {
    fn other(self) -> Self {
        match self {
            Channel::Left => Channel::Right,
            Channel::Right => Channel::Left,
        }
    }
}

#[derive(Debug, Clone)]
pub enum HPEQMessage {
    Equaliser(Channel, EQMouseEvent),

    Balance(i32),
    SubWoofer(u8),
    Stereo(bool),
    ToggleLinked,

    // Direct State Change
    State(Message),
}

pub struct HPEqualiser {
    // Just one equaliser, although we need two..
    view: EnumMap<Channel, EQDrawView>,

    // Temporary data so we can test interactions. The state will eventually feed this
    temp: EnumMap<Channel, Bands>,

    // These are internal to this page
    active_channel: Channel,
    active_band: Option<EqualiserBand>,

    drag_active: bool,
    drag_start: Option<Instant>,

    // Needs to be fed from the state, currently unavailable.
    balance: i32,
    is_stereo: bool,
    is_linked: bool,
}

impl HPEqualiser {
    pub fn new() -> Self {
        Self {
            view: Default::default(),
            temp: Default::default(),

            active_channel: Channel::Left,
            active_band: None,

            drag_active: false,
            drag_start: None,

            balance: 0,
            is_stereo: true,
            is_linked: false,
        }
    }

    fn load_temp_data(&mut self, state: &AudioState) {
        for channel in Channel::iter() {
            self.temp[channel] = state.equaliser.bands[state.equaliser.mode];
            self.view[channel].set_bands(self.temp[channel].clone());
        }
    }

    fn update(&mut self, state: &mut AudioState, msg: HPEQMessage) -> Task<HPEQMessage> {
        match msg {
            Equaliser(channel, event) => {
                self.handle_eq_event(state, channel, event);
            }

            Balance(amount) => {
                self.balance = amount;
            }

            SubWoofer(amount) => {
                // Subwoofer is fun, get various messages based on amount
                let messages = Subwoofer::get_amount_messages(amount);
                for message in messages {
                    let _ = state.handle_message(message);
                }
            }

            Stereo(enabled) => {
                self.is_stereo = enabled;
            }

            ToggleLinked => {
                self.is_linked = !self.is_linked;
            }

            HPEQMessage::State(msg) => {
                let _ = state.handle_message(msg);
            }
        }

        Task::none()
    }

    fn handle_eq_event(&mut self, state: &mut AudioState, channel: Channel, event: EQMouseEvent) {
        match event {
            EQMouseEvent::Pressed(e) => self.handle_eq_press(state, channel, e),
            EQMouseEvent::Moved(e) => self.handle_eq_moved(state, channel, e),
            EQMouseEvent::Released => self.handle_eq_released(state, channel),
            EQMouseEvent::Scrolled { position, delta } => self.handle_eq_scrolled(state, channel, position, delta),
        }
    }

    fn handle_eq_press(&mut self, state: &mut AudioState, channel: Channel, point: Point) {}
    fn handle_eq_moved(&mut self, state: &mut AudioState, channel: Channel, point: Point) {}
    fn handle_eq_released(&mut self, state: &AudioState, channel: Channel) {}
    fn handle_eq_scrolled(&mut self, state: &AudioState, c: Channel, p: Point, d: ScrollDelta) {
        let (channel, point, delta) = (&c, &p, &d);
    }

    fn view(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        column![
            self.add_eq_canvas(Channel::Left),
            container(self.add_controls(state)).height(Length::Fixed(80.0)),
            self.add_eq_canvas(Channel::Right),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(6.0)
        .padding(10.0)
        .into()
    }

    // Canvas
    fn add_eq_canvas(&self, active: Channel) -> Element<'_, HPEQMessage> {
        // Lets grab the base canvas from the view..
        let canvas: Element<'_, EQMouseEvent> = Element::from(
            Canvas::new(&self.view[active])
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into();

        let canvas = match active {
            Channel::Left => canvas.map(|m| Equaliser(Channel::Left, m)),
            Channel::Right => canvas.map(|m| Equaliser(Channel::Right, m)),
        };

        let label = match active {
            Channel::Left => "LEFT",
            Channel::Right => "RIGHT",
        };

        // Should depend on whether self.active = active
        let label_colour = match active {
            Channel::Left => Color::from_rgba8(255, 255, 255, 0.5),
            Channel::Right => Color::from_rgba8(0, 255, 0, 1.0),
        };

        let label_font = Font {
            weight: Weight::Bold,
            ..Default::default()
        };
        let container_padding = Padding {
            top: 0.0,
            bottom: 10.0,
            left: 0.0,
            right: 5.0,
        };

        let overlay = container(text(label).size(20).color(label_colour).font(label_font))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(container_padding)
            .align_x(Alignment::End)
            .align_y(Alignment::End);

        // We should just need to create the EQ canvas, then stack![] them.
        stack![canvas, overlay].into()
    }

    // Controls
    fn add_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        row![
            Space::new().width(16),
            Self::panel(self.volume_controls(state)),
            Self::panel(self.balance_sub_controls(state)),
            Self::panel(self.mono_stereo_controls(state)),
            Self::panel(self.equaliser_controls(state)),
            Space::new().width(Length::Fill),
            self.link_control(state),
        ]
        .align_y(Alignment::Center)
        .spacing(6)
        .into()
    }

    fn volume_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        let device_type = state.device_definition.device_type;
        let value = state.headphones.mic_monitor;
        let range = HPMicMonitorLevel::range();
        let monitor = themed_slider(range, value, move |v| {
            let command = match device_type {
                DeviceType::BeacnMic => Headphones::MicMonitor(HPMicMonitorLevel(v)),
                DeviceType::BeacnStudio => Headphones::StudioMicMonitor(HPMicMonitorLevel(v)),
                _ => unreachable!(),
            };

            let msg = Message::Headphones(command);
            HPEQMessage::State(msg)
        });

        let value = state.headphones.level;
        let range = HPLevel::range();
        let level = themed_slider(range, value, |v| {
            let msg = Message::Headphones(Headphones::HeadphoneLevel(HPLevel(v)));
            HPEQMessage::State(msg)
        });

        column![
            text("Headphone Level").size(11.0),
            level,
            Space::new().height(7),
            text("Mic Monitor").size(11.0),
            monitor,
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(120.0))
        .into()
    }

    fn balance_sub_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        let balance_at_zero = self.balance == 0;
        let balance_slider = themed_slider(-100..=100, self.balance, Balance)
            .style(move |theme, status| {
                let mut style = slider_theme(theme, status);
                if balance_at_zero {
                    style.handle.border_color = Color::from_rgb(0.0, 1.0, 0.0);
                }
                style
            })
            .trail_start(0);

        let value = state.subwoofer.amount;
        let range = SubwooferAmount::range();
        let range: RangeInclusive<u8> = (*range.start() as u8)..=(*range.end() as u8);
        let woofer = themed_slider(range, value, SubWoofer);
        column![
            text("Balance").size(11.0),
            balance_slider,
            Space::new().height(7),
            text("Subwoofer").size(11.0),
            woofer,
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(120.0))
        .into()
    }

    fn mono_stereo_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        // TODO: Need a Stereo / Mono Icon!

        column![
            text("Stereo"),
            Space::new().height(14),
            checkbox(self.is_stereo).on_toggle(Stereo),
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(80.0))
        .into()
    }

    fn equaliser_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        container(text("Equaliser")).into()
    }

    fn link_control(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        let name = if self.is_linked { "unlink" } else { "link" };
        svg_button_unstyled(name)
            .on_press(ToggleLinked)
            .width(Length::Fixed(30.0))
            .style(move |theme, status| {
                let style = svg_button_style(theme, status, self.is_linked);

                style
            })
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(60.0))
            .into()
    }

    // Small Helper stuff
    fn panel<'a>(content: impl Into<Element<'a, HPEQMessage>>) -> Element<'a, HPEQMessage> {
        container(content)
            .width(Length::Shrink)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgb8(35, 35, 35))),
                border: Border {
                    radius: Radius::from(5),
                    ..Default::default()
                },
                ..Default::default()
            })
            .padding(6)
            .into()
    }
}

impl AudioPage for HPEqualiser {
    fn icon(&self) -> &'static str {
        "headphones"
    }

    fn on_open(&mut self, state: &AudioState) {
        self.load_temp_data(state);
    }

    fn update(&mut self, state: &mut AudioState, msg: PageMessage) -> Task<PageMessage> {
        if let PageMessage::AudioHPEqualiser(msg) = msg {
            return self.update(state, msg).map(PageMessage::AudioHPEqualiser);
        }

        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, PageMessage> {
        self.view(state).map(PageMessage::AudioHPEqualiser)
    }
}
