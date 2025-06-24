use crate::device_manager::{ControlMessage, DefinitionState, DeviceDefinition};
use beacn_lib::crossbeam::channel::Sender;
use crate::ui::states::{DeviceState, ErrorMessage, LoadState};

// Literally nothing to do here right now
#[derive(Debug, Default, Clone)]
pub struct BeacnControllerState {
    pub device_definition: DeviceDefinition,
    pub device_state: DeviceState,
    pub device_sender: Option<Sender<ControlMessage>>,
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
        state
    }
}
