use crate::state::BeacnMicState2;
use beacn_mic_lib::device::BeacnMic;
use std::cell::RefCell;
use std::rc::Rc;

#[allow(unused)]
pub struct About {
    mic: Rc<BeacnMic>,
    state: Rc<RefCell<BeacnMicState2>>,
}

impl About {
    pub fn new(mic: Rc<BeacnMic>, state: Rc<RefCell<BeacnMicState2>>) -> Self {
        Self { mic, state }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        ui.heading("About Section");
    }
}
