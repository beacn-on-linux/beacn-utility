use crate::devices::manager::{DeviceArriveMessage, DeviceDefinition, DeviceMessage};
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::devices::states::control::ControlState;
use crate::integrations::pipeweaver::launch_pipeweaver_ui;
use crate::ui::events::channel::TrackedReceiver;
use crate::ui::pages::app::pipeweaver::{PipeweaverMessage, PipeweaverPage};
use crate::ui::pages::app::settings::{SettingsMessage, SettingsPage};
use crate::ui::pages::page::{AP, CP, Page, PageMessage};
use crate::ui::pages::{audio, common, control};
use crate::ui::widgets::helpers::navigation::{
    pipeweaver_sidebar_item, round_nav_button, settings_sidebar_item,
};
use crate::{APP_TITLE, WindowMessage};
use beacn_lib::flume::Receiver;
use beacn_lib::manager::{DeviceLocation, DeviceType};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, column, container, row, rule, text};
use iced::{Alignment, Element, Length, Size, Subscription, Task, Theme, window};
use iced_futures::subscription::from_recipe;
use std::collections::HashMap;

////////////////////////////////////////////////////////////////////////////////////////////
// This should probably be separated, but it's only a small abstraction
#[allow(clippy::large_enum_variant)]
pub(crate) enum DeviceState {
    Audio(AudioState),
    Control(ControlState),
}

impl State for DeviceState {
    fn location(&self) -> &DeviceLocation {
        match self {
            DeviceState::Audio(state) => state.location(),
            DeviceState::Control(state) => state.location(),
        }
    }

    fn definition(&self) -> &DeviceDefinition {
        match self {
            DeviceState::Audio(state) => state.definition(),
            DeviceState::Control(state) => state.definition(),
        }
    }
}
////////////////////////////////////////////////////////////////////////////////////////////
// Unlike egui, we actually attach the pages to the device to keep the state synced

pub(crate) struct Device {
    pub state: DeviceState,
    pub pages: Vec<Box<dyn Page>>,
}

////////////////////////////////////////////////////////////////////////////////////////////
// These are ingress flags, and are passed to the app

pub(crate) struct Flags {
    pub window_settings: window::Settings,

    pub window_rx: Receiver<WindowMessage>,
    pub device_rx: Receiver<DeviceMessage>,
}

////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Message {
    Device(DeviceMessage),

    ActivatePipeweaver,
    Pipeweaver(PipeweaverMessage),

    ActivateSettings,
    Settings(SettingsMessage),

    // Page Selection
    SelectDeviceAndPage { device_id: String, page_id: usize },

    // Messages that are passed to the current page
    Page(PageMessage),

    // Window Related Tasks
    Quit,
    WindowOpen,
    WindowOpened(window::Id),
    WindowCloseRequested(window::Id),
    WindowResized((window::Id, Size)),
}

pub struct BeacnUtility {
    // List of devices we're currently aware of
    devices: HashMap<String, Device>,

    // Current active device and page
    active_device: Option<String>,
    active_page: Option<usize>,

    // Hard Coded Pages
    pipeweaver_page: PipeweaverPage,
    settings_page: SettingsPage,

    // These are overrides for showing the pipeweaver and settings pages
    mixer_active: bool,
    settings_active: bool,

    // Receiver for device notifications
    device_rx: Receiver<DeviceMessage>,

    // Window Tracking and Management
    window_settings: window::Settings,
    window_rx: Receiver<WindowMessage>,
    pub(crate) active_id: Option<window::Id>,
}

impl BeacnUtility {
    pub(crate) fn new(flags: Flags) -> (Self, Task<Message>) {
        (
            Self {
                devices: HashMap::new(),

                active_device: None,
                active_page: None,

                pipeweaver_page: PipeweaverPage::new(),
                mixer_active: false,

                settings_page: SettingsPage::new(),
                settings_active: false,

                device_rx: flags.device_rx,

                window_settings: flags.window_settings,
                window_rx: flags.window_rx,
                active_id: None,
            },
            Task::none(),
        )
    }

    pub fn title(&self, _window_id: window::Id) -> String {
        APP_TITLE.into()
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Device(msg) => {
                match msg {
                    DeviceMessage::DeviceArrived(arrival) => {
                        let device = match arrival {
                            DeviceArriveMessage::Audio(state) => Device {
                                state: DeviceState::Audio(state),
                                pages: create_pages_audio(),
                            },

                            DeviceArriveMessage::Control(state) => Device {
                                state: DeviceState::Control(state),
                                pages: create_pages_controller(),
                            },
                        };

                        let hash = device.state.location().hash.clone();
                        self.devices.insert(hash.clone(), device);

                        if self.active_device.is_none() {
                            // Find a visible page for this device
                            if let Some(page) = visible_pages(&self.devices[&hash]).first() {
                                self.active_device = Some(hash.clone());
                                self.active_page = Some(*page);

                                // Trigger the on_open callback for the first visible page
                                if let Some(device) = self.devices.get_mut(&hash) {
                                    device.pages[*page].on_open_fn(&device.state);
                                }
                            }
                        }
                    }
                    DeviceMessage::DeviceRemoved(location) => {
                        self.devices.remove(&location.hash);

                        if self.active_device.as_ref() == Some(&location.hash) {
                            self.active_device = None;
                            self.active_page = None;

                            for (hash, device) in &mut self.devices {
                                if let Some(page) = visible_pages(device).first() {
                                    self.active_device = Some(hash.clone());
                                    self.active_page = Some(*page);

                                    // Trigger the on_open callback for this page
                                    device.pages[*page].on_open_fn(&device.state);

                                    break;
                                }
                            }
                        }
                    }
                }
            }

            Message::ActivatePipeweaver => {
                if !launch_pipeweaver_ui() {
                    self.mixer_active = true;
                    self.settings_active = false;

                    self.active_device = None;
                    self.active_device = None;
                }
            }

            Message::ActivateSettings => {
                self.mixer_active = false;
                self.settings_active = true;

                self.active_device = None;
                self.active_device = None;
            }

            Message::Settings(msg) => {
                // Settings will need access to things like portals, which require the window
                // ID, so pass it along to messages.
                if let Some(id) = self.active_id {
                    return self.settings_page.update(id, msg).map(Message::Settings);
                }
            }

            Message::Pipeweaver(msg) => {
                return self.pipeweaver_page.update(msg).map(Message::Pipeweaver);
            }

            Message::SelectDeviceAndPage { device_id, page_id } => {
                // A device page has been clicked, we should clear out of pipewire / about
                self.mixer_active = false;
                self.settings_active = false;

                // Check if the requested selection is already exactly what is active
                let is_active_device = self.active_device.as_ref() == Some(&device_id);
                let is_active_page = self.active_page == Some(page_id);
                if is_active_device && is_active_page {
                    return Task::none();
                }

                // 1. If we get here, we need to close the current page before opening the new one
                if let Some(device) = &self.active_device
                    && let Some(page) = self.active_page
                {
                    // We need to try and pull this device from our devices
                    if let Some(device) = self.devices.get_mut(device) {
                        device.pages[page].on_close_fn();
                    }
                }

                // Firstly, find the new page
                if let Some(device) = self.devices.get_mut(&device_id)
                    && device.pages[page_id].should_show_fn(&device.state)
                {
                    self.active_device = Some(device_id.clone());
                    self.active_page = Some(page_id);
                    device.pages[page_id].on_open_fn(&device.state);
                }
            }

            Message::Page(msg) => {
                let Some(device_id) = &self.active_device else {
                    return Task::none();
                };

                let Some(page_index) = self.active_page else {
                    return Task::none();
                };

                let Some(device) = self.devices.get_mut(device_id) else {
                    return Task::none();
                };

                return device.pages[page_index]
                    .update_fn(&mut device.state, msg)
                    .map(Message::Page);
            }

            Message::Quit => {
                return iced::exit();
            }
            Message::WindowOpen => {
                if self.active_id.is_some() {
                    return Task::none();
                }

                // Spawn up the window
                let (id, task) = window::open(self.window_settings.clone());
                self.active_id = Some(id);

                // Trigger a callback on things to do when the window is actually opened.
                return task.map(move |_| Message::WindowOpened(id));
            }
            Message::WindowOpened(_id) => {}
            Message::WindowCloseRequested(id) => {
                self.active_id = None;
                return window::close(id);
            }
            Message::WindowResized((_, size)) => {
                self.window_settings.size = size;
            }
        }
        Task::none()
    }

    pub(crate) fn view(&self, _window_id: window::Id) -> Element<'_, Message> {
        // If no devices, display no devices message.
        if self.devices.is_empty() {
            return container(
                text("No Devices Detected")
                    .size(20)
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }

        // Add the pipeweaver button at the top
        let pipeweaver_button = pipeweaver_sidebar_item(self.mixer_active);
        let mut sidebar_items = column![pipeweaver_button].align_x(Alignment::Center);

        // Devices & Inner Pages Loop
        let mut sorted_devices: Vec<&Device> = self.devices.values().collect();
        sorted_devices.sort_by_key(|d| d.state.definition().device_type);
        for device in sorted_devices {
            let device_name = match &device.state.definition().device_type {
                DeviceType::BeacnMic => "Mic",
                DeviceType::BeacnStudio => "Studio",
                DeviceType::BeacnMixCreate => "Mix Create",
                DeviceType::BeacnMix => "Mix",
            };

            let mut device_group = column![].spacing(4).align_x(Alignment::Center);
            device_group = device_group.push(
                Space::new()
                    .width(Length::Shrink)
                    .height(Length::Fixed(6.0)),
            );

            // Add the section label text
            device_group = device_group.push(text(device_name).size(12.2));

            let hash = &device.state.location().hash;
            let is_device_active = self.active_device.as_ref() == Some(hash);

            // Nest the navigation icon buttons right underneath the label
            for page_id in visible_pages(device) {
                let img_key = device.pages[page_id].icon();
                let is_page_selected = is_device_active && self.active_page == Some(page_id);

                let mut btn = round_nav_button(img_key, is_page_selected);

                if !is_page_selected {
                    btn = btn.on_press(Message::SelectDeviceAndPage {
                        device_id: hash.clone(),
                        page_id,
                    });
                }

                device_group = device_group.push(btn);
            }

            device_group = device_group
                .push(
                    Space::new()
                        .width(Length::Shrink)
                        .height(Length::Fixed(1.0)),
                )
                .push(rule::horizontal(1));

            // Push the entire cleanly padded group into the master sidebar layout canvas
            sidebar_items = sidebar_items.push(device_group);
        }

        // Finally, add the settings button to the bottom
        sidebar_items = sidebar_items
            .push(Space::new().width(Length::Shrink).height(Length::Fill))
            .push(settings_sidebar_item(self.settings_active));

        // Generate the page content
        let content_area = if self.mixer_active {
            self.pipeweaver_page.view().map(Message::Pipeweaver)
        } else if self.settings_active {
            self.settings_page.view().map(Message::Settings)
        } else {
            match (&self.active_device, self.active_page) {
                (Some(id), Some(index)) => {
                    if let Some(device) = self.devices.get(id) {
                        device.pages[index]
                            .view_fn(&device.state)
                            .map(Message::Page)
                    } else {
                        container(text("Select a device")).into()
                    }
                }
                _ => container(text("No page active")).into(),
            }
        };

        // Assemble the final layout
        row![
            // Left side navigation
            container(sidebar_items)
                .width(Length::Fixed(80.0))
                .height(Length::Fill)
                .padding(5)
                .style(|theme: &Theme| container::Style {
                    background: Some(theme.palette().background.into()),
                    ..Default::default()
                }),
            rule::vertical(1),
            // Right side content
            container(content_area)
                .width(Length::Fill)
                .height(Length::Fill)
        ]
        .into()
    }

    pub(crate) fn subscription(&self) -> Subscription<Message> {
        let device = TrackedReceiver {
            id: "device_rx",
            rx: self.device_rx.clone(),
            map_fn: |data| Message::Device(data),
        };

        let window = TrackedReceiver {
            id: "window_rx",
            rx: self.window_rx.clone(),
            map_fn: |msg| match msg {
                WindowMessage::OpenWindow => Message::WindowOpen,
                WindowMessage::Quit => Message::Quit,
            },
        };

        let device_sub = from_recipe(device);
        let window_sub = from_recipe(window);
        let resize_sub = window::resize_events().map(Message::WindowResized);
        let close_sub = window::close_requests().map(Message::WindowCloseRequested);

        Subscription::batch(vec![device_sub, window_sub, resize_sub, close_sub])
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Page Helpers
fn create_pages_audio() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(AP(audio::config::Configuration::new())),
        Box::new(AP(audio::lighting::LightingPage::new())),
        Box::new(AP(audio::studio_link::StudioLink::new())),
        Box::new(AP(audio::about::About::new())),
        Box::new(common::error_page::ErrorPage::new()),
    ]
}

fn create_pages_controller() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(CP(control::about::About::new())),
        Box::new(common::error_page::ErrorPage::new()),
    ]
}

fn visible_pages(device: &Device) -> Vec<usize> {
    device
        .pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| page.should_show_fn(&device.state).then_some(index))
        .collect()
}
