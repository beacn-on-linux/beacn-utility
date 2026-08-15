use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::ui::pages::audio::config_pages::compressor::CompressorPage;
use crate::ui::pages::audio::config_pages::expander::ExpanderPage;
use crate::ui::pages::audio::config_pages::headphones::HeadphonesPage;
use crate::ui::pages::audio::config_pages::mic_equaliser::{MicEqualiser, MicEqualiserEvent};
use crate::ui::pages::audio::config_pages::mic_setup::MicrophoneSetup;
use crate::ui::pages::audio::config_pages::suppressor::SuppressorPage;
use crate::ui::pages::audio::config_pages::{ChildMessage, ConfigPage};
use crate::ui::pages::page::{AudioPage, PageMessage};
use crate::ui::utility::pipewire::device::{PipeWireNodeType, find_pipewire_nodes_for_usb};
use crate::ui::utility::pipewire::spectrum::{SpectrumHandle, start_spectrum_analyser};
use crate::ui::widgets::helpers::composite::draw_range;
use crate::ui::widgets::helpers::tabs::render_tab;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::headphones::{HPMicOutputGain, Headphones};
use beacn_lib::types::HasRange;
use iced::widget::{button, column, container, row, rule, text};
use iced::{Alignment, Element, Length, Padding, Task};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub(crate) enum ConfigMessage {
    Equaliser(MicEqualiserEvent),
    Child(ChildMessage),
    SelectTab(usize),

    OutputGainChanged(f32),
}

pub struct Configuration {
    equaliser: MicEqualiser,
    spectrum_handler: Option<SpectrumHandle>,
    spectrum_data: Option<Arc<Mutex<Vec<f32>>>>,

    //
    selected_tab: usize,
    tab_pages: Vec<Box<dyn ConfigPage>>,
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            equaliser: MicEqualiser::new(),
            spectrum_handler: None,
            spectrum_data: None,

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

        if self.spectrum_handler.is_some() {
            return;
        }

        let location = state.location();
        let bus_addr = location.bus_id.parse::<u8>().unwrap_or(0);
        let dev_addr = location.device_address;
        let nodes = find_pipewire_nodes_for_usb(bus_addr, dev_addr);

        let mut use_node = None;
        if let Ok(nodes) = nodes {
            // We found something, we need to find the mic node
            for node in nodes {
                if node.node_type == PipeWireNodeType::Source && node.channels == 4 {
                    use_node.replace(node);
                }
            }
        }

        if let Some(node) = use_node {
            // Ok, we have a usable node, let's fire up a listener..
            let handler = start_spectrum_analyser(&node.name, 48000);

            // Get the internal Spectrum Data
            self.spectrum_data = Some(handler.data.clone());
            self.spectrum_handler = Some(handler);
        }
    }

    fn on_close(&mut self) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
        }

        // Remove anything that may be cached, we should redraw later.
        self.equaliser.clear();
    }

    fn on_tick(&mut self, _state: &mut AudioState) -> Task<PageMessage> {
        let Some(handler) = self.spectrum_handler.as_mut() else {
            return Task::none();
        };

        if handler.has_stopped() {
            self.spectrum_handler = None;
            self.spectrum_data = None;

            self.equaliser.clear_spectrum_data();
            return Task::none();
        }

        // This shouldn't happen if the handler is active, but just in case..
        let Some(data) = self.spectrum_data.as_mut() else {
            self.spectrum_handler = None;
            self.spectrum_data = None;

            self.equaliser.clear_spectrum_data();
            return Task::none();
        };

        if let Ok(guard) = data.lock() {
            self.equaliser.set_spectrum_data(guard.clone());
        }
        Task::none()
    }

    fn update(&mut self, state: &mut AudioState, message: PageMessage) -> Task<PageMessage> {
        match message {
            PageMessage::AudioConfig(msg) => match msg {
                ConfigMessage::Equaliser(event) => self
                    .equaliser
                    .update(state, event)
                    .map(ConfigMessage::Equaliser)
                    .map(PageMessage::AudioConfig),

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
                            .map(PageMessage::AudioConfig),

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
            .map(PageMessage::AudioConfig);

        let controls = self
            .equaliser
            .eq_controls(state)
            .map(ConfigMessage::Equaliser)
            .map(PageMessage::AudioConfig);

        let bottom = self.bottom_view(state).map(PageMessage::AudioConfig);
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

// This will be called when the device goes away.
impl Drop for Configuration {
    fn drop(&mut self) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
        }
    }
}
