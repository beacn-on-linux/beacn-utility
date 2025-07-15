// This struct is responsible for all the drawing, messaging, and updating of a channel on
// the Mix / Mix Create display

use crate::device_manager::ControlMessage;
use crate::integrations::pipeweaver::layout::GradientDirection::{BottomToTop, TopToBottom};
use crate::integrations::pipeweaver::layout::*;
use beacn_lib::crossbeam::channel::Sender;
use enum_map::EnumMap;
use image::{load_from_memory, ImageBuffer, Rgba, RgbaImage};
use pipeweaver_shared::{Mix, MuteTarget};

pub(crate) struct ChannelConfig {
    pub(crate) title: String,
    pub(crate) colour: Rgba<u8>,

    pub(crate) volumes: EnumMap<Mix, u8>,
    pub(crate) active_mix: Mix,
    pub(crate) mute_states: EnumMap<MuteTarget, MuteState>,
}

pub(crate) struct MuteState {
    pub(crate) is_active: bool,
    pub(crate) is_mute_to_all: bool,
}

pub(crate) struct PipeweaverChannel {
    index: u8,
    sender: Sender<ControlMessage>,
    config: ChannelConfig,
    // Add cached images below
}

impl PipeweaverChannel {
    pub fn new(index: u8, sender: Sender<ControlMessage>, config: ChannelConfig) -> Self {
        Self {
            index,
            sender,
            config,
        }
    }

    pub fn get_initial(&self) -> ImageData {
        // Firstly, lets grab some fixed dimensions
        let (w, h) = CHANNEL_DIMENSIONS;

        let mut base = ImageBuffer::from_pixel(w, h, BACKGROUND_COLOUR);
        let content = self.draw_content_box();
        let header = self.draw_header();
        let header_bar = self.draw_bar(HEADER_BAR_DIMENSIONS);
        let mute_bar = self.draw_bar(MUTE_BAR_DIMENSIONS);
        let mute_background = self.draw_mute_background();

        DrawingUtils::composite_from_pos(&mut base, &content, CHANNEL_INNER_POSITION);
        DrawingUtils::composite_from_pos(&mut base, &header, HEADER_POSITION);
        DrawingUtils::composite_from_pos(&mut base, &header_bar, HEADER_BAR_POSITION);
        DrawingUtils::composite_from_pos(&mut base, &mute_bar, MUTE_BAR_POSITION);
        DrawingUtils::composite_from_pos(&mut base, &mute_background, MUTE_AREA_POSITION);

        let dial = self.get_volume_as_rgba();
        DrawingUtils::composite_from_pos(&mut base, &dial, VOLUME_POSITION);

        ImageData {
            x: self.index as u32 * CHANNEL_DIMENSIONS.0,
            y: 0,
            image: base,
        }
    }

    fn get_volume_as_rgba(&self) -> RgbaImage {
        let active = self.config.active_mix;
        let volume = self.config.volumes[active];
        if let Some(jpeg_data) = DIAL_VOLUME_JPEG[active].get(&volume) {
            if let Ok(img) = load_from_memory(jpeg_data) {
                return img.into_rgba8();
            }
        }
        panic!("Unable to Load Volume Image for Mix: {:?}", active);
    }

    pub(crate) fn render_volume(&self) {
        let active = self.config.active_mix;
        let volume = self.config.volumes[active];
        if let Some(jpeg_data) = DIAL_VOLUME_JPEG[active].get(&volume) {
            let (mut x, y) = VOLUME_POSITION;
            x = self.index as u32 * CHANNEL_DIMENSIONS.0 + x;
            let (tx, rx) = oneshot::channel();
            let _ = self.sender.send(ControlMessage::SendImage(jpeg_data.clone(), x, y, tx));
            let _ = rx.recv();
        }
    }

    fn draw_content_box(&self) -> RgbaImage {
        DrawingUtils::draw_box(
            CHANNEL_INNER_DIMENSIONS.0,
            CHANNEL_INNER_DIMENSIONS.1,
            CHANNEL_INNER_BORDER,
            CHANNEL_INNER_RADIUS,
            CHANNEL_BORDER_COLOUR,
            BACKGROUND_COLOUR,
            CHANNEL_INNER_COLOUR,
        )
    }

    fn draw_header(&self) -> RgbaImage {
        let mut colour = self.config.colour.clone();
        colour[3] = 100;

        let (width, height) = HEADER_DIMENSIONS;
        let (text_width, text_height) = HEADER_TEXT_DIMENSIONS;
        let mut base = DrawingUtils::draw_gradient(width, height, colour, TopToBottom);
        let text = DrawingUtils::draw_text(
            self.config.title.to_string(),
            text_width,
            text_height,
            HEADER_FONT,
            HEADER_FONT_SIZE,
            TextAlign::Center,
        );

        // Draw the text over the gradient
        DrawingUtils::composite_from(&mut base, &text, 0, 0);

        // Return it
        base
    }

    fn draw_bar(&self, dimensions: Dimension) -> RgbaImage {
        ImageBuffer::from_pixel(dimensions.0, dimensions.1, self.config.colour)
    }

    fn send_volume(&self) -> RgbaImage {
        let (width, height) = VOLUME_DIMENSIONS;
        // For this, we'll just render a square
        ImageBuffer::from_pixel(width, height, CHANNEL_INNER_COLOUR)
    }

    fn draw_mute_background(&self) -> RgbaImage {
        let (w, h) = MUTE_AREA_DIMENSIONS;
        let mut colour = self.config.colour.clone();
        colour[3] = 128;

        DrawingUtils::draw_gradient(w, h, colour, BottomToTop)
    }

    fn draw_mute_box(&self) -> RgbaImage {
        todo!()
    }
}

pub(crate) struct ImageData {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) image: RgbaImage
}