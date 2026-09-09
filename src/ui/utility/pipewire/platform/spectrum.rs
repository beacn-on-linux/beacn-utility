use crate::ui::utility::pipewire::platform::audio::get_audio;
use crate::ui::utility::pipewire::{InputStream, PipewireStream, SpectrumData, SpectrumHandle};
use log::debug;
use rustfft::{Fft, FftPlanner, num_complex::Complex};
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

    let task = thread::spawn(move || analyser_inner(ports, sample_rate, data_clone, stop_clone));

    SpectrumHandle {
        task,
        stop_signal,
        data,
    }
}

// Take the ports, create spectrum handlers for them, then run the audio loop.
fn analyser_inner(ports: Vec<u32>, rate: u32, data: SpectrumData, stop: Arc<AtomicBool>) {
    let mut streams = vec![];
    for (index, port) in ports.iter().enumerate() {
        // Create the handler, and the points cache..
        let mut handler = DynamicSpectrumAnalyzer::new(rate as f32);
        let mut points = vec![MIN_DB; EQ_CURVE_RESOLUTION];

        // Clone our specific data vec
        let data = data[index].clone();

        // Move everything into the handling closure.
        debug!("Creating stream for port: {:?} (inner)", port);
        let stream = InputStream {
            channel_id: *port,
            process: Box::new(move |samples| {
                let sample_len = samples.len();
                handler.push_incoming_samples(samples);
                handler.render_spectrum_frame(&mut points, sample_len);

                if let Ok(mut guard) = data.lock() {
                    guard.copy_from_slice(&points);
                }
            }),
        };
        streams.push(stream);
    }

    let _ = get_audio(PipewireStream::Input(streams), stop);
}

#[derive(Clone)]
pub struct DynamicSpectrumAnalyzer {
    history: Vec<f32>,
    window: Vec<f32>,
    fft_buffer: Vec<Complex<f32>>,
    fft: Arc<dyn Fft<f32>>,
    sample_rate: f32,
    fft_size: usize,
}

impl DynamicSpectrumAnalyzer {
    pub fn new(sample_rate: f32) -> Self {
        let target_samples = (sample_rate * 0.085) as usize;
        let fft_size = target_samples.next_power_of_two().max(1024);

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        let mut window = Vec::with_capacity(fft_size);
        for i in 0..fft_size {
            let w = 0.5 * (1.0 - ((2.0 * PI * i as f32) / (fft_size as f32 - 1.0)).cos());
            window.push(w);
        }

        Self {
            history: vec![0.0; fft_size],
            window,
            fft_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            fft,
            sample_rate,
            fft_size,
        }
    }

    /// Add samples to the history
    pub fn push_incoming_samples(&mut self, new_block: &[f32]) {
        let incoming_len = new_block.len();
        if incoming_len == 0 {
            return;
        }

        if incoming_len >= self.fft_size {
            self.history
                .copy_from_slice(&new_block[incoming_len - self.fft_size..]);
            return;
        }

        let shift_amount = incoming_len;
        self.history.copy_within(shift_amount..self.fft_size, 0);

        let insert_start = self.fft_size - incoming_len;
        self.history[insert_start..self.fft_size].copy_from_slice(new_block);
    }

    /// Extract the current spectrum data
    pub fn render_spectrum_frame(&mut self, output_db: &mut [f32], sample_count: usize) {
        let num_points = output_db.len();
        if num_points == 0 || self.sample_rate <= 0.0 {
            return;
        }

        // Copy windowed samples into preallocated FFT buffer
        for (i, buffer) in self.fft_buffer.iter_mut().enumerate().take(self.fft_size) {
            let sample = self.history[i];
            let window = self.window[i];
            *buffer = Complex::new(sample * window, 0.0);
        }

        // 2. Perform Forward FFT
        self.fft.process(&mut self.fft_buffer);

        let min_freq = MIN_FREQUENCY;
        let max_freq = MAX_FREQUENCY;
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        // Hann window has an amplitude coherent gain of 0.5, so compensate with 4.0 / N
        let scale_factor = 4.0 / self.fft_size as f32;
        let max_valid_bin = self.fft_size / 2;

        let dt = (sample_count as f32 / self.sample_rate).max(0.001);
        // Realistic decay rate of ~28 dB/s provides smooth RTA release
        let max_decay = 28.0_f32 * dt;

        // 3. Map the bins using a band-aware log bucket approach
        for (i, item) in output_db.iter_mut().enumerate().take(num_points) {
            let t_norm = i as f32 / ((num_points - 1).max(1)) as f32;
            let target_freq = (log_min + t_norm * (log_max - log_min)).exp();

            // Establish frequency tracking boundaries in log space
            let step = (log_max - log_min) / (num_points as f32);
            let freq_low = (target_freq.ln() - step * 0.5).exp();
            let freq_high = (target_freq.ln() + step * 0.5).exp();

            // Convert frequency boundaries to FFT bin indices
            let bin_low_f = (freq_low * self.fft_size as f32) / self.sample_rate;
            let bin_high_f = (freq_high * self.fft_size as f32) / self.sample_rate;

            let bin_low = (bin_low_f.floor() as usize).clamp(0, max_valid_bin - 1);
            let bin_high = (bin_high_f.ceil() as usize).clamp(0, max_valid_bin - 1);

            let mut peak_mag = 0.0_f32;
            for bin in self.fft_buffer.iter().take(bin_high + 1).skip(bin_low) {
                let m = bin.norm() * scale_factor;
                if m > peak_mag {
                    peak_mag = m;
                }
            }

            if peak_mag == 0.0 && bin_low < max_valid_bin {
                let frac = bin_low_f - bin_low as f32;
                let b1 = (bin_low + 1).min(max_valid_bin - 1);
                let m0 = self.fft_buffer[bin_low].norm() * scale_factor;
                let m1 = self.fft_buffer[b1].norm() * scale_factor;
                peak_mag = m0 + frac * (m1 - m0);
            }

            // Convert directly to decibels
            let mut db = 20.0 * (peak_mag + 1e-6).log10();

            // Strict clamp output boundaries to required range
            db = db.clamp(MIN_DB, 0.0);

            // Ballistic damping: fast attack for vocal transients, smooth decay
            if db > *item {
                *item = (0.85 * db) + (0.15 * *item);
            } else {
                *item = (*item - max_decay).max(db);
            }
        }

        // Apply gentle 3-point spatial filter across log bins
        let temp = output_db.to_vec();
        for i in 1..num_points - 1 {
            output_db[i] = 0.20 * temp[i - 1] + 0.60 * temp[i] + 0.20 * temp[i + 1];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrum_analyzer_sine_wave() {
        let sample_rate = 48000.0;
        let mut analyzer = DynamicSpectrumAnalyzer::new(sample_rate);
        let mut output_db = vec![MIN_DB; EQ_CURVE_RESOLUTION];

        // Generate 1 kHz sine wave at -10 dBFS (amplitude ~ 0.316)
        let freq = 1000.0;
        let amplitude = 0.316_f32;
        let chunk_size = 256;
        let total_samples = 4096;

        for chunk_start in (0..total_samples).step_by(chunk_size) {
            let samples: Vec<f32> = (chunk_start..chunk_start + chunk_size)
                .map(|n| amplitude * (2.0 * PI * freq * (n as f32) / sample_rate).sin())
                .collect();
            analyzer.push_incoming_samples(&samples);
            analyzer.render_spectrum_frame(&mut output_db, samples.len());
        }

        // Find the peak in the output
        let mut max_db = MIN_DB;
        let mut peak_bin = 0;
        for (i, &db) in output_db.iter().enumerate() {
            if db > max_db {
                max_db = db;
                peak_bin = i;
            }
        }

        assert!(max_db > -18.0, "Expected peak > -18 dBFS, got {max_db}");
        let log_min = MIN_FREQUENCY.ln();
        let log_max = MAX_FREQUENCY.ln();
        let peak_freq = (log_min + (peak_bin as f32 / 127.0) * (log_max - log_min)).exp();
        assert!(
            (peak_freq - 1000.0).abs() < 150.0,
            "Expected peak near 1000 Hz, got {peak_freq} Hz at bin {peak_bin}"
        );
    }

    #[test]
    fn test_spectrum_analyzer_decay() {
        let sample_rate = 48000.0;
        let mut analyzer = DynamicSpectrumAnalyzer::new(sample_rate);
        let mut output_db = vec![MIN_DB; EQ_CURVE_RESOLUTION];

        // Send a burst of signal
        let samples = vec![0.5_f32; 1024];
        analyzer.push_incoming_samples(&samples);
        analyzer.render_spectrum_frame(&mut output_db, samples.len());
        let peak_before = output_db[50];

        // Send 0.1s of silence (4800 samples)
        let silence = vec![0.0_f32; 4800];
        analyzer.push_incoming_samples(&silence);
        analyzer.render_spectrum_frame(&mut output_db, silence.len());
        let peak_after = output_db[50];

        assert!(
            peak_after < peak_before,
            "Signal should decay after silence"
        );
        let decay = peak_before - peak_after;
        assert!(
            decay > 1.0 && decay < 5.0,
            "Expected ~2.8 dB decay over 0.1s, got {decay} dB"
        );
    }
}
