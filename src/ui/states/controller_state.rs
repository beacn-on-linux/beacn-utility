use crate::device_manager::{ControlMessage, DeviceDefinition};
use beacn_lib::crossbeam::channel::Sender;
use crate::ui::states::{DeviceState, LoadState};

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

        state.device_state.state = LoadState::RUNNING;
        state
    }
}
