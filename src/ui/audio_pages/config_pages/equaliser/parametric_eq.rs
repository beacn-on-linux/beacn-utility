use crate::ui::SVG;
use crate::ui::audio_pages::config_pages::equaliser::equaliser_util::{BiquadCoefficient, EQUtil};
use crate::ui::states::audio_state::{BeacnAudioState, EqualiserBand};
use crate::ui::widgets::draw_draggable;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::equaliser::EQBandType::{
    BellBand, HighPassFilter, HighShelf, LowPassFilter, LowShelf, NotSet, NotchFilter,
};
use beacn_lib::audio::messages::equaliser::{
    EQBand, EQBandType, EQFrequency, EQGain, EQMode, EQQ, Equaliser,
};
use egui::{
    Align, Button, Color32, CornerRadius, FontId, ImageButton, Layout, Mesh, Pos2, Rect, Response,
    Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use enum_map::EnumMap;
use log::{debug, warn};
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

/// Widget for parametric equalizer visualization
pub struct ParametricEq {
    // The current serial that this EQ represents
    serial: Option<String>,

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

impl ParametricEq {
    pub(crate) fn new() -> Self {
        Self {
            serial: None,

            eq_mode: EQMode::Simple,

            band_freq_response: Default::default(),
            band_mesh: Default::default(),

            curve_points: vec![],
            rect: Rect::NOTHING,

            active_band: EQBand::Band1,
            active_band_drag: None,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.serial = None;
        self.eq_mode = EQMode::Simple;
        self.band_freq_response = Default::default();
        self.band_mesh = Default::default();
        self.curve_points.clear();
        self.rect = Rect::NOTHING;
        self.active_band = EQBand::Band1;
        self.active_band_drag = None;
    }

    /// Shows the parametric equalizer in the UI
    pub fn ui(&mut self, ui: &mut Ui, state: &mut BeacnAudioState) -> Response {
        // Are we rendering this for the current serial?
        if let Some(serial) = &self.serial {
            let serial = serial.clone();
            if serial != state.device_definition.device_info.serial {
                debug!("Resetting EQ for New Device: {serial}");
                // If the serial doesn't match, we need to reset the widget
                self.clear();
                self.serial = Some(state.device_definition.device_info.serial.clone());
            }
        } else {
            debug!(
                "Loading EQ For: {}",
                state.device_definition.device_info.serial
            );
            self.serial = Some(state.device_definition.device_info.serial.clone());
        }

        let mode = state.equaliser.mode;
        let mut bands = state.equaliser.bands[state.equaliser.mode];

        // Firstly, make sure there's at least one active band
        if bands.values().filter(|b| b.enabled).count() == 0 {
            warn!("No Active EQ Bands, Finding Band to Enable..");
            if let Some(band) = bands.values_mut().find(|t| t.band_type != NotSet) {
                band.enabled = true;
            } else {
                warn!("All bands are disabled or not set, creating a default.");
                let mode = state.equaliser.mode;

                let eq_freq_1 = EQFrequency(36.0);
                let eq_freq_2 = EQFrequency(500.0);
                let eq_freq_3 = EQFrequency(2000.0);

                let gain = EQGain(0.0);
                let q = EQQ(0.7);

                // This is basically the default setup for the 'Simple' Mode
                let messages = vec![
                    Message::Equaliser(Equaliser::Enabled(mode, EQBand::Band1, true)),
                    Message::Equaliser(Equaliser::Enabled(mode, EQBand::Band2, true)),
                    Message::Equaliser(Equaliser::Enabled(mode, EQBand::Band3, true)),
                    Message::Equaliser(Equaliser::Type(mode, EQBand::Band1, HighPassFilter)),
                    Message::Equaliser(Equaliser::Type(mode, EQBand::Band2, BellBand)),
                    Message::Equaliser(Equaliser::Type(mode, EQBand::Band3, HighShelf)),
                    Message::Equaliser(Equaliser::Frequency(mode, EQBand::Band1, eq_freq_1)),
                    Message::Equaliser(Equaliser::Frequency(mode, EQBand::Band2, eq_freq_2)),
                    Message::Equaliser(Equaliser::Frequency(mode, EQBand::Band3, eq_freq_3)),
                    Message::Equaliser(Equaliser::Gain(mode, EQBand::Band1, gain)),
                    Message::Equaliser(Equaliser::Gain(mode, EQBand::Band2, gain)),
                    Message::Equaliser(Equaliser::Gain(mode, EQBand::Band3, gain)),
                    Message::Equaliser(Equaliser::Q(mode, EQBand::Band1, q)),
                    Message::Equaliser(Equaliser::Q(mode, EQBand::Band2, q)),
                    Message::Equaliser(Equaliser::Q(mode, EQBand::Band3, q)),
                ];

                for message in messages {
                    let _ = state.handle_message(message);
                    state.set_local_value(message);
                }
            }
        }

        // Reborrow the bands, we may have made changes.
        let mut bands = state.equaliser.bands[state.equaliser.mode];

        // We'll do a quick check here to make sure our 'Active' band is actually enabled.
        if !bands[self.active_band].enabled {
            for band in EQBand::iter() {
                if bands[band].enabled {
                    self.active_band = band;
                    break;
                }
            }
        }

        let active = self.active_band;
        let (rect, response) = ui.allocate_exact_size(
            vec2(ui.available_width(), ui.available_height() - 20.0),
            Sense::click_and_drag(),
        );

        if self.rect != rect || self.eq_mode != state.equaliser.mode {
            // Window has been resized, or we've changed mode. Reset everything.
            self.eq_mode = state.equaliser.mode;
            self.band_mesh.clear();
            self.band_freq_response.clear();
            self.curve_points.clear();
            self.rect = rect;
        }

        if ui.is_rect_visible(rect) {
            self.draw_widget(ui, rect, &mut bands);
            #[allow(clippy::collapsible_if)]
            if response.hovered() {
                if let Some(pointer_pos) = response.hover_pos() {
                    let scroll = ui.ctx().input(|i| i.raw_scroll_delta).y;
                    if scroll != 0.0 {
                        let scroll_up = scroll > 0.0;
                        self.handle_scroll(rect, pointer_pos, scroll_up, &mut bands, state);
                    }
                }
            }

            #[allow(clippy::collapsible_if)]
            if response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    self.handle_click(rect, pointer_pos, &bands);
                }
            }

            #[allow(clippy::collapsible_if)]
            if response.drag_started() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    self.handle_drag_start(rect, pointer_pos, &bands);
                }
            }

            #[allow(clippy::collapsible_if)]
            if response.dragged() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    self.handle_drag(rect, pointer_pos, &mut bands, state);
                }
            }
            if response.drag_stopped() {
                self.handle_drag_stop();
            }
        }

        ui.add_space(5.0);
        let mut is_advanced = state.equaliser.mode == EQMode::Advanced;

        // We need to force a 28px height here, so that we center
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                vec2(f32::INFINITY, 26.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.add_space(20.0);

                    if ui.checkbox(&mut is_advanced, "Advanced").changed() {
                        let new_mode = if is_advanced {
                            EQMode::Advanced
                        } else {
                            EQMode::Simple
                        };
                        state.equaliser.mode = new_mode;
                        let _ = state.handle_message(Message::Equaliser(Equaliser::Mode(new_mode)));
                    }
                },
            );

            let active_band = &mut bands[active];
            if is_advanced {
                ui.separator();

                ui.allocate_ui_with_layout(
                    vec2(f32::INFINITY, 26.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.style_mut().spacing.item_spacing = vec2(1.0, 0.0);

                            for band in EQBandType::iter() {
                                if band == NotSet {
                                    continue;
                                }

                                let is_active = active_band.band_type == band;
                                let icon = match band {
                                    LowPassFilter => "eq_low_pass",
                                    HighPassFilter => "eq_high_pass",
                                    NotchFilter => "eq_notch",
                                    BellBand => "eq_bell",
                                    LowShelf => "eq_low_shelf",
                                    HighShelf => "eq_high_shelf",
                                    _ => "",
                                };
                                let position = match band {
                                    LowPassFilter => ButtonPosition::First,
                                    HighShelf => ButtonPosition::Last,
                                    _ => ButtonPosition::Middle,
                                };
                                if eq_mode(ui, icon, is_active, position).clicked() {
                                    let msg = Equaliser::Type(mode, active, band);
                                    let _ = state.handle_message(Message::Equaliser(msg));

                                    active_band.band_type = band;
                                    self.invalidate_band(active);
                                }
                            }
                        });
                    },
                );
                ui.separator();

                ui.label("Frequency: ");
                let drag = draw_draggable(&mut active_band.frequency, 20..=20000, "Hz");
                if ui.add_sized([75.0, 20.0], drag).changed() {
                    let value = EQFrequency(active_band.frequency as f32);
                    let msg = Equaliser::Frequency(mode, active, value);
                    let _ = state.handle_message(Message::Equaliser(msg));

                    self.invalidate_band(active);
                }
            }

            ui.separator();
            ui.label("Gain: ");
            let enabled = Self::band_type_has_gain(active_band.band_type);
            let mut zero = 0.0;
            let value = if enabled {
                &mut active_band.gain
            } else {
                &mut zero
            };

            let drag = draw_draggable(value, -12.0..=12.0, "dB");
            if ui
                .add_enabled(enabled, |ui: &mut Ui| ui.add_sized([75.0, 20.0], drag))
                .changed()
            {
                let value = EQGain(active_band.gain);
                let msg = Equaliser::Gain(mode, active, value);
                let _ = state.handle_message(Message::Equaliser(msg));

                self.invalidate_band(active);
            }

            if is_advanced {
                ui.separator();

                ui.label("Q: ");
                let drag = draw_draggable(&mut active_band.q, 0.1..=10.0, "");
                if ui.add_sized([75.0, 20.0], drag).changed() {
                    let value = EQQ(active_band.q);
                    let msg = Equaliser::Q(mode, active, value);
                    let _ = state.handle_message(Message::Equaliser(msg));

                    self.invalidate_band(active);
                }

                ui.separator();
                let enabled = bands.values_mut().any(|b| !b.enabled);
                let button = Button::new("Add Band");
                #[allow(clippy::collapsible_if)]
                if ui.add_enabled(enabled, button).clicked() {
                    if let Some((band, eq)) = bands.iter_mut().find(|(_, b)| !b.enabled) {
                        if eq.band_type == NotSet {
                            warn!("EQ Band doesn't have type set, defaulting to BellBand");

                            let msg = Equaliser::Type(mode, band, BellBand);
                            let _ = state.handle_message(Message::Equaliser(msg));
                            eq.band_type = BellBand;
                        }

                        let msg = Equaliser::Enabled(mode, band, true);
                        let _ = state.handle_message(Message::Equaliser(msg));

                        eq.enabled = true;
                        self.invalidate_band(band)
                    }
                }

                let enabled = bands.values().filter(|b| b.enabled).count() > 1;
                let button = Button::new("-");
                if ui.add_enabled(enabled, button).clicked() {
                    let msg = Equaliser::Enabled(mode, active, false);
                    let _ = state.handle_message(Message::Equaliser(msg));

                    bands[active].enabled = false;
                    self.invalidate_band(active);
                }
            }
        });
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
                format!("{db}"),
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
            return;
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
            // -- Render the Base Fill
            let [p1, p2] = [pair[0], pair[1]];
            let p1_base = Pos2::new(p1.x, zero_db_y);
            let p2_base = Pos2::new(p2.x, zero_db_y);

            let base_idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p1, base_colour);
            mesh.colored_vertex(p2, base_colour);
            mesh.colored_vertex(p1_base, base_colour);

            mesh.colored_vertex(p2, base_colour);
            mesh.colored_vertex(p2_base, base_colour);
            mesh.colored_vertex(p1_base, base_colour);

            mesh.indices.extend([
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx + 3,
                base_idx + 4,
                base_idx + 5,
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

        self.adaptive_smooth_points(points, rect, 8)
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
        let db0 = Self::db_to_y(0.0, rect);
        for (index, (band, value)) in bands.iter().enumerate() {
            if !value.enabled {
                continue;
            }

            let colour = EQ_POINT_COLOURS[index % EQ_POINT_COLOURS.len()];

            let x = Self::freq_to_x(value.frequency, rect);
            let y = if Self::band_type_has_gain(value.band_type) {
                Self::db_to_y(value.gain, rect)
            } else {
                db0
            };
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
    fn handle_drag(
        &mut self,
        rect: Rect,
        pointer_pos: Pos2,
        bands: &mut Bands,
        state: &mut BeacnAudioState,
    ) {
        // We don't have an active item, so there's nothing to do
        if self.active_band_drag.is_none() {
            return;
        }
        let active = self.active_band_drag.unwrap();
        let plot_rect = Self::get_plot_rect(rect);
        let band = &mut bands[active];

        if self.eq_mode != EQMode::Simple {
            // Can't change the Frequency in simple mode, only the gain.
            let frequency = Self::x_to_freq(pointer_pos.x, plot_rect);
            let frequency = frequency.clamp(MIN_FREQUENCY as f32, MAX_FREQUENCY as f32);
            band.frequency = frequency as u32;

            let value = EQFrequency(band.frequency as f32);
            let msg = Equaliser::Frequency(self.eq_mode, active, value);
            let _ = state.handle_message(Message::Equaliser(msg));
        }

        // If this band supports gain, update it.
        if Self::band_type_has_gain(band.band_type) {
            let gain = Self::y_to_db(pointer_pos.y, plot_rect).clamp(MIN_GAIN, MAX_GAIN);
            band.gain = (gain * 10.0).round() / 10.0;

            let value = EQGain(band.gain);
            let msg = Equaliser::Gain(self.eq_mode, active, value);
            let _ = state.handle_message(Message::Equaliser(msg));
        }

        // Clear out the caches for this band as it needs a redraw
        self.invalidate_band(active);
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
        state: &mut BeacnAudioState,
    ) {
        if self.eq_mode == EQMode::Simple {
            // Can't adjust the Q in simple mode
            return;
        }

        if let Some(band) = self.get_point_near_cursor(rect, pointer_position, bands) {
            self.active_band = band;

            let q = bands[self.active_band].q;
            let new_q = if scroll_up { q + 0.2 } else { q - 0.2 };
            let new = new_q.clamp(0.1, 10.0);
            let rounded = (new * 10.0).round() / 10.0;
            bands[self.active_band].q = rounded;

            let msg = Equaliser::Q(self.eq_mode, band, EQQ(rounded));
            let _ = state.handle_message(Message::Equaliser(msg));

            // Invalidate existing renders for this band
            self.invalidate_band(band);
        }
    }

    fn invalidate_band(&mut self, band: EQBand) {
        self.band_freq_response[band] = None;
        self.band_mesh[band] = None;
        self.curve_points.clear();
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
            LowShelf => EQUtil::low_shelf_coefficient(band.frequency as f32, band.gain, band.q),
            HighShelf => EQUtil::high_shelf_coefficient(band.frequency as f32, band.gain, band.q),
            BellBand => EQUtil::bell_coefficient(band.frequency as f32, band.gain, band.q),
            NotchFilter => EQUtil::notch_coefficient(band.frequency as f32, band.q),
            HighPassFilter => EQUtil::high_pass_coefficient(band.frequency as f32, band.q),
            LowPassFilter => EQUtil::low_pass_coefficient(band.frequency as f32, band.q),
            NotSet => panic!("We need to fix this.."),
        }
    }

    fn get_plot_rect(rect: Rect) -> Rect {
        Rect::from_min_max(
            rect.min + Vec2::new(EQ_MARGIN.x, EQ_MARGIN.y),
            rect.max - Vec2::new(0.0, 0.0),
        )
    }

    fn band_type_has_gain(band_type: EQBandType) -> bool {
        !matches!(band_type, HighPassFilter | LowPassFilter | NotchFilter)
    }
}

pub enum ButtonPosition {
    First,
    Middle,
    Last,
}

pub fn eq_mode(ui: &mut Ui, img: &str, active: bool, pos: ButtonPosition) -> Response {
    let image = SVG.get(img).unwrap().clone();

    let tint_colour = if active {
        Color32::WHITE
    } else {
        Color32::from_rgb(120, 120, 120)
    };

    let corner_radius = match pos {
        ButtonPosition::First => CornerRadius {
            nw: 6,
            ne: 0,
            sw: 6,
            se: 0,
        },
        ButtonPosition::Middle => CornerRadius::ZERO,
        ButtonPosition::Last => CornerRadius {
            nw: 0,
            ne: 6,
            sw: 0,
            se: 6,
        },
    };

    ui.scope(|ui| {
        ui.add(
            ImageButton::new(image)
                .corner_radius(corner_radius)
                .tint(tint_colour)
                .selected(active),
        )
    })
    .inner
}
