use beacn_lib::audio::messages::Message;

pub mod audio;
pub mod control;

#[derive(Debug, Default, Clone)]
pub struct DeviceLoadState {
    pub state: LoadState,
    pub errors: Vec<ErrorMessage>,
}

#[derive(Debug, Default, Clone)]
pub struct ErrorMessage {
    pub error_text: Option<String>,
    pub failed_message: Option<Message>,
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum LoadState {
    #[default]
    Loading,
    Running,
    PermissionDenied,
    ResourceBusy,
    Error,
}
