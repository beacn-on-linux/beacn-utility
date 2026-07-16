use beacn_lib::audio::messages::equaliser::EQBandType::{
    HighPassFilter, LowPassFilter, NotchFilter,
};
use beacn_lib::audio::messages::equaliser::{EQBand, EQBandType};
use egui::{Pos2, Rect, Vec2};
use enum_map::EnumMap;

use crate::ui::states::audio_state::EqualiserBand;

/// A full set of equaliser bands, keyed by `EQBand`. Shared type so the
/// view and the controls layer agree on what they're passing around.
pub type Bands = EnumMap<EQBand, EqualiserBand>;

// The frequency range to be rendered
pub const MIN_FREQUENCY: u32 = 20;
pub const MAX_FREQUENCY: u32 = 20000;

// The Acceptable Gain Range
pub const MIN_GAIN: f32 = -12.0;
pub const MAX_GAIN: f32 = 12.0;

// The Margin around the EQ Area
pub const EQ_MARGIN: Vec2 = Vec2::new(25.0, 20.0);

// When attempting to interact with a dot, this is how far outside we look
pub const EQ_GRAB_THRESHOLD: f32 = 20.0;

/// Whether this band type has a meaningful gain value (as opposed to
/// filters like High/Low Pass or Notch, which are always drawn at 0dB).
pub fn band_type_has_gain(band_type: EQBandType) -> bool {
    !matches!(band_type, HighPassFilter | LowPassFilter | NotchFilter)
}

/// Pure coordinate-space math shared by both the rendering view and the
/// interaction/controls layer. None of this touches egui painting or
/// device messaging, so it's safe to reuse anywhere the EQ needs to
/// convert between screen space and frequency/gain space.
pub struct EqGeometry;

impl EqGeometry {
    /// The inner rect actually used for plotting, inset from the outer
    /// widget rect to leave room for axis labels.
    pub fn plot_rect(rect: Rect) -> Rect {
        Rect::from_min_max(rect.min + EQ_MARGIN, rect.max)
    }

    pub fn freq_to_x(freq: u32, plot_rect: Rect) -> f32 {
        let log_min = (MIN_FREQUENCY as f32).log10();
        let log_max = (MAX_FREQUENCY as f32).log10();
        let log_f = (freq as f32).log10();
        let normalized = (log_f - log_min) / (log_max - log_min);

        plot_rect.min.x + normalized * plot_rect.width()
    }

    pub fn x_to_freq(x: f32, plot_rect: Rect) -> f32 {
        let log_min = (MIN_FREQUENCY as f32).log10();
        let log_max = (MAX_FREQUENCY as f32).log10();
        let normalized = (x - plot_rect.min.x) / plot_rect.width();
        let log_f = log_min + normalized * (log_max - log_min);

        10f32.powf(log_f)
    }

    pub fn db_to_y(db: f32, plot_rect: Rect) -> f32 {
        let normalized = (MAX_GAIN - db) / (MAX_GAIN - MIN_GAIN);
        plot_rect.min.y + normalized * plot_rect.height()
    }

    pub fn y_to_db(y: f32, plot_rect: Rect) -> f32 {
        let normalized = (y - plot_rect.min.y) / plot_rect.height();
        MAX_GAIN - normalized * (MAX_GAIN - MIN_GAIN)
    }

    /// Find the band whose control point is nearest to `pointer`, within
    /// `EQ_GRAB_THRESHOLD` pixels. This is what click/drag/scroll handling
    /// in the controls layer is built on top of; the view itself never
    /// calls this.
    pub fn hit_test(plot_rect: Rect, pointer: Pos2, bands: &Bands) -> Option<EQBand> {
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

            let point = Pos2::new(x, y);
            let dist = point.distance(pointer);

            if dist < closest_dist && dist < EQ_GRAB_THRESHOLD {
                closest_dist = dist;
                closest_band = Some(band);
            }
        }
        closest_band
    }
}
