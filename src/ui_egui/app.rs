use crate::ToMainMessages;
use crate::devices::manager::{DeviceArriveMessage, DeviceDefinition, DeviceMessage};
use crate::devices::states::LoadState;
use crate::devices::states::audio::AudioState;
use crate::devices::states::control::ControlState;
use crate::integrations::pipeweaver::launch_pipeweaver_ui;
use crate::ui_egui::audio_pages::AudioPage;
use crate::ui_egui::controller_pages::ControllerPage;
use crate::ui_egui::pages::{pipeweaver_ui, settings_ui};
use crate::ui_egui::widgets::{pipeweaver_button, round_nav_button};
use crate::ui_egui::{audio_pages, controller_pages};
use crate::window_handle::App;
use beacn_lib::flume::{Receiver, Sender};
use beacn_lib::manager::DeviceType;
use egui::{
    Align, Button, Context, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Id, RichText,
    Ui,
};
use std::collections::HashMap;

pub struct BeacnMicApp {
    #[cfg_attr(unix, allow(unused))]
    main_sender: Sender<ToMainMessages>,

    show_close_modal: bool,

    device_list: Vec<DeviceDefinition>,
    active_device: Option<DeviceDefinition>,

    audio_device_list: HashMap<DeviceDefinition, AudioState>,
    control_device_list: HashMap<DeviceDefinition, ControlState>,

    audio_pages: Vec<Box<dyn AudioPage>>,
    control_pages: Vec<Box<dyn ControllerPage>>,

    device_recv: Receiver<DeviceMessage>,
    active_page: usize,

    // We can probably do better here
    mixer_active: bool,
    settings_active: bool,

    // Happens on the initial load when selecting default pages
    needs_page_open: bool,

    // Toast state for Pipeweaver button
    pipeweaver_toast_timer: Option<std::time::Instant>,
}

impl BeacnMicApp {
    pub fn new(main_sender: Sender<ToMainMessages>, device_recv: Receiver<DeviceMessage>) -> Self {
        Self {
            main_sender,

            show_close_modal: false,

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

            needs_page_open: false,

            pipeweaver_toast_timer: None,
        }
    }
}

impl App for BeacnMicApp {
    fn with_context(&mut self, ctx: &Context) {
        egui_extras::install_image_loaders(ctx);
        setup_fonts(ctx);
    }

    fn update(&mut self, ui: &mut Ui) {
        // Grab any device information that's been sent since the last update
        let messages: Vec<DeviceMessage> = self.device_recv.try_iter().collect();
        for message in messages {
            self.handle_device_message(message);
        }

        // Is our Device List empty?
        if self.device_list.is_empty() {
            egui::CentralPanel::default().show(ui, |ui: &mut Ui| {
                ui.add_sized(ui.available_size(), |ui: &mut Ui| {
                    ui.label("No Devices Detected")
                });
            });
            return;
        }

        // We need to trigger the page open if we need one
        if self.needs_page_open {
            self.open_current_page(ui.ctx());
            self.needs_page_open = false;
        }

        // Ok, next we need a modal for 'Close' behaviours
        let modal = egui::Modal::new(Id::new("close_behaviour"));
        if self.show_close_modal {
            modal.show(ui.ctx(), |ui| self.draw_close_modal(ui));
        }

        egui::Panel::left("left_panel")
            .resizable(false)
            .default_size(80.0)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(5.0);
                    let pipeweaver_btn = pipeweaver_button(ui, "pipeweaver", self.mixer_active);

                    if pipeweaver_btn.clicked() {
                        self.settings_active = false;
                        let should_toast = launch_pipeweaver_ui();

                        if should_toast {
                            self.mixer_active = false;
                            self.pipeweaver_toast_timer = Some(std::time::Instant::now());
                        } else {
                            self.close_current_page(ui.ctx());
                            self.settings_active = false;
                            self.mixer_active = true;
                            self.pipeweaver_toast_timer = None;
                        }
                    }

                    // Show toast if needed
                    if let Some(start) = self.pipeweaver_toast_timer {
                        let toast_duration = std::time::Duration::from_secs(2);
                        if start.elapsed() < toast_duration {
                            let pos = pipeweaver_btn.rect.right_center();
                            egui::Area::new(egui::Id::new("pipeweaver_toast"))
                                .fixed_pos([pos.x + 8.0, pos.y - 16.0])
                                .show(ui.ctx(), |ui| {
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.label("Pipeweaver UI Launched, check your task bar or app grid for notifications");
                                    });
                                });
                        } else {
                            self.pipeweaver_toast_timer = None;
                        }
                    }
                    ui.add_space(5.0);
                    ui.separator();

                    // Grab the device list, and reorder it based on type
                    let mut devices = self.device_list.clone();
                    devices.sort_by_key(|d| d.device_type);
                    for device in devices {
                        self.draw_device_buttons(ui, device);
                    }
                    ui.add_space(ui.available_height() - 55.0);
                    ui.separator();
                    if round_nav_button(ui, "gear", self.settings_active).clicked() {
                        self.close_current_page(ui.ctx());
                        self.mixer_active = false;
                        self.settings_active = true;
                    }
                });
            });

        // Render the main page
        self.render_content(ui);
    }

    fn should_close(&mut self) -> bool {
        #[cfg(not(unix))]
        {
            self.show_close_modal = true;
            false
        }

        // TODO: This should prompt the user, and / or check the settings
        #[cfg(unix)]
        true
    }

    fn on_close(&mut self) {
        #[cfg(not(unix))]
        {
            // // Quit the App completely.
            // let _ = self.main_sender.send(ToMainMessages::Quit);
        }

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
                    // Load the Device State
                    let state = AudioState::load_settings(definition.clone(), sender);

                    // Store the Device, and the device state
                    self.device_list.push(definition.clone());
                    self.audio_device_list.insert(definition.clone(), state);

                    if self.active_device.is_none() {
                        self.active_device = Some(definition);
                        self.needs_page_open = true;
                    }
                }
                DeviceArriveMessage::Control(definition, sender) => {
                    let state = ControlState::load_settings(definition.clone(), sender);
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
}

impl BeacnMicApp {
    fn draw_close_modal(&mut self, ui: &mut Ui) {
        ui.set_min_width(320.0);

        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new("Confirm Exit").size(20.0).strong());

            ui.add_space(12.0);
            ui.label("Closing will quit the application.");

            ui.add_space(20.0);
            ui.with_layout(egui::Layout::left_to_right(Align::Center), |ui| {
                ui.add_space((ui.available_width() - 210.0).max(0.0) / 2.0);

                if ui.add_sized([100.0, 32.0], Button::new("Cancel")).clicked() {
                    self.show_close_modal = false;
                }

                ui.add_space(10.0);
                if ui.add_sized([100.0, 32.0], Button::new("Quit")).clicked() {
                    self.show_close_modal = false;
                    let _ = self.main_sender.send(ToMainMessages::Quit);
                }
            });

            ui.add_space(8.0);
        });
    }

    fn draw_device_buttons(&mut self, ui: &mut Ui, device: DeviceDefinition) {
        if self.device_list.is_empty() || self.active_device.is_none() {
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

                let mut action = None;
                let audio_pages = self.audio_pages.iter_mut().enumerate();
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
                        && (page.should_show(device_state))
                        && round_nav_button(ui, page.icon(), selected).clicked()
                        && !selected
                    {
                        action = Some((device.clone(), index));
                    }
                }

                if let Some((device, index)) = action {
                    self.change_page(ui.ctx(), device, index);
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

                let mut action = None;
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
                        && !selected
                    {
                        action = Some((device.clone(), index));
                    }
                }

                if let Some((device, index)) = action {
                    self.change_page(ui.ctx(), device, index);
                }

                ui.add_space(5.0);
                ui.separator();
            }
        }
    }
    fn render_content(&mut self, ui: &mut Ui) {
        if self.active_device.is_none() && !self.settings_active && !self.mixer_active {
            return;
        }

        if self.mixer_active {
            egui::CentralPanel::default().show(ui, |ui| {
                pipeweaver_ui(ui);
            });
            return;
        }

        if self.settings_active {
            egui::CentralPanel::default().show(ui, |ui| {
                settings_ui(ui);
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

                let error = matches!(
                    settings.device_state.state,
                    LoadState::Error | LoadState::PermissionDenied | LoadState::ResourceBusy
                );

                // Are we in an error state, if so, show the error
                if error {
                    let position = self.audio_pages.iter().position(|p| p.show_on_error());
                    if let Some(page) = position {
                        self.active_page = page;
                    }
                }

                egui::CentralPanel::default().show(ui, |ui| {
                    self.audio_pages[self.active_page].ui(ui, settings);
                });
            }
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                let settings = self.control_device_list.get_mut(definition);
                if settings.is_none() {
                    return;
                }

                let settings = settings.unwrap();
                egui::CentralPanel::default().show(ui, |ui| {
                    self.control_pages[self.active_page].ui(ui, settings);
                });
            }
        }
    }

    fn change_page(&mut self, ctx: &Context, device: DeviceDefinition, page: usize) {
        self.close_current_page(ctx);

        // Update state
        self.active_device = Some(device);
        self.active_page = page;
        self.settings_active = false;
        self.mixer_active = false;

        self.open_current_page(ctx);
    }

    fn close_current_page(&mut self, ctx: &Context) {
        if self.settings_active || self.mixer_active {
            return;
        }

        let Some(device) = &self.active_device else {
            return;
        };

        match device.device_type {
            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                self.audio_pages[self.active_page].on_page_close(ctx);
            }
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                self.control_pages[self.active_page].on_page_close(ctx);
            }
        }
    }

    fn open_current_page(&mut self, ctx: &Context) {
        if self.settings_active || self.mixer_active {
            return;
        }

        let Some(device) = &self.active_device else {
            return;
        };

        match device.device_type {
            DeviceType::BeacnMic | DeviceType::BeacnStudio => {
                self.audio_pages[self.active_page].on_page_open(ctx);
            }
            DeviceType::BeacnMix | DeviceType::BeacnMixCreate => {
                self.control_pages[self.active_page].on_page_open(ctx);
            }
        }
    }
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "NotoSans-Regular".to_owned(),
        FontData::from_static(include_bytes!(
            "../../resources/fonts/noto/NotoSans-Regular.ttf"
        ))
        .tweak(FontTweak {
            scale: 0.95,
            ..Default::default()
        })
        .into(),
    );
    fonts.font_data.insert(
        "NotoSans-Bold".to_owned(),
        FontData::from_static(include_bytes!(
            "../../resources/fonts/noto/NotoSans-Bold.ttf"
        ))
        .tweak(FontTweak {
            scale: 0.95,
            ..Default::default()
        })
        .into(),
    );
    fonts.font_data.insert(
        "NotoSans-SemiBold".to_owned(),
        FontData::from_static(include_bytes!(
            "../../resources/fonts/noto/NotoSans-SemiBold.ttf"
        ))
        .tweak(FontTweak {
            scale: 0.95,
            ..Default::default()
        })
        .into(),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "NotoSans-Regular".to_owned());

    fonts.families.insert(
        FontFamily::Name("NotoSans-Bold".into()),
        vec!["NotoSans-Bold".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("NotoSans-SemiBold".into()),
        vec!["NotoSans-SemiBold".to_owned()],
    );

    ctx.set_fonts(fonts);
}

#[allow(unused)]
pub fn bold_text(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(
        size,
        FontFamily::Name("NotoSans-SemiBold".into()),
    ))
}
