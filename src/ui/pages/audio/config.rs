use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::compressor::CompressorPage;
use crate::ui::pages::audio::config_pages::expander::ExpanderPage;
use crate::ui::pages::audio::config_pages::headphones::HeadphonesPage;
use crate::ui::pages::audio::config_pages::mic_equaliser::{MicEqualiser, MicEqualiserEvent};
use crate::ui::pages::audio::config_pages::mic_setup::MicrophoneSetup;
use crate::ui::pages::audio::config_pages::suppressor::SuppressorPage;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::pages::page::{AudioPage, PageMessage};
use crate::ui::widgets::helpers::composite::draw_range;
use crate::ui::widgets::helpers::tabs::render_tab;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphones::{HPMicOutputGain, Headphones};
use beacn_lib::types::HasRange;
use iced::widget::{button, column, container, row, rule, text};
use iced::{Alignment, Element, Length, Padding, Task};

#[derive(Debug, Clone)]
pub(crate) enum ConfigMessage {
    Equaliser(MicEqualiserEvent),
    Child(ChildMessage),
    SelectTab(usize),

    OutputGainChanged(f32),
}

pub struct Configuration {
    equaliser: MicEqualiser,

    //
    selected_tab: usize,
    tab_pages: Vec<Box<dyn ConfigPage>>,
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            equaliser: MicEqualiser::new(),

            selected_tab: 0,
            tab_pages: vec![
                Box::new(MicrophoneSetup),
                Box::new(SuppressorPage),
                Box::new(ExpanderPage),
                Box::new(CompressorPage),
                Box::new(HeadphonesPage),
            ],
        }
    }

    fn bottom_view(&self, state: &AudioState) -> Element<'_, ConfigMessage> {
        let content = match self.tab_pages.get(self.selected_tab) {
            Some(page) => page.view(state).map(ConfigMessage::Child),
            None => container(text("No configuration page")).into(),
        };

        let tabs = self
            .tab_pages
            .iter()
            .enumerate()
            .fold(row![], |tabs, (index, page)| {
                let is_active = index == self.selected_tab;

                let mut btn = button(text(page.title()))
                    .style(move |t, s| render_tab(t, s, is_active))
                    .padding(6.0);

                if !is_active {
                    btn = btn.on_press(ConfigMessage::SelectTab(index));
                }
                let separator = container(rule::vertical(1)).center_y(Length::Fixed(28.0));
                tabs.push(btn).push(separator)
            });

        let tab_layout = column![column![tabs], rule::horizontal(1), content]
            .width(Length::Fill)
            .height(Length::Fill);

        let value = state.headphones.output_gain;
        let range = HPMicOutputGain::range();
        let gain = draw_range(
            "Output Gain",
            value,
            range,
            "dB",
            ConfigMessage::OutputGainChanged,
        );

        row![
            tab_layout,
            rule::vertical(1),
            container(gain)
                .width(Length::Fixed(100.0))
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .padding(8)
        ]
        .into()
    }
}

impl AudioPage for Configuration {
    fn icon(&self) -> &'static str {
        "mic"
    }

    fn on_open(&mut self, state: &AudioState) {
        self.equaliser.load_device(state);
    }

    fn on_close(&mut self) {
        // Remove anything that may be cached, we should redraw later.
        self.equaliser.clear();
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        match message {
            PageMessage::AudioConfigPage(msg) => match msg {
                ConfigMessage::Equaliser(event) => self
                    .equaliser
                    .update(state, event)
                    .map(ConfigMessage::Equaliser)
                    .map(PageMessage::AudioConfigPage),

                ConfigMessage::SelectTab(tab_index) => {
                    self.selected_tab = tab_index;
                    Task::none()
                }

                ConfigMessage::OutputGainChanged(gain) => {
                    let msg = Headphones::MicOutputGain(HPMicOutputGain(gain));
                    let msg = Message::Headphones(msg);
                    let _ = state.handle_message(msg);

                    Task::none()
                }

                // These are messages intended to go to device
                ConfigMessage::Child(ChildMessage::State(msg)) => {
                    let _ = state.handle_message(msg);
                    Task::none()
                }

                ConfigMessage::Child(child_msg) => {
                    match self.tab_pages.get_mut(self.selected_tab) {
                        Some(page) => page
                            .update(state, child_msg)
                            .map(ConfigMessage::Child)
                            .map(PageMessage::AudioConfigPage),

                        None => Task::none(),
                    }
                }
            },

            _ => Task::none(),
        }
    }

    fn view(&self, state: &AudioState) -> Element<'_, PageMessage> {
        let equaliser = self
            .equaliser
            .view(state)
            .map(ConfigMessage::Equaliser)
            .map(PageMessage::AudioConfigPage);

        let controls = self
            .equaliser
            .eq_controls(state)
            .map(ConfigMessage::Equaliser)
            .map(PageMessage::AudioConfigPage);

        let bottom = self.bottom_view(state).map(PageMessage::AudioConfigPage);
        column![
            // Remaining space
            container(equaliser)
                .width(Length::Fill)
                .height(Length::Fill),
            container(controls)
                .width(Length::Fill)
                .height(Length::Fixed(33.0))
                .padding(Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: 5.0,
                    left: 0.0,
                }),
            rule::horizontal(5),
            // Fixed bottom section
            container(bottom)
                .width(Length::Fill)
                .height(Length::Fixed(240.0)),
        ]
        .into()
    }
}
