use crate::device_manager::DeviceDefinition;
use crate::ui::audio_pages::AudioPage;
use crate::ui::audio_pages::config_pages::ConfigPage;
use crate::ui::audio_pages::config_pages::compressor::CompressorPage;
use crate::ui::audio_pages::config_pages::expander::ExpanderPage;
use crate::ui::audio_pages::config_pages::headphones::HeadphonesPage;
use crate::ui::audio_pages::config_pages::mic_equaliser::MicEqualiser;
use crate::ui::audio_pages::config_pages::mic_setup::MicSetupPage;
use crate::ui::audio_pages::config_pages::suppressor::NoiseSuppressionPage;
use crate::ui::audio_pages::pipewire::device_locater::{
    PipeWireNodeType, find_pipewire_nodes_for_usb,
};
use crate::ui::audio_pages::pipewire::spectrum::{SpectrumHandle, start_spectrum_analyser};
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::widgets::draw_range;
use crate::window_handle::{UserEvent, send_user_event};
use beacn_lib::audio::messages::headphones::HPMicOutputGain;
use beacn_lib::types::HasRange;
use egui::{Context, Ui, vec2};

pub struct Configuration {
    equaliser: Box<MicEqualiser>,
    spectrum_handler: Option<SpectrumHandle>,

    selected_tab: usize,
    tab_pages: Vec<Box<dyn ConfigPage>>,
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            equaliser: Box::new(MicEqualiser::new()),
            spectrum_handler: None,

            selected_tab: 0,
            tab_pages: vec![
                Box::new(MicSetupPage),
                Box::new(NoiseSuppressionPage),
                Box::new(ExpanderPage),
                Box::new(CompressorPage),
                Box::new(HeadphonesPage),
            ],
        }
    }
}

impl AudioPage for Configuration {
    fn icon(&self) -> &'static str {
        "mic"
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) {
        let eq_size = vec2(ui.available_width(), ui.available_height() - 240.);
        ui.allocate_ui_with_layout(eq_size, *ui.layout(), |ui| {
            ui.set_min_size(eq_size);
            ui.set_max_size(eq_size);
            self.equaliser.ui(ui, state);
        });

        ui.separator();

        ui.vertical(|ui| {
            // Bottom half
            let total_available = ui.available_size();
            let fixed_panel_width = 100.0;
            let tab_area_width = total_available.x - fixed_panel_width;

            ui.horizontal(|ui| {
                // Left: Tab bar + active tab
                ui.allocate_ui(egui::vec2(tab_area_width, total_available.y), |ui| {
                    ui.vertical(|ui| {
                        // Tab bar
                        ui.horizontal(|ui| {
                            for (i, page) in self.tab_pages.iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_tab == i, page.title())
                                    .clicked()
                                {
                                    self.selected_tab = i;
                                }
                            }
                        });

                        ui.separator();

                        // Active tab content
                        if let Some(page) = self.tab_pages.get_mut(self.selected_tab) {
                            page.ui(ui, state);
                        }
                    });
                });

                ui.separator();

                // Right: Fixed panel
                ui.allocate_ui(vec2(fixed_panel_width, total_available.y), |ui| {
                    let gain = &mut state.headphones;
                    if draw_range(
                        ui,
                        &mut gain.output_gain,
                        HPMicOutputGain::range(),
                        "Output Gain",
                        "dB",
                    ) {}
                });
            });
        });
    }

    fn on_page_open(&mut self, ctx: &Context, device: &DeviceDefinition) {
        if self.spectrum_handler.is_some() {
            // We already have a handler, do nothing.
            return;
        }

        // Attempt to locate this devices audio channels
        let location = device.location;
        let nodes = find_pipewire_nodes_for_usb(location.bus_number, location.address);

        let mut use_node = None;
        if let Ok(nodes) = nodes {
            // We found something, we need to find the mic node
            for node in nodes {
                if node.node_type == PipeWireNodeType::Source {
                    if node.channels == 4 {
                        use_node.replace(node);
                    }
                }
            }
        }

        if let Some(node) = use_node {
            // Ok, we have a usable node, let's fire up a listener..
            let handler = start_spectrum_analyser(&*node.name, 48000);

            // Get the internal Spectrum Data
            let data = handler.data.clone();
            self.equaliser.set_spectrum(data);

            // Tell the UI to set the minimum refresh rate to 30Hz
            send_user_event(ctx, UserEvent::SetMinimumRefreshRate(true));

            self.spectrum_handler = Some(handler);
        }
    }

    fn on_page_close(&mut self, ctx: &Context, _: &DeviceDefinition) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
            send_user_event(ctx, UserEvent::SetMinimumRefreshRate(false));
        }
    }

    fn on_close(&mut self) {
        if let Some(handler) = self.spectrum_handler.take() {
            handler.stop();
        }
        self.equaliser.clear();
    }
}
