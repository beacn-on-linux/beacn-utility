use crate::pages::config_pages::equaliser::equaliser_util::{BiquadCoefficient, EQUtil};
use crate::state::{BeacnMicState, EqualiserBand};
use beacn_mic_lib::device::BeacnMic;
use beacn_mic_lib::messages::equaliser::{EQBand, EQBandType, EQMode};
use eframe::egui;
use eframe::egui::{CornerRadius, Mesh, Shape, StrokeKind, pos2, vec2};
use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use enum_map::EnumMap;
use std::sync::{Arc, LazyLock};
use strum::IntoEnumIterator;
use wide::f32x8;

type Bands = EnumMap<EQBand, EqualiserBand>;

// The frequency range to be rendered
static MIN_FREQUENCY: u32 = 20;
static MAX_FREQUENCY: u32 = 20000;

// The Acceptable Gain Range
static MIN_GAIN: f32 = -12.0;
static MAX_GAIN: f32 = 12.0;

// The Margin around the EQ Area
static EQ_MARGIN: Vec2 = Vec2::new(25.0, 20.0);

// When attempting to interact with a dot, this is how far outside we look
static EQ_GRAB_THRESHOLD: f32 = 20.0;
static EQ_POINT_RADIUS: f32 = 6.0;
static EQ_SELECTED_RADIUS: f32 = 8.0;

static EQ_COLOURS: [[u8; 3]; 4] = [
    [239, 54, 60],
    [31, 187, 185],
    [254, 201, 37],
    [255, 15, 110],
];

static EQ_TRANSPARENT_COLOURS: LazyLock<[Color32; 4]> = LazyLock::new(|| {
    [
        Color32::from_rgba_unmultiplied(EQ_COLOURS[0][0], EQ_COLOURS[0][1], EQ_COLOURS[0][2], 5),
        Color32::from_rgba_unmultiplied(EQ_COLOURS[1][0], EQ_COLOURS[1][1], EQ_COLOURS[1][2], 5),
        Color32::from_rgba_unmultiplied(EQ_COLOURS[2][0], EQ_COLOURS[2][1], EQ_COLOURS[2][2], 5),
        Color32::from_rgba_unmultiplied(EQ_COLOURS[3][0], EQ_COLOURS[3][1], EQ_COLOURS[3][2], 5),
    ]
});

static EQ_POINT_COLOURS: LazyLock<[Color32; 4]> = LazyLock::new(|| {
    [
        Color32::from_rgb(EQ_COLOURS[0][0], EQ_COLOURS[0][1], EQ_COLOURS[0][2]),
        Color32::from_rgb(EQ_COLOURS[1][0], EQ_COLOURS[1][1], EQ_COLOURS[1][2]),
        Color32::from_rgb(EQ_COLOURS[2][0], EQ_COLOURS[2][1], EQ_COLOURS[2][2]),
        Color32::from_rgb(EQ_COLOURS[3][0], EQ_COLOURS[3][1], EQ_COLOURS[3][2]),
    ]
});

/// Widget for parametric equalizer visualization
pub struct ParametricEq {
    // The current Equaliser Mode
    eq_mode: EQMode,

    // A cache of the frequency responses and drawn mesh
    band_freq_response: EnumMap<EQBand, Option<Vec<f32>>>,
    band_mesh: EnumMap<EQBand, Option<Arc<Mesh>>>,

    // Cache of the main curve, and rect size
    curve_points: Vec<Pos2>,
    rect: Rect,

    // Active bands for interactions
    active_band: EQBand,
    active_band_drag: Option<EQBand>,
}

impl<'a> ParametricEq {
    pub(crate) fn new() -> Self {
        Self {
            eq_mode: EQMode::Simple,

            band_freq_response: Default::default(),
            band_mesh: Default::default(),

            curve_points: vec![],
            rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),

            active_band: EQBand::Band1,
            active_band_drag: None,
        }
    }

    /// Shows the parametric equalizer in the UI
    pub fn ui(&mut self, ui: &mut Ui, mic: &BeacnMic, state: &mut BeacnMicState) -> Response {
        let bands = &mut state.equaliser.bands[state.equaliser.mode];
        let (rect, response) = ui.allocate_exact_size(
            vec2(ui.available_width(), ui.available_height()),
            Sense::click_and_drag(),
        );

        if self.rect != rect || self.eq_mode != state.equaliser.mode {
            // Window has been resized, or we've changed mode. Reset everything.
            self.band_mesh.clear();
            self.band_freq_response.clear();
            self.curve_points.clear();
            self.rect = rect;
        }

        if ui.is_rect_visible(rect) {
            self.draw_widget(ui, rect, bands);

            if response.hovered() {
                if let Some(pointer_pos) = response.hover_pos() {
                    let scroll = ui.ctx().input(|i| i.raw_scroll_delta).y;
                    if scroll != 0.0 {
                        let scroll_up = scroll > 0.0;
                        self.handle_scroll(rect, pointer_pos, scroll_up, bands);
                    }
                }
            }

            if response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    self.handle_click(rect, pointer_pos, bands);
                }
            }

            if response.drag_started() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    self.handle_drag_start(rect, pointer_pos, bands);
                }
            }
            if response.dragged() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    self.handle_drag(rect, pointer_pos, bands);
                }
            }
            if response.drag_stopped() {
                self.handle_drag_stop();
            }
        }

        response
    }

    /// Draws the widget - axes, grid, EQ curve and band control points
    fn draw_widget(&mut self, ui: &mut Ui, rect: Rect, bands: &mut Bands) {
        let plot_rect = Self::get_plot_rect(rect);

        // Draw grid and axes
        self.draw_grid(ui.painter(), rect, plot_rect);

        // Draw the background for the individual bands
        for (index, band) in EQBand::iter().enumerate() {
            // Only draw it if it's enabled
            if bands[band].enabled {
                let colour = EQ_TRANSPARENT_COLOURS[index % EQ_TRANSPARENT_COLOURS.len()];
                self.draw_eq_individual(ui.painter(), band, plot_rect, colour, bands);
            }
        }

        // Draw the combined EQ response curve
        self.draw_eq_curve(ui.painter(), plot_rect, bands);

        // Draw band control points
        self.draw_band_points(ui.painter(), plot_rect, bands);
    }

    /// Draw the grid and axis labels
    fn draw_grid(&self, painter: &egui::Painter, rect: Rect, plot_rect: Rect) {
        let background = Color32::from_rgb(34, 34, 34);
        let grid_color = Color32::from_rgb(102, 102, 102);
        let text_color = Color32::from_rgb(170, 170, 170);
        let grid_stroke = Stroke::new(1.0, grid_color);
        let axis_stroke = Stroke::new(2.0, Color32::from_rgb(170, 170, 170));
        let freq_ticks = [30, 50, 100, 250, 500, 1000, 2000, 5000, 10000, 16000];

        painter.rect(
            plot_rect,
            CornerRadius::default(),
            background,
            axis_stroke,
            StrokeKind::Middle,
        );
        for &freq in &freq_ticks {
            let x = Self::freq_to_x(freq, plot_rect);

            painter.line_segment(
                [Pos2::new(x, plot_rect.min.y), Pos2::new(x, plot_rect.max.y)],
                grid_stroke,
            );

            // Draw label for selected frequencies
            painter.text(
                Pos2::new(x, rect.min.y + 5.0),
                egui::Align2::CENTER_CENTER,
                freq,
                FontId::proportional(12.0),
                text_color,
            );
        }

        // Labels every 3dB
        for db in (MIN_GAIN as i32..=MAX_GAIN as i32).step_by(3) {
            let db = db as f32;
            let y = Self::db_to_y(db, plot_rect);

            // Labels for dB values
            painter.text(
                Pos2::new(plot_rect.min.x - 10.0, y),
                egui::Align2::RIGHT_CENTER,
                format!("{}", db),
                FontId::proportional(12.0),
                text_color,
            );
        }
    }

    fn draw_eq_curve(&mut self, painter: &egui::Painter, plot_rect: Rect, bands: &Bands) {
        let curve_color = Color32::from_rgb(255, 255, 255);
        let curve_stroke = Stroke::new(3.0, curve_color);
        if self.curve_points.is_empty() {
            // Ok, for each point, we need to sum the frequency gain of all the bands, and
            // then convert those values to the correct positions.
            let mut source = vec![];
            for band in EQBand::iter() {
                // Only count it if the band is enabled
                if bands[band].enabled {
                    source.push(self.get_eq_frequency_response(plot_rect, band, bands));
                }
            }

            let len = source[0].len();
            let mut result = vec![0.0; len];
            for vec in source {
                assert_eq!(vec.len(), len, "All vectors must be the same length");
                for (i, val) in vec.iter().enumerate() {
                    result[i] += val;
                }
            }

            let steps = plot_rect.width() as usize;
            let mut points = Vec::with_capacity(steps + 1);

            for (i, result) in result.iter().enumerate().take(steps + 1) {
                let x = plot_rect.min.x + i as f32;
                points.push(pos2(x, Self::db_to_y(*result, plot_rect)));
            }
            let mut adapted_points = self.adaptive_smooth_points(points, plot_rect, 8);

            // Clamp the points inside our bounds
            let min_y = plot_rect.min.y;
            let max_y = plot_rect.height() + plot_rect.min.y;
            adapted_points
                .iter_mut()
                .for_each(|f| f.y = f.y.clamp(min_y, max_y));
            self.curve_points = adapted_points;
        }
        painter.add(Shape::line(self.curve_points.clone(), curve_stroke));
    }

    fn draw_eq_individual(
        &mut self,
        painter: &egui::Painter,
        band: EQBand,
        rect: Rect,
        colour: Color32,
        bands: &Bands,
    ) {
        if let Some(mesh) = &self.band_mesh[band] {
            painter.add(Shape::mesh(mesh.clone()));
        }

        let mut curve = self.get_eq_curve_points(rect, band, bands).clone();
        let zero_db_y = ParametricEq::db_to_y(0.0, rect);

        // Remove all points which are within 0.5px of 0
        curve.retain(|p| (p.y - zero_db_y).abs() > 0.5);
        if curve.is_empty() {
            return;
        }

        let max_y = rect.min.y + rect.height();
        let min_y = rect.min.y;

        // Clamp the points inside our bounds
        curve.iter_mut().for_each(|f| f.y = f.y.clamp(min_y, max_y));

        let mut mesh = Mesh::default();
        for pair in curve.windows(2) {
            let [p1, p2] = [pair[0], pair[1]];

            let p1_base = Pos2::new(p1.x, zero_db_y);
            let p2_base = Pos2::new(p2.x, zero_db_y);

            let base_idx = mesh.vertices.len() as u32;

            mesh.colored_vertex(p1, colour);
            mesh.colored_vertex(p2, colour);
            mesh.colored_vertex(p1_base, colour);

            mesh.colored_vertex(p2, colour);
            mesh.colored_vertex(p2_base, colour);
            mesh.colored_vertex(p1_base, colour);

            mesh.indices.extend([
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx + 3,
                base_idx + 4,
                base_idx + 5,
            ]);
        }
        let mesh = Arc::new(mesh);
        self.band_mesh[band] = Some(mesh.clone());

        painter.add(Shape::mesh(mesh));
        //painter.add(Shape::line(curve, Stroke::new(1.0, Color32::WHITE)));
    }

    /// Get the EQ Curve based on band
    fn get_eq_curve_points(&mut self, rect: Rect, band: EQBand, bands: &Bands) -> Vec<Pos2> {
        let steps = rect.width() as usize;
        let mut points = Vec::with_capacity(steps + 1);

        for i in 0..=steps {
            let i = rect.min.x + i as f32;
            points.push(pos2(i, 0.0));
        }

        let gains = self.get_eq_frequency_response(rect, band, bands);
        for (i, point) in points.iter_mut().enumerate() {
            point.y = Self::db_to_y(gains[i], rect);
        }

        let points = self.adaptive_smooth_points(points, rect, 8);
        points
    }

    fn get_eq_frequency_response(&mut self, rect: Rect, band: EQBand, bands: &Bands) -> Vec<f32> {
        if let Some(frequencies) = &self.band_freq_response[band] {
            return frequencies.clone();
        }
        let steps = rect.width() as usize;
        let mut freqs: Vec<f32> = vec![];

        let offset_x = rect.min.x;
        for i in 0..=steps {
            freqs.push(Self::x_to_freq(i as f32 + offset_x, rect));
        }
        let gains = self.eq_gain_simd(freqs.as_slice(), band, bands);
        self.band_freq_response[band] = Some(gains.clone());
        gains
    }

    /// This code attempts to 'smooth' the line below 100hz
    fn adaptive_smooth_points(&self, points: Vec<Pos2>, rect: Rect, window: usize) -> Vec<Pos2> {
        // Convert cutoff frequency to X coordinate
        let cutoff_x = Self::freq_to_x(100, rect);

        let len = points.len();
        let mut smoothed = Vec::with_capacity(len);

        for i in 0..len {
            // Skip smoothing for points beyond the cutoff frequency
            if points[i].x > cutoff_x {
                smoothed.push(points[i]);
                continue;
            }

            // For smoothing, use a weighted moving average approach
            let mut sum_y = 0.0;
            let mut weight_sum = 0.0;

            // Calculate boundaries for the window, preventing out-of-bounds access
            let start = i.saturating_sub(window);
            let end = (i + window).min(len - 1);

            for (j, value) in points.iter().enumerate().take(end + 1).skip(start) {
                // Calculate a weight based on distance from the center point
                // Points closer to the center have higher weight

                let distance = (i as isize - j as isize).abs() as f32;
                let weight = 1.0 / (1.0 + distance);
                sum_y += value.y * weight;
                weight_sum += weight;
            }

            let avg_y = if weight_sum > 0.0 {
                sum_y / weight_sum
            } else {
                points[i].y
            };
            smoothed.push(Pos2::new(points[i].x, avg_y));
        }

        smoothed
    }

    /// Draw the band control points
    fn draw_band_points(&self, painter: &egui::Painter, rect: Rect, bands: &Bands) {
        for (index, (band, value)) in bands.iter().enumerate() {
            if !value.enabled {
                continue;
            }

            let colour = EQ_POINT_COLOURS[index % EQ_POINT_COLOURS.len()];

            let x = Self::freq_to_x(value.frequency, rect);
            let y = Self::db_to_y(value.gain, rect);
            painter.circle_filled(Pos2::new(x, y), EQ_POINT_RADIUS, colour);

            if band == self.active_band {
                painter.circle_stroke(
                    Pos2::new(x, y),
                    EQ_SELECTED_RADIUS,
                    Stroke::new(1.0, colour),
                );
            }
        }
    }

    fn get_point_near_cursor(
        &mut self,
        rect: Rect,
        pointer: Pos2,
        bands: &Bands,
    ) -> Option<EQBand> {
        let plot_rect = Self::get_plot_rect(rect);
        let mut closest_dist = f32::MAX;
        let mut closest_band = None;

        // Iterate over the bands
        for (band, value) in bands {
            if !value.enabled {
                continue;
            }

            // Get the X/Y position of this band (If the band doesn't support gain, assume 0db)
            let x = Self::freq_to_x(value.frequency, plot_rect);
            let y = if !Self::band_type_has_gain(value.band_type) {
                Self::db_to_y(0.0, plot_rect)
            } else {
                Self::db_to_y(value.gain, plot_rect)
            };

            // Create a point, and get the cursors distance from it
            let point = Pos2::new(x, y);
            let dist = point.distance(pointer);

            if dist < closest_dist && dist < EQ_GRAB_THRESHOLD {
                closest_dist = dist;
                closest_band = Some(band);
            }
        }
        closest_band
    }

    fn handle_click(&mut self, rect: Rect, pointer: Pos2, bands: &Bands) {
        if let Some(index) = self.get_point_near_cursor(rect, pointer, bands) {
            self.active_band = index;
        }
    }

    fn handle_drag_start(&mut self, rect: Rect, pointer: Pos2, bands: &Bands) {
        let active = self.get_point_near_cursor(rect, pointer, bands);
        if let Some(active) = active {
            self.active_band = active;
        }
        self.active_band_drag = active;
    }

    /// Handle drag interactions with the control points
    fn handle_drag(&mut self, rect: Rect, pointer_pos: Pos2, bands: &mut Bands) {
        // We don't have an active item, so there's nothing to do
        if self.active_band_drag.is_none() {
            return;
        }
        let active = self.active_band_drag.unwrap();

        let plot_rect = Self::get_plot_rect(rect);
        let band = &mut bands[active];

        let frequency = Self::x_to_freq(pointer_pos.x, plot_rect);
        let frequency = frequency.clamp(MIN_FREQUENCY as f32, MAX_FREQUENCY as f32);
        band.frequency = frequency as u32;

        // If this band supports gain, update it.
        if Self::band_type_has_gain(band.band_type) {
            let gain = Self::y_to_db(pointer_pos.y, plot_rect).clamp(MIN_GAIN, MAX_GAIN);
            band.gain = (gain * 10.0).round() / 10.0;
        }

        // Clear out the caches for this band as it needs a redraw
        self.band_freq_response[active] = None;
        self.band_mesh[active] = None;
        self.curve_points.clear();
    }

    fn handle_drag_stop(&mut self) {
        self.active_band_drag = None;
    }

    fn handle_scroll(
        &mut self,
        rect: Rect,
        pointer_position: Pos2,
        scroll_up: bool,
        bands: &mut Bands,
    ) {
        if let Some(band) = self.get_point_near_cursor(rect, pointer_position, bands) {
            self.active_band = band;

            let q = bands[self.active_band].q;
            let new_q = if scroll_up { q + 0.2 } else { q - 0.2 };
            let new = new_q.clamp(0.1, 10.0);
            let rounded = (new * 10.0).round() / 10.0;
            bands[self.active_band].q = rounded;

            // Invalidate existing renders for this band
            self.band_freq_response[band] = None;
            self.band_mesh[band] = None;
            self.curve_points.clear();
        }
    }

    /// Convert frequency to x-coordinate in plot area
    fn freq_to_x(freq: u32, plot_rect: Rect) -> f32 {
        let log_min = (MIN_FREQUENCY as f32).log10();
        let log_max = (MAX_FREQUENCY as f32).log10();
        let log_f = (freq as f32).log10();
        let normalized = (log_f - log_min) / (log_max - log_min);

        plot_rect.min.x + normalized * plot_rect.width()
    }

    fn x_to_freq(x: f32, plot_rect: Rect) -> f32 {
        let log_min = (MIN_FREQUENCY as f32).log10();
        let log_max = (MAX_FREQUENCY as f32).log10();
        let normalized = (x - plot_rect.min.x) / plot_rect.width();
        let log_f = log_min + normalized * (log_max - log_min);

        10f32.powf(log_f)
    }

    fn db_to_y(db: f32, plot_rect: Rect) -> f32 {
        let normalized = (MAX_GAIN - db) / (MAX_GAIN - MIN_GAIN);
        plot_rect.min.y + normalized * plot_rect.height()
    }

    fn y_to_db(y: f32, plot_rect: Rect) -> f32 {
        let normalized = (y - plot_rect.min.y) / plot_rect.height();
        MAX_GAIN - normalized * (MAX_GAIN - MIN_GAIN)
    }

    /// Calculate the gain for a band at a specific frequency
    fn eq_gain(&self, freq: f32, band: EQBand, bands: &Bands) -> f32 {
        let coefficient = Self::get_coefficient(&bands[band]);
        EQUtil::freq_response_scalar(freq, &coefficient)
    }
    pub fn eq_gain_simd(&self, frequencies: &[f32], band: EQBand, bands: &Bands) -> Vec<f32> {
        let mut gains = vec![0.0; frequencies.len()];
        let chunks = frequencies.chunks_exact(8);
        let remainder = chunks.remainder();

        let coefficient = Self::get_coefficient(&bands[band]);
        for i in 0..chunks.len() {
            let chunk = &frequencies[i * 8..(i + 1) * 8];
            let freq_chunk = f32x8::new(<[f32; 8]>::try_from(chunk).unwrap());

            let gain = EQUtil::freq_response_simd(freq_chunk, &coefficient);
            gains[i * 8..(i + 1) * 8].copy_from_slice(&gain.to_array());
        }

        // Handle remainder frequencies (scalar fallback)
        if !remainder.is_empty() {
            for (i, &freq) in remainder.iter().enumerate() {
                gains[chunks.len() * 8 + i] = self.eq_gain(freq, band, bands);
            }
        }
        gains
    }

    fn get_coefficient(band: &EqualiserBand) -> BiquadCoefficient {
        match band.band_type {
            EQBandType::LowShelf => {
                EQUtil::low_shelf_coefficient(band.frequency as f32, band.gain, band.q)
            }
            EQBandType::HighShelf => {
                EQUtil::high_shelf_coefficient(band.frequency as f32, band.gain, band.q)
            }
            EQBandType::BellBand => {
                EQUtil::bell_coefficient(band.frequency as f32, band.gain, band.q)
            }
            EQBandType::NotchFilter => EQUtil::notch_coefficient(band.frequency as f32, band.q),
            EQBandType::HighPassFilter => {
                EQUtil::high_pass_coefficient(band.frequency as f32, band.q)
            }
            EQBandType::LowPassFilter => {
                EQUtil::low_pass_coefficient(band.frequency as f32, band.q)
            }
            EQBandType::NotSet => panic!("We need to fix this.."),
        }
    }

    fn get_plot_rect(rect: Rect) -> Rect {
        Rect::from_min_max(
            rect.min + Vec2::new(EQ_MARGIN.x, EQ_MARGIN.y),
            rect.max - Vec2::new(EQ_MARGIN.x, EQ_MARGIN.y),
        )
    }

    fn band_type_has_gain(band_type: EQBandType) -> bool {
        !matches!(
            band_type,
            EQBandType::HighPassFilter | EQBandType::LowPassFilter | EQBandType::NotchFilter
        )
    }
}
