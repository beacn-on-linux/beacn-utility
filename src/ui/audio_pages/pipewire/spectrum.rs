use log::error;
use rustfft::{FftPlanner, num_complex::Complex};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

const EQ_CURVE_RESOLUTION: usize = 512;

const FFT_SIZE: usize = 4096;
const SMOOTHING_ALPHA: f32 = 0.25;

// The frequency range to be rendered
pub(crate) const MIN_FREQUENCY: u32 = 20;
pub(crate) const MAX_FREQUENCY: u32 = 20000;

pub struct SpectrumData {
    pub bins: Vec<f32>,
}

impl SpectrumData {
    pub(crate) fn new() -> Self {
        Self {
            bins: vec![f32::NEG_INFINITY; EQ_CURVE_RESOLUTION],
        }
    }
}

pub struct SpectrumHandle {
    thread: thread::JoinHandle<()>,
    stop_signal: Arc<AtomicBool>,
    pub data: Arc<Mutex<SpectrumData>>,
}

impl SpectrumHandle {
    pub fn stop(self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        self.thread.join().ok();
    }
}

pub fn start_spectrum_analyser(node_name: &str, sample_rate: u32) -> SpectrumHandle {
    let stop_signal = Arc::new(AtomicBool::new(false));
    let data = Arc::new(Mutex::new(SpectrumData::new()));

    let stop_clone = stop_signal.clone();
    let data_clone = data.clone();
    let node_name = node_name.to_string();

    let thread = thread::spawn(move || {
        analyser_inner(&node_name, sample_rate, data_clone, stop_clone);
    });

    SpectrumHandle {
        thread,
        stop_signal,
        data,
    }
}

/// Linear interpolation into a spectrum array at a fractional bin position.
fn interpolate(values: &[f32], pos: f32) -> f32 {
    let max_idx = values.len() - 1;
    let pos = pos.clamp(0.0, max_idx as f32);
    let idx0 = pos.floor() as usize;
    let idx1 = (idx0 + 1).min(max_idx);
    let frac = pos - idx0 as f32;
    values[idx0] * (1.0 - frac) + values[idx1] * frac
}

/// Fractional FFT-bin sample positions covering one EQ bin's range.
fn precompute_sample_positions(low_pos: f32, high_pos: f32) -> Vec<f32> {
    let span = (high_pos - low_pos).max(0.0);
    let n_samples = (span.ceil() as usize).clamp(4, 64);

    (0..n_samples)
        .map(|s| {
            let t = if n_samples == 1 {
                0.0
            } else {
                s as f32 / (n_samples - 1) as f32
            };
            low_pos + t * span
        })
        .collect()
}

/// Average power over a set of sample positions, returned in dB.
fn average_power_db(power: &[f32], positions: &[f32], min_db: f32) -> f32 {
    let mut sum = 0.0f32;
    for &pos in positions {
        sum += interpolate(power, pos);
    }
    let avg_power = sum / positions.len() as f32;

    if avg_power > 1e-12 {
        10.0 * avg_power.log10()
    } else {
        min_db
    }
}

// Fixed-width kernel = fixed fractional-octave smoothing, since EQ bins are
// log-spaced at equal ratios.
const GAUSSIAN_HALF_WIDTH: usize = 4;

fn gaussian_kernel(half_width: usize) -> Vec<f32> {
    let sigma = half_width as f32 / 2.0;
    let mut weights: Vec<f32> = (0..=(half_width * 2))
        .map(|i| {
            let x = i as f32 - half_width as f32;
            (-0.5 * (x / sigma).powi(2)).exp()
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    for w in weights.iter_mut() {
        *w /= sum;
    }
    weights
}

fn analyser_inner(name: &str, rate: u32, data: Arc<Mutex<SpectrumData>>, stop: Arc<AtomicBool>) {
    // let sample_rate = 48000;
    // let node_name = "alsa_input.hw_Mic_0";

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

    let hann: Vec<f32> = (0..FFT_SIZE)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
        })
        .collect();

    let bin_edges: Vec<f32> = (0..=EQ_CURVE_RESOLUTION)
        .map(|i| {
            let log_min = (MIN_FREQUENCY as f32).log10();
            let log_max = (MAX_FREQUENCY as f32).log10();
            let normalized = i as f32 / EQ_CURVE_RESOLUTION as f32;

            10f32.powf(log_min + normalized * (log_max - log_min))
        })
        .collect();

    let freq_resolution = rate as f32 / FFT_SIZE as f32;

    const MIN_DB: f32 = -120.0;

    let window_gain = hann.iter().sum::<f32>() / FFT_SIZE as f32;

    // Per-EQ-bin sample positions, computed once.
    let bin_sample_positions: Vec<Vec<f32>> = (0..EQ_CURVE_RESOLUTION)
        .map(|eq_index| {
            let low_pos = bin_edges[eq_index] / freq_resolution;
            let high_pos = bin_edges[eq_index + 1] / freq_resolution;
            precompute_sample_positions(low_pos, high_pos)
        })
        .collect();

    let gaussian_weights = gaussian_kernel(GAUSSIAN_HALF_WIDTH);

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let mut ring_buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE * 2);

    let mut raw = [0u8; 4096];

    // Scratch buffers, reused every frame..
    let mut spectrum: Vec<Complex<f32>> = vec![Complex { re: 0.0, im: 0.0 }; FFT_SIZE];
    let mut power: Vec<f32> = vec![0.0; FFT_SIZE / 2 + 1];
    let mut new_bins: Vec<f32> = vec![MIN_DB; EQ_CURVE_RESOLUTION];
    let mut frequency_smoothed: Vec<f32> = vec![MIN_DB; EQ_CURVE_RESOLUTION];

    // Temporal smoothing state, persists across frames.
    let mut smoothed = vec![MIN_DB; EQ_CURVE_RESOLUTION];

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        match stdout.read(&mut raw) {
            Ok(0) => break,

            Ok(n) => {
                for frame in raw[..n].chunks_exact(4) {
                    let sample = f32::from_le_bytes(frame.try_into().unwrap());

                    ring_buffer.push(sample);
                }

                while ring_buffer.len() >= FFT_SIZE {
                    let frame = &ring_buffer[..FFT_SIZE];

                    // Window + FFT
                    for (dst, (&sample, &window)) in
                        spectrum.iter_mut().zip(frame.iter().zip(hann.iter()))
                    {
                        dst.re = sample * window;
                        dst.im = 0.0;
                    }

                    fft.process(&mut spectrum);

                    // Magnitude -> power
                    for (dst, c) in power.iter_mut().zip(spectrum[..=FFT_SIZE / 2].iter()) {
                        let magnitude = 2.0 * c.norm() / (FFT_SIZE as f32 * window_gain);
                        *dst = magnitude * magnitude;
                    }

                    // Bin mapping: power-average each EQ bin's range
                    for eq_index in 0..EQ_CURVE_RESOLUTION {
                        new_bins[eq_index] =
                            average_power_db(&power, &bin_sample_positions[eq_index], MIN_DB);
                    }

                    // Frequency smoothing (fixed-width Gaussian)
                    for i in 0..EQ_CURVE_RESOLUTION {
                        let start = i as isize - GAUSSIAN_HALF_WIDTH as isize;
                        let mut sum = 0.0f32;
                        let mut weight_sum = 0.0f32;

                        for (k, &w) in gaussian_weights.iter().enumerate() {
                            let idx = start + k as isize;
                            if idx < 0 || idx >= EQ_CURVE_RESOLUTION as isize {
                                continue;
                            }
                            sum += new_bins[idx as usize] * w;
                            weight_sum += w;
                        }

                        frequency_smoothed[i] = if weight_sum > 0.0 {
                            sum / weight_sum
                        } else {
                            new_bins[i]
                        };
                    }

                    // Temporal smoothing
                    for (old, new) in smoothed.iter_mut().zip(frequency_smoothed.iter()) {
                        *old = SMOOTHING_ALPHA * *new + (1.0 - SMOOTHING_ALPHA) * *old;
                    }

                    if let Ok(mut guard) = data.lock() {
                        guard.bins.copy_from_slice(&smoothed);
                    }

                    // 50% overlap with the next frame
                    ring_buffer.drain(..FFT_SIZE / 2);
                }
            }

            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(1));
            }

            Err(_) => break,
        }
    }

    child.kill().ok();
    child.wait().ok();
}
