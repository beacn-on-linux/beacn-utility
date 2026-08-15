use log::error;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use std::process::Stdio;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::task::JoinHandle;

const EQ_CURVE_RESOLUTION: usize = 128;

// The frequency range to be rendered
pub(crate) const MIN_FREQUENCY: f32 = 20.0;
pub(crate) const MAX_FREQUENCY: f32 = 20000.0;

pub(crate) const MIN_DB: f32 = -120.0;

pub struct SpectrumHandle {
    task: JoinHandle<()>,
    stop_signal: Arc<AtomicBool>,
    pub data: Arc<Mutex<Vec<f32>>>,
}

impl SpectrumHandle {
    pub fn stop(self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        self.task.abort();
    }

    pub fn has_stopped(&self) -> bool {
        self.task.is_finished()
    }
}

pub fn start_spectrum_analyser(node_name: &str, sample_rate: u32) -> SpectrumHandle {
    let stop_signal = Arc::new(AtomicBool::new(false));
    let data = Arc::new(Mutex::new(vec![MIN_DB; EQ_CURVE_RESOLUTION]));

    let stop_clone = stop_signal.clone();
    let data_clone = data.clone();
    let node_name = node_name.to_string();

    let task = tokio::spawn(async move {
        analyser_inner(&node_name, sample_rate, data_clone, stop_clone).await;
    });

    SpectrumHandle {
        task,
        stop_signal,
        data,
    }
}

async fn analyser_inner(name: &str, rate: u32, data: Arc<Mutex<Vec<f32>>>, stop: Arc<AtomicBool>) {
    let mut child = match Command::new("pw-record")
        .args([
            "--target",
            name,
            "--rate",
            &rate.to_string(),
            "--channel-map",
            // TODO: Caller should pass this in
            "[AUX3]",
            "--format",
            "f32",
            "--raw",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to spawn pw-record: {e}");
            return;
        }
    };

    let mut stdout = child.stdout.take().unwrap();

    let mut raw = [0u8; 4096];

    // Just some usefully recyclable buffers
    let mut frames = vec![MIN_DB; EQ_CURVE_RESOLUTION];
    let mut points = vec![MIN_DB; EQ_CURVE_RESOLUTION];

    let mut handler = DynamicSpectrumAnalyzer::new(rate as f32);
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        match stdout.read(&mut raw).await {
            Ok(0) => break,
            Ok(n) => {
                frames.clear();
                frames.extend(
                    raw[..n]
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap())),
                );

                handler.push_incoming_samples(&frames);
                handler.render_spectrum_frame(&mut points);

                if let Ok(mut guard) = data.lock() {
                    guard.copy_from_slice(&points);
                }
            }

            Err(e) => {
                error!("pw-record failed: {e}");
                break;
            }
        }
    }

    child.kill().await.ok();
    child.wait().await.ok();
}

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
        for i in 0..self.fft_size {
            let sample = self.history[i];
            let window = 0.5 * (1.0 - ((2.0 * PI * i as f32) / (self.fft_size as f32 - 1.0)).cos());
            fft_buffer[i] = Complex::new(sample * window, 0.0);
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
        for i in 0..num_points {
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
            for bin in bin_low..=bin_high {
                let m = fft_buffer[bin].norm() * scale_factor;
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
            if db > 0.0 {
                db = 0.0;
            }
            if db < MIN_DB {
                db = MIN_DB;
            }

            // Ballistic damping across frames for fluid rendering
            output_db[i] = (0.25 * db) + (0.75 * output_db[i]);
        }
    }
}
