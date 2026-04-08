use beacn_lib::audio::messages::Message;

pub(crate) mod audio_state;
pub(crate) mod controller_state;
<<<<<<< feature/pipeweaver-preflight-setup
pub(crate) mod pipeweaver_state;
=======
pub mod pipeweaver_state;
>>>>>>> main

#[derive(Debug, Default, Clone)]
pub struct DeviceState {
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
