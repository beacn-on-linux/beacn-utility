use crate::devices::manager::DefinitionState;
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::ui::pages::page::{AudioPage, PageMessage};
use beacn_lib::audio::LinkChannel;
use beacn_lib::manager::DeviceType;
use iced::widget::{button, column, pick_list, row, text};
use iced::{Alignment, Element, Length, Task};
use std::fmt::{Display, Formatter};
use strum::IntoEnumIterator;

#[derive(Debug, Clone)]
pub(crate) enum StudioLinkMessage {
    LinkChannelChanged(usize, LinkChannel),
    Refresh,
}

pub struct StudioLink;
impl StudioLink {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioPage for StudioLink {
    fn icon(&self) -> &'static str {
        "left_right"
    }

    fn should_show(&self, state: &AudioState) -> bool {
        // We're a Beacn Studio, we're not errored, and we're not driverless :D
        state.definition().device_type == DeviceType::BeacnStudio
            && !matches!(state.definition().state, DefinitionState::Error(_))
            && state.headphones.studio_driverless == Some(false)
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        let PageMessage::AudioStudioLinkPage(message) = message else {
            return Task::none();
        };

        match message {
            StudioLinkMessage::LinkChannelChanged(index, channel) => {
                if let Some(apps) = &mut state.linked {
                    if let Some(app) = apps.get_mut(index) {
                        app.channel = channel;

                        let app = app.clone();
                        let _ = state.set_link(app);
                    }
                }

                Task::none()
            }

            StudioLinkMessage::Refresh => {
                let _ = state.get_linked();
                Task::none()
            }
        }
    }

    fn view(&self, state: &AudioState) -> Element<'_, PageMessage> {
        let mut content = column![text(
            "This page requires the PC2 USB port to be plugged into a \
                 Windows PC with the Beacn Link app running."
        ),]
        .spacing(10);

        match &state.linked {
            Some(apps) if apps.is_empty() => {
                content = content.push(text("No Apps playing audio detected"));
            }

            Some(apps) => {
                for (index, app) in apps.iter().enumerate() {
                    let label = text(format!("{}: ", app.name.clone())).width(80);

                    let options: &[LinkChannelDisplay] = &[
                        LinkChannelDisplay(LinkChannel::Link1),
                        LinkChannelDisplay(LinkChannel::Link2),
                        LinkChannelDisplay(LinkChannel::Link3),
                        LinkChannelDisplay(LinkChannel::Link4),
                    ];

                    let picker = pick_list(
                        options,
                        Some(LinkChannelDisplay(app.channel)),
                        move |channel| StudioLinkMessage::LinkChannelChanged(index, channel.0),
                    )
                    .placeholder("System");

                    let row = row![label, picker].spacing(10).align_y(Alignment::Center);
                    content = content.push(row);
                }
            }

            None => {
                content = content.push(text("Unable to communicate with the Beacn Link App"));
            }
        }

        content = content
            .push(button("Refresh").on_press(StudioLinkMessage::Refresh))
            .padding(10);

        let element = Element::from(content);
        element.map(PageMessage::AudioStudioLinkPage).into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LinkChannelDisplay(pub LinkChannel);
impl Display for LinkChannelDisplay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self.0 {
            LinkChannel::System => "System",
            LinkChannel::Link1 => "Link 1",
            LinkChannel::Link2 => "Link 2",
            LinkChannel::Link3 => "Link 3",
            LinkChannel::Link4 => "Link 4",
        };
        write!(f, "{}", value)
    }
}
