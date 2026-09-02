use crate::devices::states::audio::EqualiserBandConfig;
use beacn_lib::audio::messages::eq_common::EQBand;
use beacn_lib::audio::messages::eq_common::EQBandType::*;

use crate::ui::widgets::equaliser::eq_common::{
    Bands, EqGeometry, MAX_GAIN, MIN_GAIN, band_type_has_gain,
};
use crate::ui::widgets::equaliser::eq_util::{BiquadCoefficient, EQUtil};
use enum_map::EnumMap;
use iced::alignment::Vertical;
use iced::mouse;
use iced::mouse::ScrollDelta;
use iced::widget::Action;
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke};
use iced::widget::text::Alignment;
use iced::{Color, Event, Pixels, Point, Rectangle, Renderer, Theme};
use log::debug;
use std::cell::{Cell, RefCell};
use strum::IntoEnumIterator;
use wide::f32x8;

// The width of the Plot Border
const EQ_PLOT_BORDER_WIDTH: f32 = 2.0;

// The number of points to actually use in the curves
const EQ_CURVE_RESOLUTION: usize = 512;

const EQ_POINT_RADIUS: f32 = 6.0;
const EQ_SELECTED_RADIUS: f32 = 8.0;

// How tightly the curve hugs lines between samples
const CURVE_TENSION: f32 = 1.0;
const CURVE_STROKE_WIDTH: f32 = 3.0;

const EQ_COLOURS: [[u8; 3]; 4] = [
    [239, 54, 60],
    [31, 187, 185],
    [254, 201, 37],
    [255, 15, 110],
];

fn eq_transparent_colour(index: usize) -> Color {
    let [r, g, b] = EQ_COLOURS[index % EQ_COLOURS.len()];
    Color::from_rgba8(r, g, b, 128.0 / 255.0)
}

fn eq_point_colour(index: usize) -> Color {
    let [r, g, b] = EQ_COLOURS[index % EQ_COLOURS.len()];
    Color::from_rgb8(r, g, b)
}

/// Mouse events for the EQ widget
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EQMouseEvent {
    /// Left mouse button pressed, at this position (local to the widget).
    Pressed(Point),

    /// Cursor moved to this point (while movement reporting enabled)
    Moved(Point),

    /// Left Mouse Button Released
    Released,

    /// Mouse wheel Scrolled
    Scrolled(Point, ScrollDelta),
}

// The main widget
pub struct EQDrawView {
    bands: Bands,
    active: Option<EQBand>,
    border_colour: Option<Color>,

    // Flag to indicate whether we should send move events
    track_motion: bool,

    // Size of the widget, used for external hit detection
    bounds: Cell<Rectangle>,

    // Caches of the individual band curves / fill geometry
    band_caches: EnumMap<EQBand, Cache>,

    // Cache of the grid, the main curve, and the spectrum
    grid_cache: Cache,
    curve_cache: Cache,
    spectrum_cache: Cache,

    // Frequency response cache, so we can avoid regenerating when one changes
    band_freq_response: RefCell<EnumMap<EQBand, Option<Vec<f32>>>>,

    // Spectrum Data points
    spectrum_bins: Vec<f32>,
}

impl Default for EQDrawView {
    fn default() -> Self {
        Self::new(Bands::default())
    }
}

impl EQDrawView {
    pub fn new(bands: Bands) -> Self {
        Self {
            bands,
            active: None,
            border_colour: None,
            track_motion: false,
            bounds: Cell::new(Rectangle::new(Point::ORIGIN, iced::Size::ZERO)),
            grid_cache: Cache::new(),
            band_caches: Default::default(),
            curve_cache: Cache::new(),
            spectrum_cache: Cache::new(),
            band_freq_response: RefCell::new(Default::default()),

            spectrum_bins: vec![],
        }
    }

    /// The full Boundary
    #[allow(unused)]
    pub fn bounds(&self) -> Rectangle {
        self.bounds.get()
    }

    /// The Plot Area (for hit detection)
    pub fn plot_rect(&self) -> Rectangle {
        let local = Rectangle::new(Point::ORIGIN, self.bounds.get().size());
        EqGeometry::plot_rect(local)
    }

    /// List of the Bands
    #[allow(unused)]
    pub fn bands(&self) -> &Bands {
        &self.bands
    }

    /// Replace entire bandset at once
    pub fn set_bands(&mut self, bands: Bands) {
        self.bands = bands;
        self.invalidate_all();
    }

    /// Replace a single band's data
    #[allow(unused)]
    pub fn set_band(&mut self, band: EQBand, config: EqualiserBandConfig) {
        self.bands[band] = config;
        self.invalidate_band(band);
    }

    /// Draws the ring around a band dot
    pub fn set_active(&mut self, active: Option<EQBand>) {
        debug!("Setting Active to {:?}", active);
        self.active = active;
    }

    /// Sets the border colour of the grid
    #[allow(unused)]
    pub fn set_border_colour(&mut self, colour: Option<Color>) {
        if self.border_colour != colour {
            self.border_colour = colour;
            self.grid_cache.clear();
        }
    }

    pub fn set_spectrum(&mut self, data: Vec<f32>) {
        self.spectrum_bins = data;
        self.spectrum_cache.clear();
    }

    pub fn clear_spectrum(&mut self) {
        self.spectrum_bins = vec![];
        self.spectrum_cache.clear();
    }

    /// Set's whether we should emit movement events
    pub fn set_track_motion(&mut self, track: bool) {
        self.track_motion = track;
    }

    /// Full clear and reset
    pub fn clear(&mut self) {
        self.grid_cache.clear();
        self.clear_spectrum();
        self.invalidate_all();
    }

    /// Drop all band geometry caches (for example, after a mode change)
    pub fn invalidate_all(&mut self) {
        self.band_caches = Default::default();
        self.curve_cache.clear();
        *self.band_freq_response.borrow_mut() = Default::default();
    }

    /// Drop cached geometry for a single band
    pub fn invalidate_band(&mut self, band: EQBand) {
        self.band_freq_response.borrow_mut()[band] = None;
        self.band_caches[band].clear();
        self.curve_cache.clear();
    }

    // Draw the background grid
    fn draw_grid(&self, frame: &mut Frame, rect: Rectangle, plot_rect: Rectangle) {
        let axis_stroke_colour = self
            .border_colour
            .unwrap_or(Color::from_rgb8(170, 170, 170));

        let background = Color::from_rgb8(34, 34, 34);
        let grid_colour = Color::from_rgb8(102, 102, 102);
        let text_colour = Color::from_rgb8(170, 170, 170);
        let freq_ticks = [30, 50, 100, 250, 500, 1000, 2000, 5000, 10000, 16000];

        frame.fill_rectangle(plot_rect.position(), plot_rect.size(), background);

        let half = EQ_PLOT_BORDER_WIDTH / 2.0;
        let border_rect = Rectangle::new(
            Point::new(plot_rect.x + half, plot_rect.y + half),
            iced::Size::new(
                plot_rect.width - EQ_PLOT_BORDER_WIDTH,
                plot_rect.height - EQ_PLOT_BORDER_WIDTH,
            ),
        );
        frame.stroke(
            &Path::rectangle(border_rect.position(), border_rect.size()),
            Stroke::default()
                .with_color(axis_stroke_colour)
                .with_width(EQ_PLOT_BORDER_WIDTH),
        );

        for &freq in &freq_ticks {
            let x = EqGeometry::freq_to_x(freq, plot_rect);

            frame.stroke(
                &Path::line(
                    Point::new(x, plot_rect.y + EQ_PLOT_BORDER_WIDTH),
                    Point::new(x, plot_rect.y + plot_rect.height - EQ_PLOT_BORDER_WIDTH),
                ),
                Stroke::default().with_color(grid_colour).with_width(1.0),
            );

            frame.fill_text(canvas::Text {
                content: freq.to_string(),
                position: Point::new(x, rect.y + 5.0),
                color: text_colour,
                size: Pixels(12.0),
                align_x: Alignment::Center,
                align_y: Vertical::Center,
                ..canvas::Text::default()
            });
        }

        // Labels every 3dB
        let mut db = MIN_GAIN as i32;
        while db <= MAX_GAIN as i32 {
            let y = EqGeometry::db_to_y(db as f32, plot_rect);
            frame.fill_text(canvas::Text {
                content: format!("{db}"),
                position: Point::new(plot_rect.x - 4.0, y),
                color: text_colour,
                size: Pixels(12.0),
                align_x: Alignment::Right,
                align_y: Vertical::Center,
                ..canvas::Text::default()
            });

            db += 3;
        }
    }

    fn draw_eq_curve(&self, frame: &mut Frame, plot_rect: Rectangle) {
        let curve_colour = Color::WHITE;

        let sources: Vec<Vec<f32>> = EQBand::iter()
            .filter(|&band| self.bands[band].enabled)
            .map(|band| self.get_eq_frequency_response(plot_rect, band, EQ_CURVE_RESOLUTION))
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
        let points: Vec<Point> = summed
            .iter()
            .enumerate()
            .map(|(i, &db)| {
                let x = plot_rect.x + (i as f32 / steps as f32) * plot_rect.width;
                let y = EqGeometry::db_to_y(db, plot_rect)
                    .clamp(plot_rect.y, plot_rect.y + plot_rect.height);
                Point::new(x, y)
            })
            .collect();
        let points = Self::adaptive_smooth_points(points, plot_rect, 8);

        let path = build_catmull_rom_stroke_path(
            &points,
            CURVE_TENSION,
            plot_rect.y,
            plot_rect.y + plot_rect.height,
        );
        frame.stroke(
            &path,
            Stroke::default()
                .with_color(curve_colour)
                .with_width(CURVE_STROKE_WIDTH),
        );
    }

    fn draw_eq_individual(
        &self,
        frame: &mut Frame,
        band: EQBand,
        plot_rect: Rectangle,
        colour: Color,
    ) {
        let gains = self.get_eq_frequency_response(plot_rect, band, EQ_CURVE_RESOLUTION);
        let steps = gains.len() - 1;

        let points: Vec<Point> = gains
            .iter()
            .enumerate()
            .map(|(i, &db)| {
                let x = plot_rect.x + (i as f32 / steps as f32) * plot_rect.width;
                let y = EqGeometry::db_to_y(db, plot_rect)
                    .clamp(plot_rect.y, plot_rect.y + plot_rect.height);
                Point::new(x, y)
            })
            .collect();
        let points = Self::adaptive_smooth_points(points, plot_rect, 8);

        let zero_db_y = EqGeometry::db_to_y(0.0, plot_rect);
        let path = build_catmull_rom_fill_path(
            &points,
            zero_db_y,
            CURVE_TENSION,
            plot_rect.y,
            plot_rect.y + plot_rect.height,
        );
        frame.fill(&path, colour);
    }

    fn draw_spectrum(&self, frame: &mut Frame, plot_rect: Rectangle) {
        let bins = &self.spectrum_bins;
        let colour = Color::from_rgba8(180, 180, 180, 0.5);

        let points: Vec<Point> = bins
            .iter()
            .enumerate()
            .filter_map(|(i, &db)| {
                if !db.is_finite() {
                    return None;
                }

                let spectrum_floor = -120.0_f32;
                let spectrum_ceil = 0.0_f32;

                let db = db.clamp(spectrum_floor, spectrum_ceil);

                // normalize 0..1
                let t = (db - spectrum_floor) / (spectrum_ceil - spectrum_floor);

                // map into EQ display range (-12..+12)
                let mapped_db = MIN_GAIN + t * (MAX_GAIN - MIN_GAIN);

                let x = plot_rect.x + (i as f32 / (bins.len() - 1) as f32) * plot_rect.width;

                let mut plot_rect = plot_rect;
                plot_rect.height -= EQ_PLOT_BORDER_WIDTH / 2.0;

                let y = EqGeometry::db_to_y(mapped_db, plot_rect);

                Some(Point { x, y })
            })
            .collect();

        if points.len() < 2 {
            return;
        }

        let path = Path::new(|builder| {
            if let Some(&first) = points.first() {
                builder.move_to(first);
                for &p in &points[1..] {
                    builder.line_to(p);
                }
            }
        });
        frame.stroke(&path, Stroke::default().with_color(colour).with_width(1.50));
    }

    fn adaptive_smooth_points(
        points: Vec<Point>,
        plot_rect: Rectangle,
        window: usize,
    ) -> Vec<Point> {
        let cutoff_x = EqGeometry::freq_to_x(100, plot_rect);

        let len = points.len();
        let mut smoothed = Vec::with_capacity(len);

        for i in 0..len {
            if points[i].x > cutoff_x {
                smoothed.push(points[i]);
                continue;
            }

            let mut sum_y = 0.0;
            let mut weight_sum = 0.0;

            let start = i.saturating_sub(window);
            let end = (i + window).min(len - 1);

            for (j, value) in points.iter().enumerate().take(end + 1).skip(start) {
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
            smoothed.push(Point::new(points[i].x, avg_y));
        }

        smoothed
    }

    /// Draw the band control points and the selection ring for `active`.
    fn draw_band_points(&self, frame: &mut Frame, plot_rect: Rectangle) {
        let db0 = EqGeometry::db_to_y(0.0, plot_rect);
        for (index, (band, value)) in self.bands.iter().enumerate() {
            if !value.enabled {
                continue;
            }

            let colour = eq_point_colour(index);

            let x = EqGeometry::freq_to_x(value.frequency, plot_rect);
            let y = if band_type_has_gain(value.band_type) {
                EqGeometry::db_to_y(value.gain, plot_rect)
            } else {
                db0
            };
            let position = Point::new(x, y);

            frame.fill(&Path::circle(position, EQ_POINT_RADIUS), colour);

            if Some(band) == self.active {
                frame.stroke(
                    &Path::circle(position, EQ_SELECTED_RADIUS),
                    Stroke::default().with_color(colour).with_width(1.0),
                );
            }
        }
    }

    fn get_eq_frequency_response(
        &self,
        plot_rect: Rectangle,
        band: EQBand,
        steps: usize,
    ) -> Vec<f32> {
        if let Some(frequencies) = &self.band_freq_response.borrow()[band] {
            return frequencies.clone();
        }

        let freqs: Vec<f32> = (0..=steps)
            .map(|i| {
                let x = plot_rect.x + (i as f32 / steps as f32) * plot_rect.width;
                EqGeometry::x_to_freq(x, plot_rect)
            })
            .collect();

        let gains = Self::eq_gain_simd(freqs.as_slice(), band, &self.bands);
        self.band_freq_response.borrow_mut()[band] = Some(gains.clone());
        gains
    }

    /// Calculate the gain for a band at a specific frequency
    fn eq_gain(freq: f32, band: EQBand, bands: &Bands) -> f32 {
        let coefficient = Self::get_coefficient(&bands[band]);
        EQUtil::freq_response_scalar(freq, &coefficient)
    }

    fn eq_gain_simd(frequencies: &[f32], band: EQBand, bands: &Bands) -> Vec<f32> {
        let mut gains = vec![0.0; frequencies.len()];
        // let chunks = frequencies.chunks_exact(8);
        // let remainder = chunks.remainder();
        let chunks = frequencies.as_chunks::<8>();
        let remainder = chunks.1;


        let coefficient = Self::get_coefficient(&bands[band]);
        for i in 0..chunks.0.len() {
            let chunk = &frequencies[i * 8..(i + 1) * 8];
            let freq_chunk = f32x8::new(<[f32; 8]>::try_from(chunk).unwrap());

            let gain = EQUtil::freq_response_simd(freq_chunk, &coefficient);
            gains[i * 8..(i + 1) * 8].copy_from_slice(&gain.to_array());
        }

        if !remainder.is_empty() {
            for (i, &freq) in remainder.iter().enumerate() {
                gains[chunks.0.len() * 8 + i] = Self::eq_gain(freq, band, bands);
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

impl canvas::Program<EQMouseEvent> for EQDrawView {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<EQMouseEvent>> {
        let local_position = |cursor: mouse::Cursor| -> Option<Point> {
            cursor
                .position()
                .map(|p| Point::new(p.x - bounds.x, p.y - bounds.y))
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if !cursor.is_over(bounds) {
                    return None;
                }
                let position = local_position(cursor)?;
                Some(Action::publish(EQMouseEvent::Pressed(position)).and_capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if !self.track_motion {
                    return None;
                }
                let position = local_position(cursor)?;
                Some(Action::publish(EQMouseEvent::Moved(position)))
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Action::publish(EQMouseEvent::Released))
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if !cursor.is_over(bounds) {
                    return None;
                }
                let position = local_position(cursor)?;
                Some(Action::publish(EQMouseEvent::Scrolled(position, *delta)).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // Frame-local coordinate space: (0,0) is this widget's top-left,
        // matching what `Frame::new(renderer, bounds.size())` gives us.
        let local_rect = Rectangle::new(Point::ORIGIN, bounds.size());
        let plot_rect = EqGeometry::plot_rect(local_rect);

        // The geometry caches below invalidate themselves automatically
        // on resize; `band_freq_response` is a plain cache and doesn't,
        // so it gets its own explicit check here. This also keeps
        // `bounds()`/`plot_rect()` current for the embedder's hit-testing.
        if self.bounds.get() != bounds {
            self.bounds.set(bounds);
            *self.band_freq_response.borrow_mut() = Default::default();
        }

        let mut geometries = Vec::with_capacity(EQBand::iter().count() + 3);

        geometries.push(self.grid_cache.draw(renderer, bounds.size(), |frame| {
            self.draw_grid(frame, local_rect, plot_rect);
        }));

        // This is a slightly smaller plot for drawing, just to keep us inside the lines.
        let mut plot_rect = plot_rect;
        plot_rect.x += EQ_PLOT_BORDER_WIDTH;
        plot_rect.width -= EQ_PLOT_BORDER_WIDTH * 2.0;

        for (index, band) in EQBand::iter().enumerate() {
            if self.bands[band].enabled {
                let colour = eq_transparent_colour(index);
                geometries.push(
                    self.band_caches[band].draw(renderer, bounds.size(), |frame| {
                        self.draw_eq_individual(frame, band, plot_rect, colour);
                    }),
                );
            }
        }

        geometries.push(self.curve_cache.draw(renderer, bounds.size(), |frame| {
            self.draw_eq_curve(frame, plot_rect);
        }));

        // Control points + selection ring are cheap and depend on
        // `active`, which can change every frame - draw fresh, uncached.
        let mut points_frame = Frame::new(renderer, bounds.size());
        self.draw_band_points(&mut points_frame, plot_rect);
        geometries.push(points_frame.into_geometry());

        if !self.spectrum_bins.is_empty() {
            geometries.push(self.spectrum_cache.draw(renderer, bounds.size(), |frame| {
                self.draw_spectrum(frame, plot_rect);
            }));
        }

        geometries
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

fn build_catmull_rom_stroke_path(points: &[Point], tension: f32, y_min: f32, y_max: f32) -> Path {
    Path::new(|builder| {
        if let Some(&first) = points.first() {
            builder.move_to(first);
        }
        catmull_rom_continue(builder, points, tension, y_min, y_max);
    })
}

fn build_catmull_rom_fill_path(
    points: &[Point],
    baseline_y: f32,
    tension: f32,
    y_min: f32,
    y_max: f32,
) -> Path {
    Path::new(|builder| {
        if let Some(&first) = points.first() {
            builder.move_to(Point::new(first.x, baseline_y));
            builder.line_to(first);
        }
        catmull_rom_continue(builder, points, tension, y_min, y_max);
        if let Some(&last) = points.last() {
            builder.line_to(Point::new(last.x, baseline_y));
        }
        builder.close();
    })
}

fn catmull_rom_continue(
    builder: &mut canvas::path::Builder,
    points: &[Point],
    tension: f32,
    y_min: f32,
    y_max: f32,
) {
    if points.len() < 2 {
        return;
    }

    if points.len() == 2 {
        builder.line_to(points[1]);
        return;
    }

    for i in 0..points.len() - 1 {
        let p0 = if i == 0 { points[i] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < points.len() {
            points[i + 2]
        } else {
            points[i + 1]
        };

        let control_a = Point::new(
            p1.x + (p2.x - p0.x) / (6.0 * tension),
            (p1.y + (p2.y - p0.y) / (6.0 * tension)).clamp(y_min, y_max),
        );
        let control_b = Point::new(
            p2.x - (p3.x - p1.x) / (6.0 * tension),
            (p2.y - (p3.y - p1.y) / (6.0 * tension)).clamp(y_min, y_max),
        );

        builder.bezier_curve_to(control_a, control_b, p2);
    }
}
