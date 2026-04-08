use crate::device_manager::{DeviceArriveMessage, DeviceDefinition, DeviceMessage};
use crate::ui::audio_pages::AudioPage;
use crate::ui::controller_pages::ControllerPage;
<<<<<<< feature/pipeweaver-preflight-setup
use crate::ui::mixer_page::{MixerPageState, mixer_ui};
=======
use crate::ui::mixer_page::{mixer_ui, MixerPageState};
>>>>>>> main
use crate::ui::pages::settings_ui;
use crate::ui::states::LoadState;
use crate::ui::states::audio_state::BeacnAudioState;
use crate::ui::states::controller_state::BeacnControllerState;
use crate::ui::states::pipeweaver_state::SharedPipeweaverState;
use crate::ui::widgets::{round_nav_button, round_pipeweaver_button};
use crate::ui::{audio_pages, controller_pages};
use crate::window_handle::App;
use beacn_lib::crossbeam::channel;
use beacn_lib::manager::DeviceType;
use egui::ahash::HashMap;
use egui::{Context, Ui};
use log::{debug, warn};

pub struct BeacnMicApp {
    device_list: Vec<DeviceDefinition>,
    active_device: Option<DeviceDefinition>,

    audio_device_list: HashMap<DeviceDefinition, BeacnAudioState>,
    control_device_list: HashMap<DeviceDefinition, BeacnControllerState>,

    audio_pages: Vec<Box<dyn AudioPage>>,
    control_pages: Vec<Box<dyn ControllerPage>>,

    device_recv: channel::Receiver<DeviceMessage>,
    active_page: usize,

    mixer_active: bool,
    settings_active: bool,

    pipeweaver_state: Option<SharedPipeweaverState>,
    mixer_page_state: MixerPageState,
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
            mixer_active: false,
            settings_active: false,
<<<<<<< feature/pipeweaver-preflight-setup
=======

>>>>>>> main
            pipeweaver_state: None,
            mixer_page_state: MixerPageState::default(),
        }
    }
}

impl App for BeacnMicApp {
    fn with_context(&mut self, ctx: &Context) {
        egui_extras::install_image_loaders(ctx);
    }

<<<<<<< feature/pipeweaver-preflight-setup
    fn update(&mut self, ui: &mut Ui) {
=======
    fn update(&mut self, ctx: &Context) {
>>>>>>> main
        let messages: Vec<DeviceMessage> = self.device_recv.try_iter().collect();
        for message in messages {
            self.handle_device_message(message);
        }

        if self.device_list.is_empty() {
            egui::CentralPanel::default().show_inside(ui, |ui: &mut Ui| {
                ui.add_sized(ui.available_size(), |ui: &mut Ui| {
                    ui.label("No Devices Detected")
                });
            });
            return;
        }

        egui::Panel::left("left_panel")
            .resizable(false)
            .default_size(80.0)
            .show_inside(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(5.0);
                    if round_pipeweaver_button(ui, "pipeweaver", self.mixer_active).clicked() {
                        self.settings_active = false;
                        self.mixer_active = true;
                    }
                    ui.add_space(5.0);
                    ui.separator();

                    let mut devices = self.device_list.clone();
                    devices.sort_by_key(|d| d.device_type);
                    for device in devices {
                        self.draw_device_buttons(ui, device);
                    }
                    ui.add_space(ui.available_height() - 55.0);
                    ui.separator();
                    if round_nav_button(ui, "gear", self.settings_active).clicked() {
                        self.mixer_active = false;
                        self.settings_active = true;
                    }
                });
            });

<<<<<<< feature/pipeweaver-preflight-setup
        self.render_content(ui);
=======
        self.render_content(ctx);
>>>>>>> main
    }

    fn should_close(&mut self) -> bool {
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

    fn handle_device_message(&mut self, message: DeviceMessage) {
        match message {
            DeviceMessage::DeviceArrived(device) => match device {
                DeviceArriveMessage::Audio(definition, sender) => {
                    let state = BeacnAudioState::load_settings(definition.clone(), sender);
                    self.device_list.push(definition.clone());
                    self.audio_device_list.insert(definition.clone(), state);
                    if self.active_device.is_none() {
                        self.active_device = Some(definition);
                    }
                }
                DeviceArriveMessage::Control(definition, sender, pw_state) => {
                    let state = BeacnControllerState::load_settings(definition.clone(), sender);
                    self.device_list.push(definition.clone());
                    self.control_device_list.insert(definition.clone(), state);
<<<<<<< feature/pipeweaver-preflight-setup
                    if let Some(pw) = pw_state {
                        self.pipeweaver_state = Some(pw);
                    }
=======

                    if let Some(pw) = pw_state {
                        self.pipeweaver_state = Some(pw);
                    }

>>>>>>> main
                    if self.active_device.is_none() {
                        self.active_device = Some(definition);
                    }
                }
            },
            DeviceMessage::DeviceRemoved(location) => {
                let position = self.device_list.iter().position(|d| d.location == location);
                if let Some(position) = position {
                    let definition = &self.device_list[position].clone();
                    match definition.device_type {
                        DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                            self.audio_device_list.remove(definition);
                        }
                        DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                            self.control_device_list.remove(definition);
                        }
                    }

                    self.device_list.retain(|d| d != definition);

                    let has_mixer_device = self.device_list.iter().any(|d| {
                        matches!(d.device_type, DeviceType::BeacnMix | DeviceType::BeacnMixCreate)
                    });
                    if !has_mixer_device {
                        self.pipeweaver_state = None;
                    }

                    if let Some(active_device) = &self.active_device
                        && active_device == definition
                    {
                        if self.device_list.is_empty() {
                            self.active_device = None;
                        } else {
                            let first = self.device_list.first().unwrap();
                            self.active_device = Some(first.clone());
                            self.active_page = 0;
                        }
                    }
                }
            }
        }
    }
}

impl BeacnMicApp {
    fn draw_device_buttons(&mut self, ui: &mut Ui, device: DeviceDefinition) {
        if self.device_list.is_empty() || self.active_device.is_none() {
            debug!("NOT DRAWING");
            return;
        }

        let active_device = &self.active_device.clone().unwrap();
        match device.device_type {
            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                let Some(device_state) = self.audio_device_list.get(&device) else {
                    warn!("Missing audio device state for {:?}", device.location);
                    return;
                };
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
                        && !self.settings_active
                        && !self.mixer_active;
                    let error = matches!(
                        device_state.device_state.state,
                        LoadState::Error | LoadState::PermissionDenied | LoadState::ResourceBusy
                    );

                    if page.show_on_error() == error
                        && (!page.is_link_page() || page.is_studio_with_link(device_state))
                        && round_nav_button(ui, page.icon(), selected).clicked()
                    {
                        self.settings_active = false;
                        self.mixer_active = false;
                        self.active_device = Some(device.clone());
                        self.active_page = index;
                    }
                }
                ui.add_space(5.0);
                ui.separator();
            }
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
<<<<<<< feature/pipeweaver-preflight-setup
=======
                // This is identical to the above, except with a BeacnControllerState and ControllerPages
                // There's probably a way we can simplify this :p
>>>>>>> main
                let Some(device_state) = self.control_device_list.get(&device) else {
                    warn!("Missing control device state for {:?}", device.location);
                    return;
                };
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
                        && !self.settings_active
                        && !self.mixer_active;

                    let error = matches!(
                        device_state.device_state.state,
                        LoadState::Error | LoadState::PermissionDenied | LoadState::ResourceBusy
                    );
                    if page.show_on_error() == error
                        && round_nav_button(ui, page.icon(), selected).clicked()
                    {
                        self.mixer_active = false;
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

<<<<<<< feature/pipeweaver-preflight-setup
    fn render_content(&mut self, ui: &mut Ui) {
=======
    fn render_content(&mut self, ctx: &Context) {
>>>>>>> main
        if self.active_device.is_none() && !self.settings_active && !self.mixer_active {
            return;
        }

        if self.mixer_active {
<<<<<<< feature/pipeweaver-preflight-setup
            egui::CentralPanel::default().show_inside(ui, |ui| {
=======
            egui::CentralPanel::default().show(ctx, |ui| {
>>>>>>> main
                if let Some(ref pw_state) = self.pipeweaver_state {
                    mixer_ui(ui, pw_state, &mut self.mixer_page_state);
                } else {
                    ui.label("Pipeweaver not available — no Mix or Mix Create device connected.");
                }
            });
            return;
        }

        if self.settings_active {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                settings_ui(ui);
            });
            return;
        }

        let definition = &self.active_device.clone().unwrap();
        match definition.device_type {
            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                let settings = self.audio_device_list.get_mut(definition);
                if settings.is_none() {
                    return;
                }
                let settings = settings.unwrap();

                let error = matches!(
                    settings.device_state.state,
                    LoadState::Error | LoadState::PermissionDenied | LoadState::ResourceBusy
                );

                if error {
                    let position = self.audio_pages.iter().position(|p| p.show_on_error());
                    if let Some(page) = position {
                        self.active_page = page;
                    }
                }

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.audio_pages[self.active_page].ui(ui, settings);
                });
            }
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                let settings = self.control_device_list.get_mut(definition);
                if settings.is_none() {
                    return;
                }
                let settings = settings.unwrap();
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.control_pages[self.active_page].ui(ui, settings);
                });
            }
        }
    }
}