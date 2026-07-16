use crate::ui::audio_pages::equaliser::eq_common::{
    Bands, EqGeometry, MAX_GAIN, MIN_GAIN, band_type_has_gain,
};
use crate::ui::audio_pages::equaliser::eq_util::{BiquadCoefficient, EQUtil};
use crate::ui::states::audio_state::EqualiserBandType::*;
use crate::ui::states::audio_state::{EqualiserBand, EqualiserBandConfig};
use egui::{
    Color32, CornerRadius, FontId, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind,
    Ui, Vec2, pos2, vec2,
};
use enum_map::EnumMap;
use std::sync::{Arc, LazyLock};
use strum::IntoEnumIterator;
use wide::f32x8;

// The number of points to actually use in the curves
const EQ_CURVE_RESOLUTION: usize = 512;

const EQ_POINT_RADIUS: f32 = 6.0;
const EQ_SELECTED_RADIUS: f32 = 8.0;

const EQ_COLOURS: [[u8; 3]; 4] = [
    [239, 54, 60],
    [31, 187, 185],
    [254, 201, 37],
    [255, 15, 110],
];

static EQ_TRANSPARENT_COLOURS: LazyLock<[Color32; 4]> = LazyLock::new(|| {
    [
        Color32::from_rgba_unmultiplied(EQ_COLOURS[0][0], EQ_COLOURS[0][1], EQ_COLOURS[0][2], 128),
        Color32::from_rgba_unmultiplied(EQ_COLOURS[1][0], EQ_COLOURS[1][1], EQ_COLOURS[1][2], 128),
        Color32::from_rgba_unmultiplied(EQ_COLOURS[2][0], EQ_COLOURS[2][1], EQ_COLOURS[2][2], 128),
        Color32::from_rgba_unmultiplied(EQ_COLOURS[3][0], EQ_COLOURS[3][1], EQ_COLOURS[3][2], 128),
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

/// What `EqCurveView::ui` hands back after drawing: the raw interaction
/// `Response` plus the rects it used, so a caller can hit-test / convert
/// coordinates without the view needing to know anything about messaging
/// or app state.
pub struct EqViewOutput {
    pub response: Response,
    pub plot_rect: Rect,
}

/// Pure widget for parametric equalizer visualization.
///
/// This only ever reads `Bands` and draws. It does not send messages, does
/// not mutate band data, and does not decide what a click or drag means —
/// that's the job of whatever owns it (see `eq_controls::ParametricEq`).
pub struct EqDrawView {
    // A cache of the frequency responses and drawn mesh
    band_freq_response: EnumMap<EqualiserBand, Option<Vec<f32>>>,
    band_mesh: EnumMap<EqualiserBand, Option<Arc<Mesh>>>,

    // Cache of the main curve, and rect size (used to know when to
    // invalidate the caches above on resize)
    curve_mesh: Option<Arc<Mesh>>,
    rect: Rect,
}

impl EqDrawView {
    pub fn new() -> Self {
        Self {
            band_freq_response: Default::default(),
            band_mesh: Default::default(),
            curve_mesh: None,
            rect: Rect::NOTHING,
        }
    }

    /// Full reset — use when switching to a completely different device /
    /// context.
    pub fn clear(&mut self) {
        self.band_freq_response = Default::default();
        self.band_mesh = Default::default();
        self.curve_mesh = None;
        self.rect = Rect::NOTHING;
    }

    /// Drop cached geometry for every band (e.g. after switching between
    /// Simple/Advanced mode, where the underlying band data changes
    /// wholesale) without forgetting the current layout rect.
    pub fn invalidate_all(&mut self) {
        self.band_freq_response = Default::default();
        self.band_mesh = Default::default();
        self.curve_mesh = None;
    }

    /// Drop cached geometry for a single band, e.g. after its frequency,
    /// gain, Q or type has changed.
    pub fn invalidate_band(&mut self, band: EqualiserBand) {
        self.band_freq_response[band] = None;
        self.band_mesh[band] = None;
        self.curve_mesh = None;
    }

    /// Draw the EQ curve into `desired_size` of space. `active_band` is
    /// only used to draw the selection ring — the view has no concept of
    /// "current selection" itself, that lives with the caller.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        desired_size: Vec2,
        bands: &Bands,
        active_band: Option<EqualiserBand>,
    ) -> EqViewOutput {
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        if self.rect != rect {
            self.rect = rect;
            self.invalidate_all();
        }

        let plot_rect = EqGeometry::plot_rect(rect);

        if ui.is_rect_visible(rect) {
            self.draw_widget(ui, rect, plot_rect, bands, active_band);
        }

        EqViewOutput {
            response,
            plot_rect,
        }
    }

    /// Draws the widget - axes, grid, EQ curve and band control points
    fn draw_widget(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        plot_rect: Rect,
        bands: &Bands,
        active_band: Option<EqualiserBand>,
    ) {
        // Draw grid and axes
        self.draw_grid(ui.painter(), rect, plot_rect);

        // Draw the background for the individual bands
        for (index, band) in EqualiserBand::iter().enumerate() {
            // Only draw it if it's enabled
            if bands[band].enabled {
                let colour = EQ_TRANSPARENT_COLOURS[index % EQ_TRANSPARENT_COLOURS.len()];
                self.draw_eq_individual(ui.painter(), band, plot_rect, colour, bands);
            }
        }

        // Draw the combined EQ response curve
        self.draw_eq_curve(ui.painter(), plot_rect, bands);

        // Draw band control points
        self.draw_band_points(ui.painter(), plot_rect, bands, active_band);
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
            let x = EqGeometry::freq_to_x(freq, plot_rect);

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
            let y = EqGeometry::db_to_y(db, plot_rect);

            // Labels for dB values
            painter.text(
                Pos2::new(plot_rect.min.x - 10.0, y),
                egui::Align2::RIGHT_CENTER,
                format!("{db}"),
                FontId::proportional(12.0),
                text_color,
            );
        }
    }

    fn draw_eq_curve(&mut self, painter: &egui::Painter, plot_rect: Rect, bands: &Bands) {
        if let Some(mesh) = &self.curve_mesh {
            painter.add(Shape::mesh(mesh.clone()));
            return;
        }

        let curve_color = Color32::from_rgb(255, 255, 255);

        let sources: Vec<Vec<f32>> = EqualiserBand::iter()
            .filter(|&band| bands[band].enabled)
            .map(|band| self.get_eq_frequency_response(plot_rect, band, bands, EQ_CURVE_RESOLUTION))
            .collect();

        let summed: Vec<f32> = if sources.is_empty() {
            vec![0.0; EQ_CURVE_RESOLUTION + 1]
        } else {
            let mut result = vec![0.0; sources[0].len()];
            for vec in &sources {
                for (r, v) in result.iter_mut().zip(vec) {
                    *r += v;
                }
            }
            result
        };

        let steps = summed.len() - 1;
        let points: Vec<Pos2> = summed
            .iter()
            .enumerate()
            .map(|(i, &db)| {
                let x = plot_rect.min.x + (i as f32 / steps as f32) * plot_rect.width();
                let y = EqGeometry::db_to_y(db, plot_rect).clamp(plot_rect.min.y, plot_rect.max.y);
                pos2(x, y)
            })
            .collect();

        let points = Self::adaptive_smooth_points(points, plot_rect, 8);
        let mesh = Arc::new(Self::build_curve_mesh(&points, 3.0, curve_color));
        painter.add(Shape::mesh(mesh.clone()));
        self.curve_mesh = Some(mesh);
    }

    fn build_curve_mesh(points: &[Pos2], stroke_width: f32, color: Color32) -> Mesh {
        let mut mesh = Mesh::default();
        let half = stroke_width / 2.0;

        for pair in points.windows(2) {
            let [p1, p2] = [pair[0], pair[1]];
            let dir = (p2 - p1).normalized();
            let normal = vec2(-dir.y, dir.x);
            let offset = normal * half;

            let base_idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p1 + offset, color); // 0: top-left
            mesh.colored_vertex(p1 - offset, color); // 1: bottom-left
            mesh.colored_vertex(p2 + offset, color); // 2: top-right
            mesh.colored_vertex(p2 - offset, color); // 3: bottom-right
            mesh.indices.extend([
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx + 1,
                base_idx + 3,
                base_idx + 2,
            ]);
        }

        mesh
    }

    fn draw_eq_individual(
        &mut self,
        painter: &egui::Painter,
        band: EqualiserBand,
        rect: Rect,
        colour: Color32,
        bands: &Bands,
    ) {
        if let Some(mesh) = &self.band_mesh[band] {
            painter.add(Shape::mesh(mesh.clone()));
            return;
        }

        let mut curve = self
            .get_eq_curve_points(rect, band, bands, EQ_CURVE_RESOLUTION)
            .clone();
        let zero_db_y = EqGeometry::db_to_y(0.0, rect);

        // Remove all points which are within 0.5px of 0
        curve.retain(|p| (p.y - zero_db_y).abs() > 0.5);
        if curve.is_empty() {
            return;
        }

        let max_y = rect.min.y + rect.height();
        let min_y = rect.min.y;

        // Clamp the points inside our bounds
        curve.iter_mut().for_each(|f| f.y = f.y.clamp(min_y, max_y));

        // Prune values where Y hasn't changed
        let curve = Self::prune_flat_points(&curve, 1.0);
        let mut mesh = Mesh::default();

        let feather_width = 1.0;
        let premultiply = |c: Color32, alpha_factor: f32| -> Color32 {
            let a = (c.a() as f32 * alpha_factor).round() as u8;
            let alpha_norm = a as f32 / 255.0;
            let r = (c.r() as f32 * alpha_norm).round() as u8;
            let g = (c.g() as f32 * alpha_norm).round() as u8;
            let b = (c.b() as f32 * alpha_norm).round() as u8;
            Color32::from_rgba_premultiplied(r, g, b, a)
        };

        let base_colour = premultiply(colour, 1.0);
        let feather_colour = premultiply(colour, 0.0);

        for pair in curve.windows(2) {
            let [p1, p2] = [pair[0], pair[1]];
            let p1_base = Pos2::new(p1.x, zero_db_y);
            let p2_base = Pos2::new(p2.x, zero_db_y);

            let base_idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p1, base_colour);
            mesh.colored_vertex(p2, base_colour);
            mesh.colored_vertex(p1_base, base_colour);
            mesh.colored_vertex(p2_base, base_colour);

            mesh.indices.extend([
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx + 1,
                base_idx + 3,
                base_idx + 2,
            ]);

            // -- Render the Feather
            let direction = p2 - p1;
            let perpendicular = vec2(-direction.y, direction.x).normalized();

            // Work out if we're going positive or negative from the 0dB point
            let sign = if p1.y < zero_db_y && p2.y < zero_db_y {
                -1.0
            } else {
                1.0
            };
            let feather_vec = perpendicular * feather_width * sign;

            let p1_outer = p1 + feather_vec;
            let p2_outer = p2 + feather_vec;

            let base_idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p1, base_colour);
            mesh.colored_vertex(p2, base_colour);
            mesh.colored_vertex(p1_outer, feather_colour);

            mesh.colored_vertex(p2, base_colour);
            mesh.colored_vertex(p2_outer, feather_colour);
            mesh.colored_vertex(p1_outer, feather_colour);

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
    }

    fn prune_flat_points(points: &[Pos2], threshold: f32) -> Vec<Pos2> {
        let mut result = Vec::with_capacity(points.len());
        let mut last_y = f32::NAN;

        // Push the first point onto the stack
        if let Some(&first) = points.first() {
            result.push(first);
            last_y = first.y;
        }

        // Iterate through the remaining points
        for &point in &points[1..] {
            if (point.y - last_y).abs() >= threshold {
                result.push(point);
                last_y = point.y;
            }
        }

        // Push the last point if it's not there already
        #[allow(clippy::collapsible_if)]
        if result.last() != points.last() {
            if let Some(&last) = points.last() {
                result.push(last)
            }
        }
        result
    }

    /// Get the EQ Curve based on band
    fn get_eq_curve_points(
        &mut self,
        rect: Rect,
        band: EqualiserBand,
        bands: &Bands,
        steps: usize,
    ) -> Vec<Pos2> {
        let gains = self.get_eq_frequency_response(rect, band, bands, steps);
        let points = gains
            .iter()
            .enumerate()
            .map(|(i, &db)| {
                let x = rect.min.x + (i as f32 / steps as f32) * rect.width();
                pos2(x, EqGeometry::db_to_y(db, rect))
            })
            .collect();

        Self::adaptive_smooth_points(points, rect, 8)
    }

    fn get_eq_frequency_response(
        &mut self,
        rect: Rect,
        band: EqualiserBand,
        bands: &Bands,
        steps: usize,
    ) -> Vec<f32> {
        if let Some(frequencies) = &self.band_freq_response[band] {
            return frequencies.clone();
        }

        let freqs: Vec<f32> = (0..=steps)
            .map(|i| {
                let x = rect.min.x + (i as f32 / steps as f32) * rect.width();
                EqGeometry::x_to_freq(x, rect)
            })
            .collect();

        let gains = Self::eq_gain_simd(freqs.as_slice(), band, bands);
        self.band_freq_response[band] = Some(gains.clone());
        gains
    }

    /// This code attempts to 'smooth' the line below 100hz
    fn adaptive_smooth_points(points: Vec<Pos2>, rect: Rect, window: usize) -> Vec<Pos2> {
        // Convert cutoff frequency to X coordinate
        let cutoff_x = EqGeometry::freq_to_x(100, rect);

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
    fn draw_band_points(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        bands: &Bands,
        active_band: Option<EqualiserBand>,
    ) {
        let db0 = EqGeometry::db_to_y(0.0, rect);
        for (index, (band, value)) in bands.iter().enumerate() {
            if !value.enabled {
                continue;
            }

            let colour = EQ_POINT_COLOURS[index % EQ_POINT_COLOURS.len()];

            let x = EqGeometry::freq_to_x(value.frequency, rect);
            let y = if band_type_has_gain(value.band_type) {
                EqGeometry::db_to_y(value.gain, rect)
            } else {
                db0
            };
            painter.circle_filled(Pos2::new(x, y), EQ_POINT_RADIUS, colour);

            if Some(band) == active_band {
                painter.circle_stroke(
                    Pos2::new(x, y),
                    EQ_SELECTED_RADIUS,
                    Stroke::new(1.0, colour),
                );
            }
        }
    }

    /// Calculate the gain for a band at a specific frequency
    fn eq_gain(freq: f32, band: EqualiserBand, bands: &Bands) -> f32 {
        let coefficient = Self::get_coefficient(&bands[band]);
        EQUtil::freq_response_scalar(freq, &coefficient)
    }

    fn eq_gain_simd(frequencies: &[f32], band: EqualiserBand, bands: &Bands) -> Vec<f32> {
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
                gains[chunks.len() * 8 + i] = Self::eq_gain(freq, band, bands);
            }
        }
        gains
    }

    fn get_coefficient(band: &EqualiserBandConfig) -> BiquadCoefficient {
        match band.band_type {
            LowShelf => EQUtil::low_shelf_coefficient(band.frequency as f32, band.gain, band.q),
            HighShelf => EQUtil::high_shelf_coefficient(band.frequency as f32, band.gain, band.q),
            BellBand => EQUtil::bell_coefficient(band.frequency as f32, band.gain, band.q),
            NotchFilter => EQUtil::notch_coefficient(band.frequency as f32, band.q),
            HighPassFilter => EQUtil::high_pass_coefficient(band.frequency as f32, band.q),
            LowPassFilter => EQUtil::low_pass_coefficient(band.frequency as f32, band.q),
            NotSet => panic!("We need to fix this.."),
        }
    }
}

impl Default for EqDrawView {
    fn default() -> Self {
        Self::new()
    }
}
