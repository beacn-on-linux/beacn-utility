use crate::devices::states::audio::EqualiserBandConfig;
use beacn_lib::audio::messages::eq_common::{EQBand, EQBandType};
use beacn_lib::audio::messages::eq_common::EQBandType::*;
use enum_map::EnumMap;
use iced::mouse::ScrollDelta;
use iced::{Point, Rectangle, Size};

/// A full set of equaliser bands, keyed by `EqualiserBand`.
pub type Bands = EnumMap<EQBand, EqualiserBandConfig>;

// The frequency range to be rendered.
pub const MIN_FREQUENCY: u32 = 20;
pub const MAX_FREQUENCY: u32 = 20000;

// The acceptable gain range.
pub const MIN_GAIN: f32 = -12.0;
pub const MAX_GAIN: f32 = 12.0;

// The actual graph area is offset by this much for things like labels.
pub const EQ_MARGIN: Size = Size::new(25.0, 20.0);

// When attempting to interact with a dot, this is how far outside we look.
pub const EQ_GRAB_THRESHOLD: f32 = 20.0;

/// Whether this band type has a meaningful gain value (as opposed to
/// filters like High/Low Pass or Notch, which are always drawn at 0dB).
pub fn band_type_has_gain(band_type: EQBandType) -> bool {
    !matches!(band_type, HighPassFilter | LowPassFilter | NotchFilter)
}

const VALUE_PER_LINE: f32 = 0.2;
const PIXELS_PER_LINE: f32 = 20.0;

pub fn get_q_delta(delta: ScrollDelta) -> f32 {
    match delta {
        ScrollDelta::Lines { y, .. } => y * VALUE_PER_LINE,
        ScrollDelta::Pixels { y, .. } => y * VALUE_PER_LINE / PIXELS_PER_LINE,
    }
}

/// Pure coordinate-space math shared by both the rendering view and the
/// interaction/controls layer.
///
/// None of this touches Iced painting or device messaging, so it can be
/// reused anywhere the EQ needs to convert between widget space and
/// frequency/gain space.
pub struct EqGeometry;

impl EqGeometry {
    /// The inner rectangle actually used for plotting, inset from the
    /// outer widget rectangle to leave room for axis labels.
    pub fn plot_rect(rect: Rectangle) -> Rectangle {
        Rectangle {
            x: rect.x + EQ_MARGIN.width,
            y: rect.y + EQ_MARGIN.height,
            width: rect.width - EQ_MARGIN.width,
            height: rect.height - EQ_MARGIN.height - 4.0,
        }
    }

    // pub fn plot_rect(rect: Rectangle) -> Rectangle {
    //     Rectangle {
    //         x: rect.x + EQ_MARGIN.width,
    //         y: rect.y + EQ_MARGIN.height,
    //         width: rect.width - EQ_MARGIN.width,
    //         height: rect.height - EQ_MARGIN.height,
    //     }
    // }

    /// Convert a frequency in Hz to an X coordinate.
    ///
    /// Frequency is mapped logarithmically, as expected for an EQ display.
    pub fn freq_to_x(freq: u32, plot_rect: Rectangle) -> f32 {
        let log_min = (MIN_FREQUENCY as f32).log10();
        let log_max = (MAX_FREQUENCY as f32).log10();
        let log_f = (freq as f32).log10();

        let normalized = (log_f - log_min) / (log_max - log_min);

        plot_rect.x + normalized * plot_rect.width
    }

    /// Convert an X coordinate back to frequency in Hz.
    pub fn x_to_freq(x: f32, plot_rect: Rectangle) -> f32 {
        let log_min = (MIN_FREQUENCY as f32).log10();
        let log_max = (MAX_FREQUENCY as f32).log10();

        let normalized = (x - plot_rect.x) / plot_rect.width;
        let log_f = log_min + normalized * (log_max - log_min);

        10.0_f32.powf(log_f)
    }

    /// Convert gain in dB to a Y coordinate.
    pub fn db_to_y(db: f32, plot_rect: Rectangle) -> f32 {
        let normalized = (MAX_GAIN - db) / (MAX_GAIN - MIN_GAIN);

        plot_rect.y + normalized * plot_rect.height
    }

    /// Convert a Y coordinate back to gain in dB.
    pub fn y_to_db(y: f32, plot_rect: Rectangle) -> f32 {
        let normalized = (y - plot_rect.y) / plot_rect.height;

        MAX_GAIN - normalized * (MAX_GAIN - MIN_GAIN)
    }

    /// Find the enabled band whose control point is nearest to `pointer`,
    /// provided it is within `EQ_GRAB_THRESHOLD` pixels.
    ///
    /// This is deliberately kept in the geometry/interaction layer rather
    /// than inside the drawing widget.
    pub fn hit_test(plot_rect: Rectangle, pointer: Point, bands: &Bands) -> Option<EQBand> {
        let mut closest_dist = f32::MAX;
        let mut closest_band = None;

        for (band, value) in bands {
            if !value.enabled {
                continue;
            }

            let x = Self::freq_to_x(value.frequency, plot_rect);

            let y = if band_type_has_gain(value.band_type) {
                Self::db_to_y(value.gain, plot_rect)
            } else {
                Self::db_to_y(0.0, plot_rect)
            };

            let point = Point::new(x, y);

            let dx = point.x - pointer.x;
            let dy = point.y - pointer.y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance < closest_dist && distance < EQ_GRAB_THRESHOLD {
                closest_dist = distance;
                closest_band = Some(band);
            }
        }

        closest_band
    }
}
