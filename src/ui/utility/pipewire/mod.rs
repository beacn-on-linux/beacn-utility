use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(target_os = "linux")]
pub mod device;

#[cfg(target_os = "linux")]
pub mod spectrum;

#[cfg(target_os = "linux")]
mod audio;

#[cfg(target_os = "linux")]
mod ffi;

#[cfg(target_os = "linux")]
mod pod;

#[cfg(target_os = "linux")]
const TO_U32: fn(&String) -> Option<u32> = |s: &String| s.parse::<u32>().ok();

#[cfg(target_os = "linux")]
const TO_BOOL: fn(&String) -> Option<bool> = |s: &String| s.parse::<bool>().ok();

pub type Process = Box<dyn FnMut(&[f32]) + Send + Sync>;
#[allow(unused)]
pub struct ChannelStream {
    pub channel_id: u32,
    pub process: Process,
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct PipeWireNode {
    pub name: String,
    pub id: u32,
    pub is_split_child: bool,
    pub node_type: PipeWireNodeType,
    pub channels: HashMap<String, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum PipeWireNodeType {
    Source,
    Sink,
}

type SpectrumData = Vec<Arc<Mutex<Vec<f32>>>>;
#[allow(unused)]
pub struct SpectrumHandle {
    task: thread::JoinHandle<()>,
    stop_signal: Arc<AtomicBool>,
    pub data: SpectrumData,
}

impl SpectrumHandle {
    pub(crate) fn stop(self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        let _ = self.task.join();
    }
    pub fn has_stopped(&self) -> bool {
        self.task.is_finished()
    }
}

#[allow(unused_variables)]
pub fn find_pipewire_nodes_for_usb(bus: u8, address: u8) -> Result<Vec<PipeWireNode>> {
    #[cfg(target_os = "linux")]
    {
        device::find_pipewire_nodes_for_usb(bus, address)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(vec![])
    }
}

// TODO: This should probably result :D
#[allow(unused_variables)]
pub fn start_spectrum_analyser(ports: Vec<u32>, sample_rate: u32) -> SpectrumHandle {
    #[cfg(target_os = "linux")]
    spectrum::start_spectrum_analyser(ports, sample_rate);

    #[cfg(not(target_os = "linux"))]
    {
        // Reason it should result.. Should be noted that you can't call this without ports
        // and you can't get ports without PipeWireNodes.. So this function should, in theory,
        // NEVER be called on non-linux systems.
        let handle = thread::spawn(|| {});
        SpectrumHandle {
            task: handle,
            stop_signal: Arc::new(Default::default()),
            data: vec![],
        }
    }
}
