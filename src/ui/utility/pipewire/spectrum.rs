use crate::ui::utility::pipewire::audio::get_audio;
use crate::ui::utility::pipewire::{ChannelStream, SpectrumData, SpectrumHandle};
use log::debug;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::thread;

const EQ_CURVE_RESOLUTION: usize = 128;

// The frequency range to be rendered
pub(crate) const MIN_FREQUENCY: f32 = 20.0;
pub(crate) const MAX_FREQUENCY: f32 = 20000.0;

pub(crate) const MIN_DB: f32 = -120.0;

// Take a vec of pipewire ports, spawn up the analyser in its own thread.
pub fn start_spectrum_analyser(ports: Vec<u32>, sample_rate: u32) -> SpectrumHandle {
    debug!("Starting Spectrum Analyser for {} ports", ports.len());
    let stop_signal = Arc::new(AtomicBool::new(false));
    let data = {
        let len = ports.len();
        let mut v = Vec::with_capacity(len);
        (0..len).for_each(|_| v.push(Arc::new(Mutex::new(vec![MIN_DB; EQ_CURVE_RESOLUTION]))));

        v
    };

    let stop_clone = stop_signal.clone();
    let data_clone = data.clone();

    let task = thread::spawn(move || analyser_inner2(ports, sample_rate, data_clone, stop_clone));

    SpectrumHandle {
        task,
        stop_signal,
        data,
    }
}

// Take the ports, create spectrum handlers for them, then run the audio loop.
fn analyser_inner2(ports: Vec<u32>, rate: u32, data: SpectrumData, stop: Arc<AtomicBool>) {
    let mut streams = vec![];
    for (index, port) in ports.iter().enumerate() {
        // Create the handler, and the points cache..
        let mut handler = DynamicSpectrumAnalyzer::new(rate as f32);
        let mut points = vec![MIN_DB; EQ_CURVE_RESOLUTION];

        // Clone our specific data vec
        let data = data[index].clone();

        // Move everything into the handling closure.
        debug!("Creating stream for port: {:?} (inner)", port);
        let stream = ChannelStream {
            channel_id: *port,
            process: Box::new(move |samples| {
                handler.push_incoming_samples(samples);
                handler.render_spectrum_frame(&mut points);

                if let Ok(mut guard) = data.lock() {
                    guard.copy_from_slice(&points);
                }
            }),
        };
        streams.push(stream);
    }

    let _ = get_audio(streams, stop);
}

#[derive(Clone)]
pub struct DynamicSpectrumAnalyzer {
    history: Vec<f32>,
    sample_rate: f32,
    fft_size: usize,
}

impl DynamicSpectrumAnalyzer {
    pub fn new(sample_rate: f32) -> Self {
        let target_samples = (sample_rate * 0.085) as usize;
        let fft_size = target_samples.next_power_of_two().max(1024);

        Self {
            history: vec![0.0; fft_size],
            sample_rate,
            fft_size,
        }
    }

    /// Add samples to the history
    pub fn push_incoming_samples(&mut self, new_block: &[f32]) {
        let incoming_len = new_block.len();
        if incoming_len == 0 || self.fft_size < incoming_len {
            return;
        }

        let shift_amount = incoming_len;
        self.history.copy_within(shift_amount..self.fft_size, 0);

        let insert_start = self.fft_size - incoming_len;
        self.history[insert_start..self.fft_size].copy_from_slice(new_block);
    }

    /// Extract the current spectrum data
    pub fn render_spectrum_frame(&self, output_db: &mut [f32]) {
        let num_points = output_db.len();
        if num_points == 0 || self.sample_rate <= 0.0 {
            return;
        }

        // Copy the chronological history straight into the FFT buffer
        let mut fft_buffer = vec![Complex::new(0.0, 0.0); self.fft_size];
        for (i, buffer) in fft_buffer.iter_mut().enumerate().take(self.fft_size) {
            let sample = self.history[i];
            let window = 0.5 * (1.0 - ((2.0 * PI * i as f32) / (self.fft_size as f32 - 1.0)).cos());
            *buffer = Complex::new(sample * window, 0.0);
        }

        // 2. Perform Forward FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.fft_size);
        fft.process(&mut fft_buffer);

        let min_freq = MIN_FREQUENCY;
        let max_freq = MAX_FREQUENCY;
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        let scale_factor = 2.0 / self.fft_size as f32;
        let max_valid_bin = self.fft_size / 2;

        // 3. Map the bins using a true band-aware bucket approach
        for (i, item) in output_db.iter_mut().enumerate().take(num_points) {
            let t_norm = i as f32 / ((num_points - 1).max(1)) as f32;
            let target_freq = (log_min + t_norm * (log_max - log_min)).exp();

            // Establish pixel tracking boundaries safely in log space
            let step = (log_max - log_min) / (num_points as f32);
            let freq_low = (target_freq.ln() - step * 0.5).exp();
            let freq_high = (target_freq.ln() + step * 0.5).exp();

            // Convert frequency boundaries to exact FFT bin indices
            let bin_low_f = (freq_low * self.fft_size as f32) / self.sample_rate;
            let bin_high_f = (freq_high * self.fft_size as f32) / self.sample_rate;

            let bin_low = (bin_low_f.floor() as usize).clamp(0, max_valid_bin - 1);
            let bin_high = (bin_high_f.ceil() as usize).clamp(0, max_valid_bin - 1);

            let mut peak_mag = 0.0_f32;
            for bin in fft_buffer.iter().take(bin_high + 1).skip(bin_low) {
                let m = bin.norm() * scale_factor;
                if m > peak_mag {
                    peak_mag = m;
                }
            }

            if peak_mag == 0.0 && bin_low < max_valid_bin {
                let frac = bin_low_f - bin_low as f32;
                let b1 = (bin_low + 1).min(max_valid_bin - 1);
                let m0 = fft_buffer[bin_low].norm() * scale_factor;
                let m1 = fft_buffer[b1].norm() * scale_factor;
                peak_mag = m0 + frac * (m1 - m0);
            }

            // Convert directly to decibels
            let mut db = 20.0 * (peak_mag + 1e-6).log10();

            // Strict clamp output boundaries to your required range
            db = db.clamp(MIN_DB, 0.0);

            // Ballistic damping across frames for fluid rendering
            *item = (0.25 * db) + (0.75 * *item);
        }
    }
}
