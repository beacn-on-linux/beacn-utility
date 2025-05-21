use std::f32::consts::PI;
use wide::f32x8;

static SAMPLE_RATE: f32 = 48000.0;

// Reference: https://www.w3.org/TR/audio-eq-cookbook/
pub(crate) struct EQUtil;
impl EQUtil {
    /// Calculate frequency response magnitude in dB for given frequency and filter coefficient
    pub(crate) fn freq_response_scalar(freq: f32, coefficients: &BiquadCoefficient) -> f32 {
        // Compute angular frequencies
        let w = 2.0 * PI * freq / SAMPLE_RATE;
        let cos_w = w.cos();
        let cos_2w = (2.0 * w).cos();
        let sin_w = w.sin();
        let sin_2w = (2.0 * w).sin();

        // Numerator components
        let num_real = coefficients.b0 + coefficients.b1 * cos_w + coefficients.b2 * cos_2w;
        let num_imag = -(coefficients.b1 * sin_w + coefficients.b2 * sin_2w);

        // Denominator components
        let den_real = 1.0 + coefficients.a1 * cos_w + coefficients.a2 * cos_2w;
        let den_imag = -(coefficients.a1 * sin_w + coefficients.a2 * sin_2w);

        // Magnitude squared of denominator
        let denom_mag_sq = den_real * den_real + den_imag * den_imag;

        // Complex division (num / den)
        let real = (num_real * den_real + num_imag * den_imag) / denom_mag_sq;
        let imag = (num_imag * den_real - num_real * den_imag) / denom_mag_sq;

        // Magnitude of frequency response
        let mag = (real * real + imag * imag).sqrt();

        // Convert magnitude to dB scale
        20.0 * mag.log10()
    }

    /// Calculate frequency response magnitude in dB for 8 frequencies via SIMD
    pub(crate) fn freq_response_simd(freqs: f32x8, coefficients: &BiquadCoefficient) -> f32x8 {
        // This is basically the same as the scalar version, except we do 8 frequencies at once
        let b0 = f32x8::splat(coefficients.b0);
        let b1 = f32x8::splat(coefficients.b1);
        let b2 = f32x8::splat(coefficients.b2);
        let a1 = f32x8::splat(coefficients.a1);
        let a2 = f32x8::splat(coefficients.a2);

        // Compute angular frequencies
        let w = freqs * f32x8::splat(2.0 * PI / SAMPLE_RATE);

        let cos_w = w.cos();
        let cos_2w = (w * f32x8::splat(2.0)).cos();
        let sin_w = w.sin();
        let sin_2w = (w * f32x8::splat(2.0)).sin();

        // Numerator components
        let num_real = b0 + b1 * cos_w + b2 * cos_2w;
        let num_imag = -(b1 * sin_w + b2 * sin_2w);

        // Denominator components
        let den_real = f32x8::splat(1.0) + a1 * cos_w + a2 * cos_2w;
        let den_imag = -(a1 * sin_w + a2 * sin_2w);

        // Magnitude squared of denominator
        let denom_mag_sq = den_real * den_real + den_imag * den_imag;

        // Complex division (num / den)
        let real = (num_real * den_real + num_imag * den_imag) / denom_mag_sq;
        let imag = (num_imag * den_real - num_real * den_imag) / denom_mag_sq;

        // Magnitude of frequency response
        let mag = (real * real + imag * imag).sqrt();

        // Convert magnitude to dB scale
        f32x8::splat(20.0) * mag.log10()
    }

    // Coefficient Calculations
    pub(crate) fn low_shelf_coefficient(freq: f32, gain: f32, q: f32) -> BiquadCoefficient {
        let a = 10.0_f32.powf(gain / 40.0);
        let w0 = 2.0 * PI * freq / SAMPLE_RATE;

        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let sqrt_a = a.sqrt();

        let b0 = a * (a + 1.0 - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
        let b1 = 2.0 * a * (a - 1.0 - (a + 1.0) * cos_w0);
        let b2 = a * (a + 1.0 - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
        let a0 = a + 1.0 + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        let a1 = -2.0 * (a - 1.0 + (a + 1.0) * cos_w0);
        let a2 = a + 1.0 + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;

        let mut coefficients = BiquadCoefficient {
            b0,
            b1,
            b2,
            a0,
            a1,
            a2,
        };
        Self::normalise(&mut coefficients);
        coefficients
    }

    pub(crate) fn high_shelf_coefficient(freq: f32, gain: f32, q: f32) -> BiquadCoefficient {
        let a = 10.0_f32.powf(gain / 40.0);
        let sqrt_a = a.sqrt();

        let w0 = 2.0 * PI * freq / SAMPLE_RATE;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = a * (a + 1.0 + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
        let b1 = -2.0 * a * (a - 1.0 + (a + 1.0) * cos_w0);
        let b2 = a * (a + 1.0 + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
        let a0 = a + 1.0 - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        let a1 = 2.0 * (a - 1.0 - (a + 1.0) * cos_w0);
        let a2 = a + 1.0 - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;

        let mut coefficients = BiquadCoefficient {
            b0,
            b1,
            b2,
            a0,
            a1,
            a2,
        };
        Self::normalise(&mut coefficients);
        coefficients
    }

    pub(crate) fn bell_coefficient(freq: f32, gain: f32, q: f32) -> BiquadCoefficient {
        let a = 10.0_f32.powf(gain / 40.0);
        let w0 = 2.0 * PI * freq / SAMPLE_RATE;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        let mut coefficients = BiquadCoefficient {
            b0,
            b1,
            b2,
            a0,
            a1,
            a2,
        };
        Self::normalise(&mut coefficients);
        coefficients
    }

    pub(crate) fn notch_coefficient(freq: f32, q: f32) -> BiquadCoefficient {
        // Notch filter gain param is usually ignored; Q defines bandwidth
        let w0 = 2.0 * PI * freq / SAMPLE_RATE;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        let mut coefficients = BiquadCoefficient {
            b0,
            b1,
            b2,
            a0,
            a1,
            a2,
        };
        Self::normalise(&mut coefficients);
        coefficients
    }

    pub(crate) fn high_pass_coefficient(freq: f32, q: f32) -> BiquadCoefficient {
        let w0 = 2.0 * PI * freq / SAMPLE_RATE;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        let mut coefficients = BiquadCoefficient {
            b0,
            b1,
            b2,
            a0,
            a1,
            a2,
        };
        Self::normalise(&mut coefficients);
        coefficients
    }

    pub(crate) fn low_pass_coefficient(freq: f32, q: f32) -> BiquadCoefficient {
        let w0 = 2.0 * PI * freq / SAMPLE_RATE;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        let mut coefficients = BiquadCoefficient {
            b0,
            b1,
            b2,
            a0,
            a1,
            a2,
        };
        Self::normalise(&mut coefficients);
        coefficients
    }

    fn normalise(coefficients: &mut BiquadCoefficient) {
        coefficients.b0 /= coefficients.a0;
        coefficients.b1 /= coefficients.a0;
        coefficients.b2 /= coefficients.a0;
        coefficients.a1 /= coefficients.a0;
        coefficients.a2 /= coefficients.a0;
    }
}

/// Represents the coefficients for a biquad filter
#[derive(Clone, Debug)]
pub struct BiquadCoefficient {
    pub(crate) a0: f32,
    pub(crate) a1: f32,
    pub(crate) a2: f32,
    pub(crate) b0: f32,
    pub(crate) b1: f32,
    pub(crate) b2: f32,
}
