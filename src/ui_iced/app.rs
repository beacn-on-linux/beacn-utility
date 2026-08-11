use crate::devices::manager::DeviceDefinition;
use crate::devices::states::State;
use crate::devices::states::audio::AudioState;
use crate::devices::states::control::ControlState;
use beacn_lib::manager::DeviceLocation;

#[derive(Debug, Clone)]
pub enum Message {
    ActivatePipeweaver,
    ActivateSettings,
}

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
