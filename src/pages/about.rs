use crate::state::BeacnMicState;
use beacn_mic_lib::device::BeacnMic;
use std::cell::RefCell;
use std::rc::Rc;

#[allow(unused)]
pub struct About {
    mic: Rc<BeacnMic>,
    state: Rc<RefCell<BeacnMicState>>,
}

impl About {
    pub fn new(mic: Rc<BeacnMic>, state: Rc<RefCell<BeacnMicState>>) -> Self {
        Self { mic, state }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        ui.heading("About Section");
    }
}
