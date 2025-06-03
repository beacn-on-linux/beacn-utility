use beacn_lib::controller::BeacnControlDevice;
use beacn_lib::manager::DeviceType;

// Literally nothing to do here right now
#[derive(Debug, Default, Clone)]
pub struct ControlState {
    pub device_type: DeviceType,
}

impl ControlState {
    pub fn load_settings(dev: &Box<dyn BeacnControlDevice>, device_type: DeviceType) -> Self {
        let mut state = Self::default();
        state.device_type = device_type;

        state
    }
}
