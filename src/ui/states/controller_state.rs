use crate::APP_NAME;
use crate::device_manager::{ControlMessage, DefinitionState, DeviceDefinition};
use crate::ui::states::{DeviceState, ErrorMessage, LoadState};
use anyhow::Result;
use beacn_lib::crossbeam::channel::Sender;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::time::Duration;
use xdg::BaseDirectories;

// Literally nothing to do here right now
#[derive(Debug, Default, Clone)]
pub struct BeacnControllerState {
    pub device_definition: DeviceDefinition,
    pub device_state: DeviceState,
    pub device_sender: Option<Sender<ControlMessage>>,

    pub saved_settings: SavedSettings,
}

impl BeacnControllerState {
    pub fn load_settings(definition: DeviceDefinition, sender: Sender<ControlMessage>) -> Self {
        let mut state = Self::default();
        state.device_sender = Some(sender);
        state.device_definition = definition;

        // Before we do anything else, is this definition in an error state?
        if let DefinitionState::Error(error) = &state.device_definition.state {
            state.device_state.state = LoadState::ERROR;
            state.device_state.errors.push(ErrorMessage {
                error_text: Some(format!("Failed to Open Device: {}", error)),
                failed_message: None,
            });

            return state;
        }

        state.device_state.state = LoadState::RUNNING;

        // Grab the settings from a possible saved config file
        state.load_from_file();
        let _ = state.set_display_brightness(state.saved_settings.button_brightness, false);
        let _ = state.set_button_brightness(state.saved_settings.display_brightness, false);
        let _ = state.set_display_dim(state.saved_settings.display_dim, false);

        state
    }

    pub fn set_display_brightness(&mut self, brightness: u8, save: bool) -> Result<()> {
        let (tx, rx) = oneshot::channel();

        self.saved_settings.display_brightness = brightness;
        let message = ControlMessage::DisplayBrightness(brightness, tx);
        self.send_control(message)?;
        rx.recv()??;
        if save {
            self.save_to_file();
        }
        Ok(())
    }

    pub fn set_button_brightness(&mut self, brightness: u8, save: bool) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.saved_settings.button_brightness = brightness;
        let message = ControlMessage::ButtonBrightness(brightness, tx);
        self.send_control(message)?;
        rx.recv()??;
        if save {
            self.save_to_file();
        }
        Ok(())
    }

    pub fn set_display_dim(&mut self, timeout: Duration, save: bool) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.saved_settings.display_dim = timeout;
        let message = ControlMessage::DimTimeout(timeout, tx);
        self.send_control(message)?;
        rx.recv()??;
        if save {
            self.save_to_file();
        }
        Ok(())
    }

    fn send_control(&self, message: ControlMessage) -> Result<()> {
        if let Some(tx) = &self.device_sender {
            tx.send(message)?;
        }
        Ok(())
    }

    pub fn load_from_file(&mut self) {
        let file_name = format!("{}.json", self.device_definition.device_info.serial);
        let xdg_dirs = BaseDirectories::with_prefix(APP_NAME);
        let config_file = xdg_dirs.find_config_file(file_name);

        debug!("Attempting to load Config from {:?}", config_file);
        if let Some(file) = config_file {
            if let Ok(file) = File::open(file) {
                if let Ok(config) = serde_json::from_reader(file) {
                    debug!("Load Successful");
                    self.saved_settings = config;
                    return;
                }
            }
        }

        debug!("Config Load Failed, Setting Defaults");
        // Load the default settings, then save them.
        self.saved_settings = SavedSettings::default();
        self.save_to_file();
    }

    pub fn save_to_file(&self) {
        let file_name = format!("{}.json", self.device_definition.device_info.serial);
        let xdg_dirs = BaseDirectories::with_prefix(APP_NAME);
        let config_file = xdg_dirs.place_config_file(file_name);

        if let Ok(file) = config_file {
            if let Ok(file) = File::create(file) {
                if let Err(e) = serde_json::to_writer_pretty(file, &self.saved_settings) {
                    warn!("Config Saving Failed: {}", e);
                }
            }
        }
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
