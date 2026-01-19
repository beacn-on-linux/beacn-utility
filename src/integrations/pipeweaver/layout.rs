// This file is a mess, and it's mostly intentional for the first pass, it primarily informs
// on how to render everything, positions, shapes, etc... I'll keep some level of documentation

use crate::APP_NAME;
use anyhow::{Context, Result, anyhow, bail};
use enum_map::EnumMap;
use fontdue::Font;
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageBuffer, Rgb, RgbImage, Rgba, RgbaImage, load_from_memory};
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use pipeweaver_shared::Mix;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::fs::File;
use std::io::ErrorKind::UnexpectedEof;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::PathBuf;
use std::time::Instant;
use strum::IntoEnumIterator;
use xdg::BaseDirectories;

// First thing we need, is to device the font used for rendering on the screen
#[allow(unused)]
pub(crate) static FONT: &[u8] =
    include_bytes!("../../../resources/fonts/rubik/static/Rubik-SemiBold.ttf");
pub(crate) static FONT_BOLD: &[u8] =
    include_bytes!("../../../resources/fonts/rubik/static/Rubik-Bold.ttf");

pub(crate) static HEADER: &[u8] = include_bytes!("../../../resources/screens/header.jpg");

pub(crate) static JPEG_QUALITY: u8 = 70;

// Now, for sanity's sake, we're going to define some basic types
pub(crate) type Dimension = (u32, u32);
pub(crate) type Position = (u32, u32);

// These types are used for rendering the Dials, and are mostly related to precaching images
// in memory to allow 'quick switching' without the need for costly regeneration
type DistanceAngleMap = Lazy<(Vec<Vec<f32>>, Vec<Vec<f32>>)>;
type DialBaseImage = Lazy<RgbaImage>;
type DialValueImage = Lazy<EnumMap<Mix, HashMap<u8, RgbaImage>>>;
type DialTextImage = Lazy<HashMap<u8, RgbaImage>>;
type DialVolumeJPEG = Lazy<EnumMap<Mix, HashMap<u8, Vec<u8>>>>;

// Resolution of the Beacn Mix / Mix Create Screens, and how many channels to display
pub(crate) static DISPLAY_DIMENSIONS: Dimension = (800, 480);
pub(crate) static CHANNEL_COUNT: u32 = 4;

pub(crate) static POSITION_ROOT: Position = (0, 80);

// Ok, so these statics are all self referencing, retrieving a jpeg for a dial will cause it
// to generate the angle map for the circles, the text, the Mix A / B images for each percentage
// as well as a base circle. All of these then get composited and cached into about 200 "final"
// JPEGs which can be sent as-is to the Mix / Mix Create
//
// This list may go up in the future when Metering is added, which will throw us into the thousands
// of cached images region, at which point I'll be ensuring that disk caching is an option, until
// then though, it takes 6 seconds in DEBUG mode to generate these images, and 0.6s in RELEASE
// mode, which is good enough for now.
pub(crate) static DISTANCE_ANGLE_MAP: DistanceAngleMap = Lazy::new(DialHandler::precompute_maps);
pub(crate) static DIAL_BASE_IMAGE: DialBaseImage =
    Lazy::new(DialHandler::precompute_dial_background);
pub(crate) static DIAL_MIX_IMAGES: DialValueImage = Lazy::new(DialHandler::precompute_dial_volumes);
pub(crate) static DIAL_TEXT_IMAGES: DialTextImage = Lazy::new(DialHandler::precompute_dial_text);
pub(crate) static DIAL_VOLUME_JPEG: DialVolumeJPEG = Lazy::new(DialHandler::composite_dials);

// Next up, we define some colours, which will be used when generating components
pub(crate) static TEXT_COLOUR: Rgba<u8> = Rgba([180, 180, 180, 255]);
pub(crate) static BG_COLOUR: Rgba<u8> = Rgba([42, 48, 45, 255]);

pub(crate) static DIAL_INACTIVE: Rgba<u8> = Rgba([37, 41, 39, 255]);
pub(crate) static MIX_A_DIAL: Rgba<u8> = Rgba([89, 177, 182, 255]);
pub(crate) static MIX_B_DIAL: Rgba<u8> = Rgba([224, 124, 36, 255]);

pub(crate) static CHANNEL_BORDER_COLOUR: Rgba<u8> = Rgba([101, 101, 101, 255]);
pub(crate) static CHANNEL_INNER_COLOUR: Rgba<u8> = Rgba([51, 55, 53, 255]);

// Ok, so for positions and sizing, start with the basic draw area for a channel
pub(crate) static CHANNEL_DIMENSIONS: Dimension = (
    DISPLAY_DIMENSIONS.0 / CHANNEL_COUNT,
    DISPLAY_DIMENSIONS.1 - POSITION_ROOT.1,
);

// So we're going to approach this by creating a 'base' canvas of DISPLAY_WIDTH / CHANNEL_COUNT,
// so 200x480, and all elements need absolute positioning inside that region. So we're going
// to define all the components with dimensions and positions, and use them to organise things
pub(crate) static CHANNEL_MARGIN: u32 = 10;

// Define the Dimensions, Positions and style of the 'Inner' Box
pub(crate) static CHANNEL_INNER_DIMENSIONS: Dimension =
    (CHANNEL_DIMENSIONS.0 - (CHANNEL_MARGIN * 2), 310);

pub(crate) static CHANNEL_INNER_DIMENSIONS_MIX: Dimension =
    (CHANNEL_DIMENSIONS.0 - (CHANNEL_MARGIN * 2), 262);

pub(crate) static CHANNEL_INNER_POSITION: Position = (CHANNEL_MARGIN, CHANNEL_MARGIN);
pub(crate) static CHANNEL_INNER_BORDER: BorderThickness = BorderThickness(3, 3, 3, 3);
pub(crate) static CHANNEL_INNER_RADIUS: BorderRadius = BorderRadius(8, 8, 0, 0);

// Define the Channel 'Content' dimensions (Channel Inner - Border Width)
pub(crate) static CONTENT_DIMENSIONS: Dimension = (
    CHANNEL_INNER_DIMENSIONS.0 - CHANNEL_INNER_BORDER.1 - CHANNEL_INNER_BORDER.3,
    CHANNEL_INNER_DIMENSIONS.1 - CHANNEL_INNER_BORDER.0 - CHANNEL_INNER_BORDER.2,
);
pub(crate) static CONTENT_POSITION: Position = (
    CHANNEL_INNER_POSITION.0 + CHANNEL_INNER_BORDER.3,
    CHANNEL_INNER_POSITION.1 + CHANNEL_INNER_BORDER.0,
);

// First element inside the main box is the channel header, so let's configure that.
pub(crate) static HEADER_DIMENSIONS: Dimension =
    (CONTENT_DIMENSIONS.0, 30 + CHANNEL_INNER_RADIUS.0);
pub(crate) static HEADER_POSITION: Position = (
    CONTENT_POSITION.0,
    CONTENT_POSITION.1 + CHANNEL_INNER_RADIUS.0,
);
pub(crate) static HEADER_FONT_SIZE: f32 = 22.0;
pub(crate) static HEADER_FONT: &[u8] = FONT_BOLD;
pub(crate) static HEADER_TEXT_DIMENSIONS: Dimension = (CONTENT_DIMENSIONS.0, 30);

// Generic Bar Layout
pub(crate) static BAR_DIMENSIONS: Dimension = (CONTENT_DIMENSIONS.0, 6);

// Next the coloured Bar
pub(crate) static HEADER_BAR_POSITION: Position =
    (CONTENT_POSITION.0, HEADER_POSITION.1 + HEADER_DIMENSIONS.1);

// Now the Dial (Simple Square)
static VOLUME_CROP: u32 = 10;
pub(crate) static VOLUME_DIMENSIONS: Dimension = (CONTENT_DIMENSIONS.0, CONTENT_DIMENSIONS.0);
pub(crate) static VOLUME_POSITION: Position =
    (CONTENT_POSITION.0, HEADER_BAR_POSITION.1 + BAR_DIMENSIONS.1);
static VOLUME_FONT: &[u8] = FONT_BOLD;
static VOLUME_FONT_SIZE: f32 = 34.0;

// Next a coloured bar before the mute buttons
pub(crate) static MUTE_BAR_POSITION: Position = (
    CONTENT_POSITION.0,
    VOLUME_POSITION.1 + VOLUME_DIMENSIONS.1 - VOLUME_CROP,
);

// Finally, the Mute Button Section
pub(crate) static MUTE_AREA_DIMENSIONS: Dimension = (CONTENT_DIMENSIONS.0, 85);
pub(crate) static MUTE_AREA_DIMENSIONS_MIX: Dimension =
    (CONTENT_DIMENSIONS.0, MUTE_BUTTON_DIMENSIONS.1);
pub(crate) static MUTE_AREA_POSITION: Position =
    (CONTENT_POSITION.0, MUTE_BAR_POSITION.1 + BAR_DIMENSIONS.1);

pub(crate) static MUTE_BUTTON_DIMENSIONS: Dimension = (CONTENT_DIMENSIONS.0, 37);

pub(crate) static MUTE_POSITION_A: Position = MUTE_AREA_POSITION;
pub(crate) static MUTE_LOCAL_POSITION_A: Position = (0, 0);
pub(crate) static MUTE_A_BORDER: BorderThickness = BorderThickness(0, 0, 2, 0);

pub(crate) static MUTE_GAP: Dimension = (0, 9);

pub(crate) static ICON_MARGIN: u32 = 10;

pub(crate) static MUTE_TEXT_DIMENSIONS: Dimension = (CONTENT_DIMENSIONS.0, 30);
pub(crate) static MUTE_FONT_SIZE: f32 = 20.0;
pub(crate) static MUTE_FONT: &[u8] = FONT;

pub(crate) static MUTE_POSITION_B: Position = (
    MUTE_POSITION_A.0,
    MUTE_POSITION_A.1 + MUTE_BUTTON_DIMENSIONS.1 + MUTE_A_BORDER.3 + MUTE_GAP.1,
);
pub(crate) static MUTE_LOCAL_POSITION_B: Position = (
    MUTE_LOCAL_POSITION_A.0,
    MUTE_LOCAL_POSITION_A.1 + MUTE_BUTTON_DIMENSIONS.1 + MUTE_A_BORDER.3 + MUTE_GAP.1,
);
pub(crate) static MUTE_B_BORDER: BorderThickness = BorderThickness(2, 0, 0, 0);

pub(crate) static MUTE_COLOUR_OFF: Rgba<u8> = Rgba([80, 80, 80, 220]);
pub(crate) static MUTE_COLOUR_ON: Rgba<u8> = Rgba([255, 0, 0, 100]);

static MUTE_UNMUTED_ICON_BYTES: &[u8] =
    include_bytes!("../../../resources/ui/icons/volume-high-solid.png");
pub(crate) static MUTE_UNMUTED_ICON: Lazy<RgbaImage> = Lazy::new(|| {
    load_from_memory(MUTE_UNMUTED_ICON_BYTES)
        .expect("Failed to Load Image")
        .to_rgba8()
});
static MUTE_MUTED_ICON_BYTES: &[u8] =
    include_bytes!("../../../resources/ui/icons/volume-xmark-solid.png");
pub(crate) static MUTE_MUTED_ICON: Lazy<RgbaImage> = Lazy::new(|| {
    load_from_memory(MUTE_MUTED_ICON_BYTES)
        .expect("Failed to Load Image")
        .to_rgba8()
});

pub(crate) static BORDER_RADIUS_NONE: BorderRadius = BorderRadius(0, 0, 0, 0);

// Helper Structs
/// Top left, Top right, Bottom left, Bottom right
#[derive(Debug, Copy, Clone)]
pub(crate) struct BorderRadius(u32, u32, u32, u32);

/// Top, Right, Bottom, Left
#[derive(Debug, Copy, Clone)]
pub(crate) struct BorderThickness(
    pub(crate) u32,
    pub(crate) u32,
    pub(crate) u32,
    pub(crate) u32,
);

pub(crate) enum GradientDirection {
    TopToBottom,
    BottomToTop,
}

#[allow(unused)]
pub(crate) enum TextAlign {
    Left,
    Center,
    Right,
}

pub(crate) struct DrawingUtils;
impl DrawingUtils {
    // Generates a box with custom borders, and corners
    pub(crate) fn draw_box(
        width: u32,
        height: u32,
        border_thickness: BorderThickness,
        border_radius: BorderRadius,
        border_colour: Rgba<u8>,
        background_colour: Rgba<u8>,
        foreground_colour: Rgba<u8>,
    ) -> RgbaImage {
        // This code is not the prettiest princess, but it works for what I need.
        // Known edge cases:
        // - If the border radius is larger than half the box dimension, or if the border thickness
        //   is greater than the radius, the drawing may be incorrect or visually broken.
        // - Extreme radii or border widths may cause overlapping or missing pixels in corners.
        // These cases are not handled because they are not needed for the current UI design.
        let feather = 1. / 2f32.sqrt();
        let mut img = ImageBuffer::new(width, height);

        // Abstract out the u8s to f32s
        let (tlr, trr, blr, brr) = (
            border_radius.0 as f32,
            border_radius.1 as f32,
            border_radius.2 as f32,
            border_radius.3 as f32,
        );
        let (bt, br, bb, bl) = (
            border_thickness.0 as f32,
            border_thickness.1 as f32,
            border_thickness.2 as f32,
            border_thickness.3 as f32,
        );

        // Calculate the 'Content' area as a rectangle for later masking based on the borders
        let inner_left = bl;
        let inner_right = width as f32 - br;
        let inner_top = bt;
        let inner_bottom = height as f32 - bb;

        for y in 0..height {
            for x in 0..width {
                // When working with feathering and the likes, it's best to position ourselves in
                // the center of the pixel, to allow for subpixel blending
                let (xf, yf) = (x as f32 + 0.5, y as f32 + 0.5);

                // This is primarily a check to see if we're inside a corner zone, the zone is
                // defined by the radius, so a 20px radius will be 20x20.
                //
                // If we are, return the radius, the x/y coordinate of the top left of the corner
                // as well as the thickness of borders attached to this corner
                let (radius, cx, cy, horiz_thickness, vert_thickness) = if xf < tlr && yf < tlr {
                    (tlr, tlr, tlr, bt, bl)
                } else if xf >= width as f32 - trr && yf < trr {
                    (trr, width as f32 - trr, trr, bt, br)
                } else if xf < blr && yf > height as f32 - blr {
                    (blr, blr, height as f32 - blr, bb, bl)
                } else if xf >= width as f32 - brr && yf >= height as f32 - brr {
                    (brr, width as f32 - brr, height as f32 - brr, bb, br)
                } else {
                    (0.0, 0.0, 0.0, 0.0, 0.0)
                };

                // We need to account for situations where one border is thicker than the other, in
                // this case, we need to adjust based on the thickest border.
                let adj_border = horiz_thickness.max(vert_thickness);

                // We also need to check whether we're inside the content area, for masking purposes
                let in_content_rect =
                    xf >= inner_left && xf <= inner_right && yf >= inner_top && yf < inner_bottom;

                // If our radius is set, we are somewhere in the corner
                let (target_color, alpha) = if radius > 0.0 {
                    // Grab a relative position inside this corner
                    let dx = xf - cx;
                    let dy = yf - cy;

                    // Calculate how far away we are from the edge
                    let dist = (dx * dx + dy * dy).sqrt();

                    // The outer edge is defined by the radius
                    let outer_edge = radius;

                    // The inner edge is the radius adjusted for the border
                    let inner_edge = (radius - adj_border).max(0.0);

                    if dist <= inner_edge {
                        // We are inside the inner edge of the border, fill with foreground
                        (foreground_colour, foreground_colour[3] as f32 / 255.0)
                    } else if dist <= outer_edge {
                        // We're outside the edge, apply feathering if appropriate
                        let t = ((outer_edge - dist) / feather).clamp(0.0, 1.0);
                        (
                            Self::blend_rgba(border_colour, foreground_colour, t),
                            Self::blend_alpha(border_colour, foreground_colour, t),
                        )
                    } else {
                        // We're purely outside, so just use the background
                        (background_colour, background_colour[3] as f32 / 255.0)
                    }
                } else if in_content_rect {
                    //debug!("Alpha: {}", foreground_colour[3] as f32 / 255.);

                    // We're in the content area, so apply masking and colouring
                    (foreground_colour, foreground_colour[3] as f32 / 255.0)
                } else {
                    // We're part of the border, so apply the border
                    (border_colour, border_colour[3] as f32 / 255.0)
                };

                img.put_pixel(
                    x,
                    y,
                    Rgba([
                        target_color[0],
                        target_color[1],
                        target_color[2],
                        (alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                    ]),
                );
            }
        }

        img
    }

    fn blend_rgba(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
        let t = t.clamp(0.0, 1.0);
        Rgba([
            (a[0] as f32 * t + b[0] as f32 * (1.0 - t)).round() as u8,
            (a[1] as f32 * t + b[1] as f32 * (1.0 - t)).round() as u8,
            (a[2] as f32 * t + b[2] as f32 * (1.0 - t)).round() as u8,
            255,
        ])
    }

    fn blend_alpha(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        (a[3] as f32 / 255.0) * t + (b[3] as f32 / 255.0) * (1.0 - t)
    }

    pub(crate) fn draw_gradient(
        w: u32,
        h: u32,
        colour: Rgba<u8>,
        direction: GradientDirection,
    ) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        let source_alpha = colour[3] as f32;

        for y in 0..h {
            for x in 0..w {
                let factor = match direction {
                    GradientDirection::TopToBottom => y as f32 / (h - 1) as f32,
                    GradientDirection::BottomToTop => (h - 1 - y) as f32 / (h - 1) as f32,
                };

                let alpha = (source_alpha * factor).round() as u8;
                img.put_pixel(x, y, Rgba([colour[0], colour[1], colour[2], alpha]));
            }
        }
        img
    }

    pub(crate) fn draw_text(
        text: String,
        width: u32,
        height: u32,
        font: &[u8],
        font_size: f32,
        colour: Rgba<u8>,
        align: TextAlign,
    ) -> RgbaImage {
        let font = Font::from_bytes(font, fontdue::FontSettings::default()).unwrap();
        let (font_r, font_g, font_b) = (colour[0], colour[1], colour[2]);
        let mut img = RgbaImage::new(width, height);

        // Font-wide vertical metrics
        let line_metrics = font.horizontal_line_metrics(font_size).unwrap();
        let ascent = line_metrics.ascent;
        let descent = line_metrics.descent;
        let total_font_height = ascent - descent;

        // Baseline placement: center the total ascent+descent box
        let baseline_y = ((height as f32 - total_font_height) / 2.0 + ascent).round() as i32;

        // Prepare glyphs, measure total width
        let mut text_width = 0;
        let mut glyphs = Vec::new();

        for c in text.chars() {
            let (metrics, bitmap) = font.rasterize(c, font_size);
            text_width += metrics.advance_width as usize;
            glyphs.push((metrics, bitmap));
        }

        // Horizontal alignment
        let start_x = match align {
            TextAlign::Left => 0,
            TextAlign::Right => img.width() as i32 - text_width as i32,
            TextAlign::Center => ((img.width() as i32 - text_width as i32) / 2).max(0),
        };

        let mut cursor_x = start_x;
        for (metrics, bitmap) in glyphs {
            let glyph_width = metrics.width;
            let glyph_height = metrics.height;

            for y in 0..glyph_height {
                for x in 0..glyph_width {
                    let alpha = bitmap[y * glyph_width + x];
                    if alpha > 0 {
                        let px = cursor_x + x as i32 + metrics.xmin;
                        let py = baseline_y - metrics.ymin + y as i32 - glyph_height as i32;

                        if px >= 0 && py >= 0 && px < img.width() as i32 && py < img.height() as i32
                        {
                            img.put_pixel(
                                px as u32,
                                py as u32,
                                Rgba([font_r, font_g, font_b, alpha]),
                            );
                        }
                    }
                }
            }

            cursor_x += metrics.advance_width as i32;
        }
        img
    }

    pub(crate) fn composite_from(base: &mut RgbaImage, overlay: &RgbaImage, x: u32, y: u32) {
        let base_width = base.width();
        let base_height = base.height();

        for (ox, oy, overlay_pixel) in overlay.enumerate_pixels() {
            let dest_x = x + ox;
            let dest_y = y + oy;

            if dest_x >= base_width || dest_y >= base_height {
                continue;
            }

            let [sr, sg, sb, sa] = overlay_pixel.0;
            if sa == 0 {
                continue;
            }

            let [dr, dg, db, da] = base.get_pixel(dest_x, dest_y).0;

            let src_a = sa as f32 / 255.0;
            let dst_a = da as f32 / 255.0;

            // Result alpha (source-over)
            let out_a = src_a + dst_a * (1.0 - src_a);
            if out_a == 0.0 {
                continue;
            }

            // Blend colors
            let out_r = (sr as f32 * src_a + dr as f32 * dst_a * (1.0 - src_a)) / out_a;
            let out_g = (sg as f32 * src_a + dg as f32 * dst_a * (1.0 - src_a)) / out_a;
            let out_b = (sb as f32 * src_a + db as f32 * dst_a * (1.0 - src_a)) / out_a;

            base.put_pixel(
                dest_x,
                dest_y,
                Rgba([
                    (out_r.clamp(0.0, 255.0)) as u8,
                    (out_g.clamp(0.0, 255.0)) as u8,
                    (out_b.clamp(0.0, 255.0)) as u8,
                    (out_a * 255.0).clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }

    pub(crate) fn composite_from_pos(base: &mut RgbaImage, overlay: &RgbaImage, pos: Position) {
        let (x, y) = pos;
        Self::composite_from(base, overlay, x, y);
    }

    pub(crate) fn image_as_jpeg(
        image: RgbaImage,
        background: Rgba<u8>,
        quality: u8,
    ) -> Result<Vec<u8>> {
        let flattened = Self::flatten_rgba_image(&image, background);

        let mut jpeg_data = Vec::new();
        let mut cursor = Cursor::new(&mut jpeg_data);
        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, quality);
        encoder.encode(
            &flattened,
            flattened.width(),
            flattened.height(),
            ExtendedColorType::from(image::ColorType::Rgb8),
        )?;
        Ok(jpeg_data)
    }

    pub fn flatten_rgba_image(rgba_img: &RgbaImage, background: Rgba<u8>) -> RgbImage {
        let (width, height) = rgba_img.dimensions();
        let mut rgb_img = RgbImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let rgba = *rgba_img.get_pixel(x, y);
                let blended = Self::flatten_rgba_pixel(rgba, background);
                rgb_img.put_pixel(x, y, blended);
            }
        }

        rgb_img
    }

    fn flatten_rgba_pixel(pixel: Rgba<u8>, bg: Rgba<u8>) -> Rgb<u8> {
        let fg_alpha = pixel[3] as f32 / 255.0;
        let bg_alpha = bg[3] as f32 / 255.0;
        let out_alpha = fg_alpha + bg_alpha * (1.0 - fg_alpha);

        if out_alpha == 0.0 {
            return Rgb([0, 0, 0]);
        }

        let blend = |f: u8, b: u8| {
            ((f as f32 * fg_alpha + b as f32 * bg_alpha * (1.0 - fg_alpha)) / out_alpha).round()
                as u8
        };

        Rgb([
            blend(pixel[0], bg[0]),
            blend(pixel[1], bg[1]),
            blend(pixel[2], bg[2]),
        ])
    }

    pub fn get_volume_image(volume: u8, mix: Mix) -> Result<Vec<u8>> {
        let mut base = DIAL_BASE_IMAGE.clone();
        let dial = DIAL_MIX_IMAGES[mix]
            .get(&volume)
            .ok_or(anyhow!("Not Found"))?;
        let text = DIAL_TEXT_IMAGES
            .get(&volume)
            .ok_or(anyhow!("Text Not Found"))?;

        // Composite this together...
        Self::composite_from(&mut base, dial, 0, 0);
        Self::composite_from(&mut base, text, 0, 0);

        // Drop the bottom 6 pixels from the image
        let (width, mut height) = VOLUME_DIMENSIONS;
        height -= VOLUME_CROP;
        let cropped = image::imageops::crop_imm(&base, 0, 0, width, height);
        Self::image_as_jpeg(cropped.to_image(), CHANNEL_INNER_COLOUR, JPEG_QUALITY)
    }
}

struct DialHandler;
impl DialHandler {
    pub fn composite_dials() -> EnumMap<Mix, HashMap<u8, Vec<u8>>> {
        let start = Instant::now();

        let file_name = "image_cache.bin".to_string();
        let xdg_dirs = BaseDirectories::with_prefix(APP_NAME);
        let cache_file = xdg_dirs.find_cache_file(file_name.clone());
        debug!("Attempting to load Cache from {cache_file:?}");
        if let Some(file) = cache_file {
            if let Ok(map) = Self::load_cache(file) {
                info!("Loaded Cache in {:?}", start.elapsed());
                return map;
            } else {
                warn!("Cache Load Failed, Regenerating");
            }
        }

        debug!("Generating Images (This will take a second!)");
        let mut map = EnumMap::default();

        for mix in Mix::iter() {
            let mut volume_map = HashMap::new();
            for i in 0..=100 {
                if let Ok(image) = DrawingUtils::get_volume_image(i, mix) {
                    volume_map.insert(i, image);
                }
            }
            map[mix] = volume_map;
        }

        debug!("Generated In {:?}", start.elapsed());

        debug!("Attempting to Save to Cache");
        let time = Instant::now();
        let cache_file = xdg_dirs.place_cache_file(file_name);
        if let Ok(file) = cache_file {
            if let Err(e) = Self::save_cache(file, &map) {
                warn!("Cache Saving Failed: {e}");
            } else {
                info!("Cache Saved in {:?}", time.elapsed());
            }
        }
        map
    }

    fn precompute_dial_background() -> RgbaImage {
        let (width, height) = VOLUME_DIMENSIONS;
        Self::generate_dial(width, height, 100, DIAL_INACTIVE)
    }

    fn precompute_dial_volumes() -> EnumMap<Mix, HashMap<u8, RgbaImage>> {
        let (width, height) = VOLUME_DIMENSIONS;
        let mut enum_map = EnumMap::default();
        for mix in Mix::iter() {
            let colour = match mix {
                Mix::A => MIX_A_DIAL,
                Mix::B => MIX_B_DIAL,
            };
            let mut map = HashMap::new();
            for i in 0..=100 {
                let img = Self::generate_dial(width, height, i, colour);
                map.insert(i, img);
            }
            enum_map[mix] = map;
        }
        enum_map
    }

    fn precompute_dial_text() -> HashMap<u8, RgbaImage> {
        let (width, height) = VOLUME_DIMENSIONS;
        let mut map = HashMap::new();
        for i in 0..=100 {
            let text = format!("{i:.0}%");
            let img = DrawingUtils::draw_text(
                text,
                width,
                height,
                VOLUME_FONT,
                VOLUME_FONT_SIZE,
                TEXT_COLOUR,
                TextAlign::Center,
            );
            map.insert(i, img);
        }
        map
    }

    fn precompute_maps() -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
        let (width, height) = VOLUME_DIMENSIONS;
        let center = width as f32 / 2.0;

        let mut distance_map = vec![vec![0.0; width as usize]; height as usize];
        let mut angle_map = vec![vec![0.0; width as usize]; height as usize];

        for y in 0..height as usize {
            for x in 0..width as usize {
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                distance_map[y][x] = dx * dx + dy * dy;
                angle_map[y][x] = Self::normalize_angle(-dy.atan2(dx)); // clockwise
            }
        }

        (distance_map, angle_map)
    }
    fn normalize_angle(angle: f32) -> f32 {
        let mut a = angle % (2.0 * PI);
        if a < 0.0 {
            a += 2.0 * PI;
        }
        a
    }

    fn generate_dial(width: u32, height: u32, percent: u8, colour: Rgba<u8>) -> RgbaImage {
        let padding = 10;
        let outer_radius = ((width.min(height) / 2) - padding) as f32;
        let thickness = 15.0;
        let inner_radius = outer_radius - thickness;

        let gap_angle = 0.2 * 2.0 * PI; // 20% gap in radians
        let arc_span = 2.0 * PI - gap_angle;
        let start_angle = (3.0 * PI / 2.0) + gap_angle / 2.0;
        let adjusted_angle = start_angle + arc_span;

        let feather_width = 1.5; // Width in pixels for edge softening
        let outer_blend_start = outer_radius - feather_width;
        let inner_blend_start = inner_radius + feather_width;

        let outer_blend_start_sq = outer_blend_start * outer_blend_start;
        let inner_blend_start_sq = inner_blend_start * inner_blend_start;

        let outer_radius_sq = outer_radius * outer_radius;
        let inner_radius_sq = inner_radius * inner_radius;

        let value_percent = percent as f32 / 100.;
        let value_span = arc_span * value_percent;

        let mut img = RgbaImage::new(width, height);

        for y in 0..height as usize {
            for x in 0..width as usize {
                let distance = DISTANCE_ANGLE_MAP.0[y][x];

                // Skip anything outside the ring
                if distance < inner_radius_sq || distance > outer_radius_sq {
                    continue;
                }

                let angle = DISTANCE_ANGLE_MAP.1[y][x];
                let angle_from_start = Self::normalize_angle(adjusted_angle - angle);
                if angle_from_start <= value_span {
                    let distance_sqrt = distance.sqrt(); // only here, for feathering
                    let alpha = if distance > outer_blend_start_sq {
                        // Feather outer edge
                        let fade = (outer_radius - distance_sqrt) / feather_width;
                        (fade * 255.0).clamp(0.0, 255.0)
                    } else if distance < inner_blend_start_sq {
                        // Feather inner edge
                        let fade = (distance_sqrt - inner_radius) / feather_width;
                        (fade * 255.0).clamp(0.0, 255.0)
                    } else {
                        255.0
                    };
                    let colour = Rgba([colour.0[0], colour.0[1], colour.0[2], alpha as u8]);
                    img.put_pixel(x as u32, y as u32, colour);
                }
            }
        }
        img
    }

    fn save_cache(path: PathBuf, map: &EnumMap<Mix, HashMap<u8, Vec<u8>>>) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for (mix, volume_map) in map.iter() {
            let mix_id = mix as u8;
            for (&volume, data) in volume_map {
                writer.write_all(&[mix_id, volume])?;
                let len = data.len() as u32;
                writer.write_all(&len.to_le_bytes())?;
                writer.write_all(data)?;
            }
        }
        Ok(())
    }

    fn load_cache(path: PathBuf) -> Result<EnumMap<Mix, HashMap<u8, Vec<u8>>>> {
        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        let mut map: EnumMap<Mix, HashMap<u8, Vec<u8>>> = EnumMap::default();

        loop {
            let mut header = [0u8; 6];
            if let Err(e) = reader.read_exact(&mut header) {
                if e.kind() == UnexpectedEof {
                    break; // EOF reached, stop reading
                }
                bail!("Failed to read header from cache file");
            }

            let mix = match header[0] {
                0 => Mix::A,
                1 => Mix::B,
                _ => bail!("Invalid mix identifier: {}", header[0]),
            };
            let volume = header[1];
            let len = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;

            let mut data = vec![0u8; len];
            reader.read_exact(&mut data).with_context(|| {
                format!("Failed to read image data for mix {mix:?}, volume {volume}")
            })?;

            map[mix].insert(volume, data);
        }

        Ok(map)
    }
}
