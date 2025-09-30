use crate::device_manager::{DeviceArriveMessage, DeviceDefinition, DeviceMessage};
use crate::ui::app_settings::settings_ui;
use crate::ui::audio_pages::AudioPage;
use crate::ui::controller_pages::ControllerPage;
use crate::ui::states::LoadState;
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::states::controller_state::BeacnControllerState;
use crate::ui::widgets::round_nav_button;
use crate::ui::{audio_pages, controller_pages};
use crate::window_handle::App;
use beacn_lib::crossbeam::channel;
use beacn_lib::manager::DeviceType;
use egui::ahash::HashMap;
use egui::{Context, Ui};
use log::debug;
use std::any::Any;

pub struct BeacnMicApp {
    device_list: Vec<DeviceDefinition>,
    active_device: Option<DeviceDefinition>,

    audio_device_list: HashMap<DeviceDefinition, BeacnAudioState>,
    control_device_list: HashMap<DeviceDefinition, BeacnControllerState>,

    audio_pages: Vec<Box<dyn AudioPage>>,
    control_pages: Vec<Box<dyn ControllerPage>>,

    device_recv: channel::Receiver<DeviceMessage>,
    active_page: usize,

    settings_active: bool,
}

impl BeacnMicApp {
    pub fn new(device_recv: channel::Receiver<DeviceMessage>) -> Self {
        Self {
            device_list: vec![],
            active_device: None,

            audio_device_list: HashMap::default(),
            control_device_list: HashMap::default(),

            audio_pages: vec![
                Box::new(audio_pages::config::Configuration::new()),
                Box::new(audio_pages::lighting::LightingPage::new()),
                Box::new(audio_pages::link::Linked::new()),
                Box::new(audio_pages::about::About::new()),
                Box::new(audio_pages::error::ErrorPage::new()),
            ],

            control_pages: vec![
                Box::new(controller_pages::about::About::new()),
                Box::new(controller_pages::error::ErrorPage::new()),
            ],

            device_recv,
            active_page: 0,
            settings_active: false,
        }
    }
}

impl BeacnMicApp {
    pub fn with_context(&self, ctx: &Context) {
        egui_extras::install_image_loaders(ctx);
    }
}

impl App for BeacnMicApp {
    fn update(&mut self, ctx: &Context) {
        // Grab any device information that's been sent since the last update
        let messages: Vec<DeviceMessage> = self.device_recv.try_iter().collect();
        for message in messages {
            self.handle_device_message(message);
        }

        // Is our Device List empty?
        if self.device_list.is_empty() {
            egui::CentralPanel::default().show(ctx, |ui: &mut Ui| {
                ui.add_sized(ui.available_size(), |ui: &mut Ui| {
                    ui.label("No Devices Detected")
                });
            });
            return;
        }

        egui::SidePanel::left("left_panel")
            .resizable(false)
            .default_width(80.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Grab the device list, and reorder it based on type
                    let mut devices = self.device_list.clone();
                    devices.sort_by_key(|d| d.device_type);
                    for device in devices {
                        self.draw_device_buttons(ui, device);
                    }
                    ui.add_space(ui.available_height() - 55.0);
                    ui.separator();
                    if round_nav_button(ui, "gear", self.settings_active).clicked() {
                        self.settings_active = true;
                    }
                });
            });

        // Render the main page
        self.render_content(ctx);
    }

    fn should_close(&mut self) -> bool {
        // TODO: This should prompt the user, and / or check the settings
        true
    }

    fn on_close(&mut self) {
        for audio_page in &mut self.audio_pages {
            audio_page.on_close();
        }

        for controller_pages in &mut self.control_pages {
            controller_pages.on_close();
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl BeacnMicApp {
    pub fn handle_device_message(&mut self, message: DeviceMessage) {
        match message {
            DeviceMessage::DeviceArrived(device) => match device {
                DeviceArriveMessage::Audio(definition, sender) => {
                    // Load the Device State
                    let state = BeacnAudioState::load_settings(definition.clone(), sender);

                    // Store the Device, and the device state
                    self.device_list.push(definition.clone());
                    self.audio_device_list.insert(definition.clone(), state);

                    if self.active_device.is_none() {
                        self.active_device = Some(definition);
                    }
                }
                DeviceArriveMessage::Control(definition, sender) => {
                    let state = BeacnControllerState::load_settings(definition.clone(), sender);
                    self.device_list.push(definition.clone());
                    self.control_device_list.insert(definition.clone(), state);

                    if self.active_device.is_none() {
                        self.active_device = Some(definition);
                    }
                }
            },
            DeviceMessage::DeviceRemoved(location) => {
                // Find the index of this device in the device list
                let position = self.device_list.iter().position(|d| d.location == location);
                if let Some(position) = position {
                    // This is a little complicated, first get the device definition, and
                    // remove it from the relevant device list.
                    let definition = &self.device_list[position].clone();
                    match definition.device_type {
                        DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                            // Remove this device from the audio device list
                            self.audio_device_list.remove(definition);
                        }
                        DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                            self.control_device_list.remove(definition);
                        }
                    }

                    // Now remove it from the main device list
                    self.device_list.retain(|d| d != definition);

                    // Make sure we're not referencing this device as active
                    if let Some(active_device) = &self.active_device
                        && active_device == definition
                    {
                        if self.device_list.is_empty() {
                            self.active_device = None;
                        } else {
                            // Reset the State, set the active device as the first device
                            let first = self.device_list.first().unwrap();
                            self.active_device = Some(first.clone());
                            self.active_page = 0;
                        }
                    }
                }
            }
        }
    }

    fn draw_device_buttons(&mut self, ui: &mut Ui, device: DeviceDefinition) {
        if self.device_list.is_empty() || self.active_device.is_none() {
            debug!("NOT DRAWING");
            return;
        }

        let active_device = &self.active_device.clone().unwrap();
        match device.device_type {
            // These are probably going to eventually need to be separated, when
            // Studio Link support is added, a new page will be needed
            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                let device_state = self.audio_device_list.get(&device).unwrap();
                ui.add_space(5.0);

                match device.device_type {
                    DeviceType::BeacnMic => ui.label("Mic"),
                    DeviceType::BeacnStudio => ui.label("Studio"),
                    _ => ui.label("ERROR"),
                };

                let audio_pages = self.audio_pages.iter().enumerate();
                for (index, page) in audio_pages {
                    let selected = *active_device == device
                        && self.active_page == index
                        && !self.settings_active;
                    let error = matches!(
                        device_state.device_state.state,
                        LoadState::Error | LoadState::PermissionDenied | LoadState::ResourceBusy
                    );

                    if page.show_on_error() == error
                        && (!page.is_link_page() || page.is_studio_with_link(device_state))
                        && round_nav_button(ui, page.icon(), selected).clicked()
                    {
                        self.settings_active = false;
                        self.active_device = Some(device.clone());
                        self.active_page = index;
                    }
                }
                ui.add_space(5.0);
                ui.separator();
            }
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                // This is identical to the above, except with a BeacnControllerState and ControllerPages
                // There's probably a way we can simplify this :p
                let device_state = self.control_device_list.get(&device).unwrap();
                ui.add_space(5.0);

                match device.device_type {
                    DeviceType::BeacnMix => ui.label("Mix"),
                    DeviceType::BeacnMixCreate => ui.label("Mix Create"),
                    _ => ui.label("ERROR"),
                };

                let control_pages = self.control_pages.iter().enumerate();
                for (index, page) in control_pages {
                    let selected = *active_device == device
                        && self.active_page == index
                        && !self.settings_active;

                    let error = matches!(
                        device_state.device_state.state,
                        LoadState::Error | LoadState::PermissionDenied | LoadState::ResourceBusy
                    );
                    if page.show_on_error() == error
                        && round_nav_button(ui, page.icon(), selected).clicked()
                    {
                        self.settings_active = false;
                        self.active_device = Some(device.clone());
                        self.active_page = index;
                    }
                }
                ui.add_space(5.0);
                ui.separator();
            }
        }
    }
    fn render_content(&mut self, ctx: &Context) {
        if self.active_device.is_none() && !self.settings_active {
            return;
        }

        if self.settings_active {
            egui::CentralPanel::default().show(ctx, |ui| {
                settings_ui(ui, ctx);
            });
            return;
        }

        let definition = &self.active_device.clone().unwrap();
        match definition.device_type {
            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                // Get the Settings from the definition
                let settings = self.audio_device_list.get_mut(definition);
                if settings.is_none() {
                    return;
                }
                let settings = settings.unwrap();

                // Are we in an error state, if so, show the error
                if settings.device_state.state == LoadState::Error {
                    let position = self.audio_pages.iter().position(|p| p.show_on_error());
                    if let Some(page) = position {
                        self.active_page = page;
                    }
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    self.audio_pages[self.active_page].ui(ui, settings);
                });
            }
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                let settings = self.control_device_list.get_mut(definition);
                if settings.is_none() {
                    return;
                }

                let settings = settings.unwrap();
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.control_pages[self.active_page].ui(ui, settings);
                });
            }
        }
    }
}
