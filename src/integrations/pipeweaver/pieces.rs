// Ok, this is a simple system for channels which render the component parts of a channel plate
// in a way that can be cleanly cached (along with some cache helpers).

use crate::integrations::pipeweaver::ChannelType;
use crate::integrations::pipeweaver::helpers::{Mix, MuteTarget};
use crate::integrations::pipeweaver::layout::GradientDirection::{BottomToTop, TopToBottom};
use crate::integrations::pipeweaver::layout::*;
use anyhow::Result;
use image::imageops::{crop, crop_imm};
use image::{Rgba, RgbaImage};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

pub(crate) type Jpeg = Arc<Vec<u8>>;
pub(crate) type Rgb3 = [u8; 3];

/// Everything about a single mute button that changes how it looks
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MuteFace {
    pub(crate) active: bool,
    pub(crate) to_all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum PieceKey {
    Top {
        colour: Rgb3,
        title: String,
    },
    Mute {
        kind: ChannelType,
        colour: Rgb3,
        a: MuteFace,
        b: Option<MuteFace>,
    },
}

impl PieceKey {
    /// Where the piece sits inside a channel
    pub(crate) fn origin(&self) -> Position {
        match self {
            PieceKey::Top { .. } => CHANNEL_INNER_POSITION,
            PieceKey::Mute { .. } => (CHANNEL_INNER_POSITION.0, MUTE_BAR_POSITION.1),
        }
    }
}

/// The static background of a slot. Sending one wipes everything in the slot.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Plate {
    Blank,
    Tall,
    Short,
}

/// Top left of a channel slot on the display
pub(crate) fn slot_origin(index: usize) -> Position {
    (CHANNEL_DIMENSIONS.0 * index as u32, POSITION_ROOT.1)
}

/// Where the dial sits inside a channel
pub(crate) fn dial_origin() -> Position {
    VOLUME_POSITION
}

// The Plates
static CHROME_TALL: LazyLock<RgbaImage> = LazyLock::new(|| build_chrome(CHANNEL_INNER_DIMENSIONS));
static CHROME_SHORT: LazyLock<RgbaImage> =
    LazyLock::new(|| build_chrome(CHANNEL_INNER_DIMENSIONS_MIX));

static PLATE_TALL: LazyLock<Jpeg> = LazyLock::new(|| encode(&CHROME_TALL));
static PLATE_SHORT: LazyLock<Jpeg> = LazyLock::new(|| encode(&CHROME_SHORT));
static PLATE_BLANK: LazyLock<Jpeg> = LazyLock::new(|| {
    let (w, h) = CHANNEL_DIMENSIONS;
    encode(&RgbaImage::from_pixel(w, h, BG_COLOUR))
});

// The strip across the top is already a JPEG, so it can be sent exactly as it is
pub(crate) static HEADER_STRIP: LazyLock<Jpeg> = LazyLock::new(|| Arc::new(HEADER.to_vec()));

fn build_chrome(inner: Dimension) -> RgbaImage {
    let (w, h) = CHANNEL_DIMENSIONS;
    let mut base = RgbaImage::from_pixel(w, h, BG_COLOUR);

    let content = DrawingUtils::draw_box(
        inner.0,
        inner.1,
        CHANNEL_INNER_BORDER,
        CHANNEL_INNER_RADIUS,
        CHANNEL_BORDER_COLOUR,
        BG_COLOUR,
        CHANNEL_INNER_COLOUR,
    );
    DrawingUtils::composite_from_pos(&mut base, &content, CHANNEL_INNER_POSITION);
    base
}

fn encode(img: &RgbaImage) -> Jpeg {
    Arc::new(
        DrawingUtils::image_as_jpeg(img.clone(), BG_COLOUR, JPEG_QUALITY)
            .expect("Failed to encode JPEG"),
    )
}

pub(crate) fn plate_jpeg(plate: Plate) -> Jpeg {
    match plate {
        Plate::Blank => PLATE_BLANK.clone(),
        Plate::Tall => PLATE_TALL.clone(),
        Plate::Short => PLATE_SHORT.clone(),
    }
}

fn rgba(c: Rgb3) -> Rgba<u8> {
    Rgba([c[0], c[1], c[2], 255])
}

pub(crate) fn render_piece(key: &PieceKey) -> Result<Jpeg> {
    let img = match key {
        PieceKey::Top { colour, title } => render_top(rgba(*colour), title),
        PieceKey::Mute { kind, colour, a, b } => render_mute(*kind, rgba(*colour), *a, *b),
    };
    Ok(Arc::new(DrawingUtils::image_as_jpeg(
        img,
        BG_COLOUR,
        JPEG_QUALITY,
    )?))
}

fn render_top(colour: Rgba<u8>, title: &str) -> RgbaImage {
    let (ox, oy) = CHANNEL_INNER_POSITION;
    let w = CHANNEL_INNER_DIMENSIONS.0;
    let h = HEADER_BAR_POSITION.1 + BAR_DIMENSIONS.1 - oy;

    // These rows are identical on the tall and short plates
    let mut base = crop_imm(&*CHROME_TALL, ox, oy, w, h).to_image();

    let header = draw_header(colour, title);
    DrawingUtils::composite_from(
        &mut base,
        &header,
        HEADER_POSITION.0 - ox,
        HEADER_POSITION.1 - oy,
    );

    let bar = RgbaImage::from_pixel(BAR_DIMENSIONS.0, BAR_DIMENSIONS.1, colour);
    DrawingUtils::composite_from(
        &mut base,
        &bar,
        HEADER_BAR_POSITION.0 - ox,
        HEADER_BAR_POSITION.1 - oy,
    );
    base
}

fn render_mute(kind: ChannelType, colour: Rgba<u8>, a: MuteFace, b: Option<MuteFace>) -> RgbaImage {
    let full = b.is_some();
    let (chrome, inner_height) = if full {
        (&*CHROME_TALL, CHANNEL_INNER_DIMENSIONS.1)
    } else {
        (&*CHROME_SHORT, CHANNEL_INNER_DIMENSIONS_MIX.1)
    };

    // From the top of the mute bar to the bottom of the content box
    let (ox, oy) = (CHANNEL_INNER_POSITION.0, MUTE_BAR_POSITION.1);
    let h = CHANNEL_INNER_POSITION.1 + inner_height - oy;
    let mut base = crop_imm(chrome, ox, oy, CHANNEL_INNER_DIMENSIONS.0, h).to_image();

    // Same order as the old full render: bar, gradient, then the buttons
    let bar = RgbaImage::from_pixel(BAR_DIMENSIONS.0, BAR_DIMENSIONS.1, colour);
    DrawingUtils::composite_from(
        &mut base,
        &bar,
        MUTE_BAR_POSITION.0 - ox,
        MUTE_BAR_POSITION.1 - oy,
    );

    let background = draw_mute_background(colour, full);
    DrawingUtils::composite_from(
        &mut base,
        &background,
        MUTE_AREA_POSITION.0 - ox,
        MUTE_AREA_POSITION.1 - oy,
    );

    let box_a = draw_mute_box(kind, colour, full, MuteTarget::TargetA, a);
    DrawingUtils::composite_from(
        &mut base,
        &box_a,
        MUTE_POSITION_A.0 - ox,
        MUTE_POSITION_A.1 - oy,
    );

    if let Some(b) = b {
        let box_b = draw_mute_box(kind, colour, full, MuteTarget::TargetB, b);
        DrawingUtils::composite_from(
            &mut base,
            &box_b,
            MUTE_POSITION_B.0 - ox,
            MUTE_POSITION_B.1 - oy,
        );
    }
    base
}

fn draw_header(colour: Rgba<u8>, title: &str) -> RgbaImage {
    let mut colour = colour;
    colour[3] = 100;

    let (width, height) = HEADER_DIMENSIONS;
    let (text_width, text_height) = HEADER_TEXT_DIMENSIONS;
    let mut base = DrawingUtils::draw_gradient(width, height, colour, TopToBottom);
    let text = DrawingUtils::draw_text(
        title.to_string(),
        text_width,
        text_height,
        HEADER_FONT,
        HEADER_FONT_SIZE,
        TEXT_COLOUR,
        TextAlign::Center,
    );

    // Draw the text over the gradient
    DrawingUtils::composite_from(&mut base, &text, 0, 0);
    base
}

/// `full` is the tall layout, which has the room for both buttons
fn draw_mute_background(colour: Rgba<u8>, full: bool) -> RgbaImage {
    let (w, h) = MUTE_AREA_DIMENSIONS;
    let (m1, h1) = MUTE_AREA_DIMENSIONS_MIX;

    let mut colour = colour;
    colour[3] = 120;

    let mut gradient = DrawingUtils::draw_gradient(w, h, colour, BottomToTop);
    if full {
        gradient
    } else {
        crop(&mut gradient, 0, 0, m1, h1).to_image()
    }
}

/// The button plus the gradient behind it, cropped to the button
fn draw_mute_box(
    kind: ChannelType,
    colour: Rgba<u8>,
    full: bool,
    target: MuteTarget,
    face: MuteFace,
) -> RgbaImage {
    // Ok, first we need the mute background
    let mut background = draw_mute_background(colour, full);
    let text = match kind {
        ChannelType::Source => match face.to_all {
            true => "Mute to All",
            false => "Mute To...",
        },
        ChannelType::Target => "Mute",
    };

    let border_draw = match target {
        MuteTarget::TargetA => MUTE_A_BORDER,
        MuteTarget::TargetB => MUTE_B_BORDER,
    };

    let (width, height) = MUTE_BUTTON_DIMENSIONS;

    let (box_colour, icon) = match face.active {
        true => (MUTE_COLOUR_ON, &*MUTE_MUTED_ICON),
        false => (MUTE_COLOUR_OFF, &*MUTE_UNMUTED_ICON),
    };

    let mute_box = DrawingUtils::draw_box(
        width,
        height,
        border_draw,
        BORDER_RADIUS_NONE,
        CHANNEL_BORDER_COLOUR,
        Rgba([0, 0, 0, 0]), // The background needs to be transparent so we can overlay it
        box_colour,
    );

    let (x, y) = match target {
        MuteTarget::TargetA => MUTE_LOCAL_POSITION_A,
        MuteTarget::TargetB => MUTE_LOCAL_POSITION_B,
    };

    // Draw the box onto the background
    DrawingUtils::composite_from(&mut background, &mute_box, x, y);

    // The text size needs to be shrunk based on the icon size
    let (mut text_width, text_height) = MUTE_TEXT_DIMENSIONS;
    text_width = text_width - icon.width() - (ICON_MARGIN * 2);

    let text = DrawingUtils::draw_text(
        text.to_string(),
        text_width,
        text_height,
        MUTE_FONT,
        MUTE_FONT_SIZE,
        TEXT_COLOUR,
        TextAlign::Left,
    );

    let middle = height / 2;
    let text_middle = text.height() / 2;
    let icon_middle = icon.height() / 2;

    let text_y = middle - text_middle + y + border_draw.0;
    let icon_y = middle - icon_middle + y + border_draw.0;

    let text_x = icon.width() + (ICON_MARGIN * 2);
    let icon_x = ICON_MARGIN;

    DrawingUtils::composite_from(&mut background, &text, text_x, text_y);
    DrawingUtils::composite_from(&mut background, icon, icon_x, icon_y);

    // Grab the specific area from the Mute Box
    crop_imm(&background, x, y, width, height).to_image()
}

// Screen Model and Cache
#[derive(Default)]
pub(crate) struct PieceCache {
    pieces: HashMap<PieceKey, Jpeg>,
}

impl PieceCache {
    /// Returns the piece, rendering it first if we've never seen it
    pub(crate) fn get(&mut self, key: &PieceKey) -> Result<Jpeg> {
        if let Some(jpeg) = self.pieces.get(key) {
            return Ok(jpeg.clone());
        }
        let jpeg = render_piece(key)?;
        self.pieces.insert(key.clone(), jpeg.clone());
        Ok(jpeg)
    }

    /// Drops anything that no current (or one toggle away) channel state can produce
    pub(crate) fn retain_live(&mut self, live: &HashSet<PieceKey>) {
        self.pieces.retain(|k, _| live.contains(k));
    }
}

/// What we last sent to a single channel slot
#[derive(Default, Clone)]
pub(crate) struct SlotShown {
    pub(crate) plate: Option<Plate>,
    pub(crate) top: Option<PieceKey>,
    pub(crate) mute: Option<PieceKey>,
    pub(crate) dial: Option<(Mix, u8, u8)>,
}

/// What we believe the device is currently displaying. Anything not matching what we want
/// gets sent, anything matching is skipped, and anything we skipped (because the device was
/// suspended) simply stays out of date until the next sync.
#[derive(Default)]
pub(crate) struct ScreenModel {
    pub(crate) header_drawn: bool,
    pub(crate) slots: [SlotShown; 4],
}

impl ScreenModel {
    /// Call whenever something else has drawn over the screen (splash, error messages)
    pub(crate) fn invalidate(&mut self) {
        *self = Self::default();
    }
}
