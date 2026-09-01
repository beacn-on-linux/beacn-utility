mod ring_buffer;

use crate::ui::utility::pipewire::ring_buffer::RingBuffer;
use std::cell::{RefCell, UnsafeCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub type InputProcess = Box<dyn FnMut(&[f32]) + Send + Sync>;
#[allow(unused)]
pub struct InputStream {
    pub channel_id: u32,
    pub process: InputProcess,
}

pub type OutputProcess = Box<dyn FnMut(&mut [f32]) + Send + Sync>;
#[allow(unused)]
pub struct OutputStream {
    pub channel_id: u32,
    pub process: OutputProcess,
}

#[allow(unused)]
pub enum PipewireStream {
    Input(Vec<InputStream>),
    Output(Vec<OutputStream>),
}

#[allow(unused)]
impl PipewireStream {
    pub fn len(&self) -> usize {
        match self {
            PipewireStream::Input(v) => v.len(),
            PipewireStream::Output(v) => v.len(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct PipeWireNode {
    pub name: String,
    pub id: u32,
    pub is_split_child: bool,
    pub node_type: PipeWireNodeType,
    pub channels: HashMap<String, u32>,
    pub ports: Vec<PipeWirePort>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum PipeWireNodeType {
    Source,
    Sink,
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct PipeWirePort {
    pub name: String,
    pub id: u32,
    pub port_type: PipeWirePortType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum PipeWirePortType {
    Input,
    Output,
    Monitor,
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

// SAFETY: The buffer is only ever accessed by a single thread at a single point in time.
#[allow(unused)]
struct SampleBuffer(UnsafeCell<RingBuffer>);
unsafe impl Send for SampleBuffer {}
unsafe impl Sync for SampleBuffer {}

#[allow(unused)]
pub struct LoopbackHandler {
    task: Option<thread::JoinHandle<()>>,
    stop_signal: Arc<AtomicBool>,

    state: RefCell<LoopbackHandlerState>,

    samples: Arc<SampleBuffer>,

    samples_len: Arc<AtomicUsize>,
    samples_pos: Arc<AtomicUsize>,

    input_port: u32,
    output_port: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum LoopbackHandlerState {
    Recording,
    Playing,
    Stopped,
}

#[cfg(target_os = "linux")]
pub mod platform {
    mod audio;
    mod ffi;
    mod pod;

    pub mod device;
    pub mod loopback;
    pub mod spectrum;

    use crate::ui::utility::pipewire::{PipeWireNode, SpectrumHandle};
    use anyhow::Result;

    const TO_U32: fn(&String) -> Option<u32> = |s: &String| s.parse::<u32>().ok();
    const TO_BOOL: fn(&String) -> Option<bool> = |s: &String| s.parse::<bool>().ok();

    pub fn find_pipewire_nodes_for_usb(bus: u8, address: u8) -> Result<Vec<PipeWireNode>> {
        device::find_pipewire_nodes_for_usb(bus, address)
    }

    pub fn start_spectrum_analyser(ports: Vec<u32>, sample_rate: u32) -> SpectrumHandle {
        spectrum::start_spectrum_analyser(ports, sample_rate)
    }
}

#[cfg(not(target_os = "linux"))]
pub mod platform {
    use crate::ui::utility::pipewire::ring_buffer::RingBuffer;
    use crate::ui::utility::pipewire::{
        LoopbackHandler, LoopbackHandlerState, PipeWireNode, SampleBuffer, SpectrumHandle,
    };
    use anyhow::Result;
    use std::cell::{RefCell, UnsafeCell};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool};
    use std::thread;
    use std::time::Duration;

    pub fn find_pipewire_nodes_for_usb(_: u8, _: u8) -> Result<Vec<PipeWireNode>> {
        Ok(vec![])
    }

    pub fn start_spectrum_analyser(_: Vec<u32>, _: u32) -> SpectrumHandle {
        // This shouldn't be called on non-linux systems.

        let handle = thread::spawn(|| {});
        SpectrumHandle {
            task: handle,
            stop_signal: Arc::new(Default::default()),
            data: vec![],
        }
    }

    // This is a pure NOOP implementation of the loopback handler, as with the spectrum analyser
    // above this should never be called, but needs to exist for the config. I should probably
    // trait this but that's only relevant if there were other implementations, which there
    // currently aren't, and I'd still have the impl a no-op for it.
    impl LoopbackHandler {
        pub fn new(input_port: u32, output_port: u32) -> Self {
            let ring_buffer = RingBuffer::new(0);
            let samples_len = ring_buffer.len_handle();
            let samples_pos = ring_buffer.head_handle();

            Self {
                task: None,
                stop_signal: Arc::new(AtomicBool::new(false)),

                state: RefCell::new(LoopbackHandlerState::Stopped),

                samples: Arc::new(SampleBuffer(UnsafeCell::new(ring_buffer))),
                samples_len,
                samples_pos,

                input_port,
                output_port,
            }
        }

        pub fn state(&self) -> LoopbackHandlerState {
            LoopbackHandlerState::Stopped
        }
        pub fn perform_record(&mut self) {}
        pub fn perform_playback(&mut self) {}
        pub fn stop(&mut self) {}
        pub fn clear_buffer(&mut self) {}
        pub fn current_len(&self) -> Duration {
            Duration::from_secs(0)
        }
        pub fn current_pos(&self) -> Duration {
            Duration::from_secs(0)
        }
    }
}
