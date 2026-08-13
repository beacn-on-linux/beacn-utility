use crate::ui::app::Message;
use crate::ui::pages::app::settings::SettingsMessage;
use crate::{HASH, VERSION};
use iced::widget::{Space, button, checkbox, container, rule, text};
use iced::{Alignment, Element, Length, Task};

const PIPEWEAVER_URL: &str = "https://github.com/pipeweaver/pipeweaver";

#[derive(Debug, Copy, Clone)]
pub enum PipeweaverMessage {
    GetPipeweaver,
}

pub struct PipeweaverPage {}
impl PipeweaverPage {
    pub(crate) fn new() -> Self {
        Self {}
    }

    pub(crate) fn update(&mut self, message: PipeweaverMessage) -> Task<PipeweaverMessage> {
        match message {
            PipeweaverMessage::GetPipeweaver => {
                let _ = open::that_detached(PIPEWEAVER_URL);
            }
        }

        Task::none()
    }

    pub(crate) fn view(&self) -> Element<'_, PipeweaverMessage> {
        let title = "Enhance your Beacn on Linux experience with Pipeweaver";

        let part_one = "Pipeweaver brings streaming-focused audio control to Linux, with mixing, routing, and separate personal and stream outputs.";
        let part_two = "If you have a Mix / Mix Create, the Beacn Utility will talk to Pipeweaver to bring volume and mix control to your devices, similar to how you've used them on Windows.";
        let part_three = "Pipeweaver isn’t running right now. If you’ve already installed it, just start it up. If not, hit the button below and give it an install!";

        let button_text = text("Get Pipeweaver")
            .size(18)
            .width(160)
            .height(32)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        let button = button(button_text)
            .on_press(PipeweaverMessage::GetPipeweaver)
            .padding(8);

        let button = container(button)
            .width(Length::Fill)
            .align_x(Alignment::Center);

        let content = iced::widget::column![
            text(title).size(24),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
            text(part_one),
            Space::new().height(10),
            text(part_two),
            Space::new().height(10),
            text(part_three),
            Space::new().height(10),
            rule::horizontal(1),
            Space::new().height(10),
            button
        ]
        .spacing(8)
        .padding(20);

        Element::from(content).into()
    }
}
