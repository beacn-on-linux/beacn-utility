use crate::ui::utility::pipewire::platform::audio::get_audio;
use crate::ui::utility::pipewire::ring_buffer::RingBuffer;
use crate::ui::utility::pipewire::{
    InputStream, LoopbackHandler, OutputStream, PipewireStream, SampleBuffer,
};
use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const SAMPLE_RATE: usize = 48000;
const BUFFER_LEN_SECS: usize = 10;

impl LoopbackHandler {
    pub fn new(input_port: u32, output_port: u32) -> Self {
        // 10 seconds of sample data at 48khz
        let capacity = SAMPLE_RATE * BUFFER_LEN_SECS;

        let ring_buffer = RingBuffer::new(capacity);
        let samples_len = ring_buffer.len_handle();

        Self {
            task: None,
            stop_signal: Arc::new(AtomicBool::new(false)),
            samples: Arc::new(SampleBuffer(UnsafeCell::new(ring_buffer))),
            samples_len,

            input_port,
            output_port,
        }
    }

    pub fn is_running(&self) -> bool {
        if let Some(task) = &self.task {
            !task.is_finished()
        } else {
            false
        }
    }

    pub fn perform_record(&mut self) {
        // Make sure we're not already running
        self.stop();

        // Clear any existing buffer
        self.clear_buffer();

        let samples_fill = self.samples.clone();
        let stopper = self.stop_signal.clone();
        let input_handler = InputStream {
            channel_id: self.input_port,
            process: Box::new(move |buf| {
                let samples_fill = unsafe { &mut *samples_fill.0.get() };

                // If we've hit the end of the buffer, we should trigger an internal stop.
                if samples_fill.write(buf) < buf.len() {
                    stopper.store(true, Ordering::Relaxed);
                }
            }),
        };

        let stopper = self.stop_signal.clone();
        let handle = thread::spawn(move || {
            let _ = get_audio(PipewireStream::Input(vec![input_handler]), stopper);
        });

        self.task = Some(handle);
    }

    pub fn perform_playback(&mut self) {
        // Make sure nothing's running
        self.stop();

        let samples_play = self.samples.clone();
        let output_handler = OutputStream {
            channel_id: self.output_port,
            process: Box::new(move |buf| {
                let samples_play = unsafe { &mut *samples_play.0.get() };
                samples_play.read_looped(buf);
            }),
        };

        let stopper = self.stop_signal.clone();
        let handle = thread::spawn(move || {
            let _ = get_audio(PipewireStream::Output(vec![output_handler]), stopper);
        });
        self.task = Some(handle);
    }

    pub fn stop(&mut self) {
        // Trigger the Stop, then wait for the thread to end
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(thread) = self.task.take() {
            self.stop_signal.store(true, Ordering::Relaxed);
            let _ = thread.join();
        }

        // Reset the Stop Signaller
        self.stop_signal.store(false, Ordering::Relaxed);
    }

    fn clear_buffer(&mut self) {
        let samples = unsafe { &mut *self.samples.0.get() };
        samples.clear();
    }

    fn current_len(&self) -> Duration {
        let len = self.samples_len.load(Ordering::Acquire);
        Duration::from_secs_f64(len as f64 / SAMPLE_RATE as f64)
    }
}
