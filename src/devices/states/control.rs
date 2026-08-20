use crate::devices::manager::{ControlMessage, DefinitionState, DeviceDefinition, ErrorType};
use crate::devices::states::{DeviceLoadState, ErrorMessage, LoadState, State};
use crate::get_config_path;
use anyhow::{Result, bail};
use beacn_lib::controller::messages::Message;
use beacn_lib::flume::Sender;
use beacn_lib::manager::DeviceLocation;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::time::Duration;

// Literally nothing to do here right now
#[derive(Debug, Default, Clone)]
pub struct ControlState {
    pub device_definition: DeviceDefinition,
    pub device_state: DeviceLoadState,
    pub device_sender: Option<Sender<ControlMessage>>,

    pub saved_settings: SavedSettings,
}

impl State for ControlState {
    fn location(&self) -> &DeviceLocation {
        &self.device_definition.location
    }

    fn definition(&self) -> &DeviceDefinition {
        &self.device_definition
    }
}

impl ControlState {
    pub fn handle_message(&mut self, message: Message, save: bool) -> Result<Message> {
        let (tx, rx) = oneshot::channel();
        let message = ControlMessage::Handle(message, tx);

        match &self.device_sender {
            Some(sender) => {
                // Send the message, return the response (or fail).
                sender.send(message)?;
                let message = rx.recv()?;

                // Quickly intercept the message, and set our local value
                if let Ok(message) = &message
                    && self.set_local_value(message)
                    && save
                {
                    self.save_to_file();
                };
                Ok(message?)
            }
            None => bail!("Device Sender not Ready"),
        }
    }

    pub async fn handle_message_async(&mut self, message: Message, save: bool) -> Result<Message> {
        let (tx, rx) = oneshot::channel();
        let message = ControlMessage::Handle(message, tx);

        match &self.device_sender {
            Some(sender) => {
                // Send the message, return the response (or fail).
                sender.send_async(message).await?;
                let message = rx.await?;

                // Quickly intercept the message, and set our local value
                if let Ok(message) = &message
                    && self.set_local_value(message)
                    && save
                {
                    self.save_to_file();
                }

                Ok(message?)
            }
            None => bail!("Device Sender not Ready"),
        }
    }

    fn set_local_value(&mut self, message: &Message) -> bool {
        match message {
            Message::DisplayBrightness(b) => {
                self.saved_settings.display_brightness = *b;
                true
            }
            Message::ButtonBrightness(b) => {
                self.saved_settings.button_brightness = *b;
                true
            }
            Message::DisplayDimTime(t) => {
                self.saved_settings.display_dim = *t;
                true
            }
            _ => {
                // Anything else can't be saved, so don't try.
                false
            }
        }
    }

    pub async fn load_settings_async(
        definition: DeviceDefinition,
        sender: Sender<ControlMessage>,
    ) -> Self {
        let mut state = ControlState {
            device_definition: definition,
            device_sender: Some(sender),
            ..Default::default()
        };

        // Before we do anything else, is this definition in an error state?
        if let DefinitionState::Error(error) = &state.device_definition.state {
            match error {
                ErrorType::PermissionDenied => {
                    state.device_state.state = LoadState::PermissionDenied
                }
                ErrorType::ResourceBusy => state.device_state.state = LoadState::ResourceBusy,
                ErrorType::Other(s) => {
                    state.device_state.state = LoadState::Error;
                    state.device_state.errors.push(ErrorMessage {
                        error_text: Some(format!("Device Definition Error: {s}")),
                        failed_message: None,
                    });
                }
                ErrorType::Unknown => {
                    state.device_state.state = LoadState::Error;
                    state.device_state.errors.push(ErrorMessage {
                        error_text: Some("Unknown Error".to_string()),
                        failed_message: None,
                    });
                }
            }
            return state;
        }

        // Grab the settings from a possible saved config file
        state.load_from_file();
        let messages = [
            Message::DisplayBrightness(state.saved_settings.display_brightness),
            Message::ButtonBrightness(state.saved_settings.button_brightness),
            Message::DisplayDimTime(state.saved_settings.display_dim),
        ];

        debug!("Sending Initial Messages");
        for message in messages {
            debug!("Sending Message: {:?}", message);
            // Skip this message if it's not valid for this version
            if let Err(e) = state.handle_message_async(message.clone(), false).await {
                state.device_state.state = LoadState::Error;
                state.device_state.errors.push(ErrorMessage {
                    error_text: Some(format!("{e:?}")),
                    failed_message: None,
                })
            }
        }

        if state.device_state.state == LoadState::Loading {
            state.device_state.state = LoadState::Running;
        }
        state
    }

    pub fn load_from_file(&mut self) {
        let serial = self.device_definition.device_info.serial.clone();
        let file_name = format!("{}.json", serial);

        match get_config_path() {
            Ok(path) => {
                let config_file = path.join(file_name);
                if config_file.exists() {
                    match File::open(&config_file) {
                        Ok(file) => match serde_json::from_reader(file) {
                            Ok(config) => {
                                info!("Loaded Device config from: {:?}", config_file);
                                self.saved_settings = config;
                                return;
                            }
                            Err(e) => warn!("Config Loading Failed: {e}"),
                        },
                        Err(e) => warn!("Config Loading Failed: {e}"),
                    }
                } else {
                    info!("Creating Device Config for: {:?}", serial);
                }
            }
            Err(e) => {
                warn!("Unable to locate config directory, cannot load settings: {e}");
            }
        }

        // Load the default settings, then save them.
        self.saved_settings = SavedSettings::default();
        self.save_to_file();
    }

    pub fn save_to_file(&self) {
        let file_name = format!("{}.json", self.device_definition.device_info.serial);

        if let Ok(path) = get_config_path() {
            let config_file = path.join(file_name);
            match File::create(&config_file) {
                Ok(file) => {
                    if let Err(e) = serde_json::to_writer_pretty(file, &self.saved_settings) {
                        warn!("Config Saving Failed: {e}");
                    }
                }
                Err(e) => warn!("Config Saving Failed: {e}"),
            }
            return;
        }

        warn!("Unable to locate config directory, cannot save.");
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SavedSettings {
    #[serde(deserialize_with = "validate_screen_percent")]
    pub display_brightness: u8,

    #[serde(deserialize_with = "validate_display_dim")]
    pub display_dim: Duration,

    #[serde(deserialize_with = "validate_button_brightness")]
    pub button_brightness: u8,
}

impl Default for SavedSettings {
    fn default() -> Self {
        Self {
            display_brightness: 40,
            display_dim: Duration::from_secs(60 * 3),
            button_brightness: 5,
        }
    }
}

// This should never be a problem, but we'll validate the input fully.
fn validate_screen_percent<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let percent = u8::deserialize(deserializer)?;
    if percent > 100 {
        Err(serde::de::Error::custom("Percent should be below 100"))
    } else {
        Ok(percent)
    }
}

fn validate_button_brightness<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let brightness = u8::deserialize(deserializer)?;
    if brightness > 100 {
        Err(serde::de::Error::custom("Brightness should be below 10"))
    } else {
        Ok(brightness)
    }
}

fn validate_display_dim<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let timeout = Duration::deserialize(deserializer)?;
    if timeout > Duration::from_secs(60 * 4) {
        Err(serde::de::Error::custom(
            "Dim Time should be less than 4 minutes",
        ))
    } else {
        Ok(timeout)
    }
}
