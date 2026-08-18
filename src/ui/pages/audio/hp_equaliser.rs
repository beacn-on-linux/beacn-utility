use crate::devices::states::audio::AudioState;
use crate::ui::pages::page::{AudioPage, PageMessage};
use enum_map::Enum;
use iced::border::Radius;
use iced::font::Weight;
use iced::widget::{column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Task};
use strum_macros::EnumIter;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Enum, EnumIter)]
enum ActiveEQ {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum HPEQMessage {
    DUMMY,
}

pub struct HPEqualiser {}

impl HPEqualiser {
    pub fn new() -> Self {
        Self {}
    }

    fn update(&mut self, state: &mut AudioState, msg: HPEQMessage) -> Task<HPEQMessage> {
        Task::none()
    }

    fn view(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        column![
            self.add_eq_canvas(ActiveEQ::Left),
            container(self.add_controls(state)).height(Length::Fixed(80.0)),
            self.add_eq_canvas(ActiveEQ::Right),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(6.0)
        .padding(10.0)
        .into()
    }

    // Canvas
    fn add_eq_canvas(&self, active: ActiveEQ) -> Element<'_, HPEQMessage> {
        let label = match active {
            ActiveEQ::Left => "LEFT",
            ActiveEQ::Right => "RIGHT",
        };

        // Should depend on whether self.active = active
        let label_colour = match active {
            ActiveEQ::Left => Color::from_rgba8(255, 255, 255, 0.5),
            ActiveEQ::Right => Color::from_rgba8(0, 255, 0, 1.0),
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
            .align_y(Alignment::End)
            .into();

        // We should just need to create the EQ canvas, then stack![] them.

        overlay
    }

    // Controls
    fn add_controls(&self, state: &AudioState) -> Element<'_, HPEQMessage> {
        container(text("Controls!")).into()
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
