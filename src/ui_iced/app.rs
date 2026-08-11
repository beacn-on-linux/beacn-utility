use crate::WindowMessage;
use crate::devices::manager::{DeviceDefinition, DeviceMessage};
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::devices::states::control::ControlState;
use crate::ui_iced::pages::page::Page;
use beacn_lib::flume::Receiver;
use beacn_lib::manager::DeviceLocation;
use iced::widget::{container, text};
use iced::{Element, Subscription, Task, window};
use std::collections::HashMap;

////////////////////////////////////////////////////////////////////////////////////////////
// This should probably be separated, but it's only a small abstraction
pub enum DeviceState {
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

pub struct Device {
    pub state: DeviceState,
    pub pages: Vec<Box<dyn Page>>,
}

////////////////////////////////////////////////////////////////////////////////////////////
// These are ingress flags, and are passed to the app

pub struct Flags {
    pub window_settings: window::Settings,

    pub window_rx: Receiver<WindowMessage>,
    pub device_rx: Receiver<DeviceMessage>,
}

////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub enum Message {
    ActivatePipeweaver,
    ActivateSettings,

    // Window Related Tasks
    WindowOpened(window::Id),
}

pub struct BeacnUtility {
    // List of devices we're currently aware of
    devices: HashMap<String, Device>,

    // Current active device and page
    active_device: Option<String>,
    active_page: Option<Box<dyn Page>>,

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
    pub fn new(flags: Flags) -> (Self, Task<Message>) {
        (
            Self {
                devices: HashMap::new(),

                active_device: None,
                active_page: None,

                mixer_active: false,
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
        "Beacn Utility".into()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(&self, _window_id: window::Id) -> Element<'_, Message> {
        container(text("Hello, world!")).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}
