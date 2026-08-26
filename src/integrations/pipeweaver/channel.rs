// This struct is responsible for all the drawing, messaging, and updating of a channel on
// the Mix / Mix Create display

use crate::integrations::pipeweaver::ChannelType;
use crate::integrations::pipeweaver::helpers::{Mix, MuteTarget};
use crate::integrations::pipeweaver::layout::GradientDirection::{BottomToTop, TopToBottom};
use crate::integrations::pipeweaver::layout::*;
use anyhow::{Result, anyhow};
use beacn_lib::manager::DeviceType;
use enum_map::{EnumMap, enum_map};
use image::imageops::{crop, crop_imm};
use image::{ImageBuffer, Rgba, RgbaImage, load_from_memory};
use serde_json::Value;

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum ChannelChangedProperty {
    Title,
    Colour,

    Volumes(Mix),
    MuteState(MuteTarget),
}

#[allow(unused)]
pub(crate) struct ChannelRenderer {
    beacn_type: DeviceType,

    pub(crate) title: String,
    pub(crate) colour: Rgba<u8>,

    pub(crate) volumes: EnumMap<Mix, u8>,

    // Meter is the actual current value, target is how we're getting there
    pub(crate) meter: u8,
    pub(crate) meter_target: f32,

    pub(crate) channel_type: ChannelType,

    pub(crate) mute_states: EnumMap<MuteTarget, MuteState>,
}

pub(crate) struct MuteState {
    pub(crate) is_active: bool,
    pub(crate) is_mute_to_all: bool,
}

pub(crate) struct BeacnImage {
    pub(crate) position: Position,
    pub(crate) image: RgbaImage,
}

pub(crate) struct RawImage {
    pub(crate) position: Position,
    pub(crate) image: Vec<u8>,
}

// Some JSON reading Helpers..
fn get_str<'a>(value: &'a Value, pointer: &str) -> Result<&'a str> {
    value
        .pointer(pointer)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing or invalid string field at '{pointer}'"))
}

fn get_u8(value: &Value, pointer: &str) -> Result<u8> {
    value
        .pointer(pointer)
        .and_then(|v| v.as_u64())
        .map(|v| v as u8)
        .ok_or_else(|| anyhow!("Missing or invalid u8 field at '{pointer}'"))
}

fn get_array<'a>(value: &'a Value, pointer: &str) -> Result<&'a Vec<Value>> {
    value
        .pointer(pointer)
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("Missing or invalid array field at '{pointer}'"))
}

fn parse_colour(device: &Value) -> Result<Rgba<u8>> {
    let r = get_u8(device, "/description/colour/red")?;
    let g = get_u8(device, "/description/colour/green")?;
    let b = get_u8(device, "/description/colour/blue")?;
    Ok(Rgba([r, g, b, 255]))
}

struct ParsedSourceDevice {
    name: String,
    colour: Rgba<u8>,
    volume_a: u8,
    volume_b: u8,
    mute_a: bool,
    mute_b: bool,
    mute_a_to_all: bool,
    mute_b_to_all: bool,
}

impl ParsedSourceDevice {
    fn parse(device: &Value) -> Result<Self> {
        let name = get_str(device, "/description/name")?.to_owned();
        let colour = parse_colour(device)?;

        let volume_a = get_u8(device, "/volumes/volume/A")?;
        let volume_b = get_u8(device, "/volumes/volume/B")?;

        let mute_state = get_array(device, "/mute_states/mute_state")?;
        let mute_a = mute_state.iter().any(|v| v == "TargetA");
        let mute_b = mute_state.iter().any(|v| v == "TargetB");

        let mute_a_to_all = get_array(device, "/mute_states/mute_targets/TargetA")?.is_empty();
        let mute_b_to_all = get_array(device, "/mute_states/mute_targets/TargetB")?.is_empty();

        Ok(Self {
            name,
            colour,
            volume_a,
            volume_b,
            mute_a,
            mute_b,
            mute_a_to_all,
            mute_b_to_all,
        })
    }
}

struct ParsedTargetDevice {
    name: String,
    colour: Rgba<u8>,
    volume: u8,
    is_muted: bool,
}

impl ParsedTargetDevice {
    fn parse(device: &Value) -> Result<Self> {
        let name = get_str(device, "/description/name")?.to_owned();
        let colour = parse_colour(device)?;

        let volume = get_u8(device, "/volume")?;
        let is_muted = get_str(device, "/mute_state")? == "Muted";

        Ok(Self {
            name,
            colour,
            volume,
            is_muted,
        })
    }
}

impl ChannelRenderer {
    pub fn from_source_device_value(device: &Value) -> Result<Self> {
        let data = ParsedSourceDevice::parse(device)?;

        Ok(Self {
            beacn_type: DeviceType::BeacnMixCreate,
            title: data.name,
            colour: data.colour,
            volumes: enum_map! { Mix::A => data.volume_a, Mix::B => data.volume_b },
            meter: 0,
            meter_target: 0.0,
            channel_type: ChannelType::Source,
            mute_states: enum_map! {
                MuteTarget::TargetA => MuteState {
                    is_active: data.mute_a,
                    is_mute_to_all: data.mute_a_to_all,
                },
                MuteTarget::TargetB => MuteState {
                    is_active: data.mute_b,
                    is_mute_to_all: data.mute_b_to_all,
                }
            },
        })
    }

    pub fn from_target_device_value(device: &Value) -> Result<Self> {
        let data = ParsedTargetDevice::parse(device)?;

        Ok(Self {
            beacn_type: DeviceType::BeacnMixCreate,
            title: data.name,
            colour: data.colour,
            volumes: enum_map! { Mix::A => data.volume, Mix::B => 0 },
            meter: 0,
            meter_target: 0.0,
            channel_type: ChannelType::Target,
            mute_states: enum_map! {
                MuteTarget::TargetA => MuteState {
                    is_active: data.is_muted,
                    is_mute_to_all: true,
                },
                MuteTarget::TargetB => MuteState {
                    is_active: false,
                    is_mute_to_all: false,
                }
            },
        })
    }

    pub fn set_beacn_device(&mut self, device_type: DeviceType) {
        self.beacn_type = device_type;
    }

    pub fn update_from_source_device_value(
        &mut self,
        device: &Value,
    ) -> Result<Vec<ChannelChangedProperty>> {
        let data = ParsedSourceDevice::parse(device)?;
        let mut updates = vec![];

        if data.name != self.title {
            self.title = data.name;
            updates.push(ChannelChangedProperty::Title);
        }

        if self.colour != data.colour {
            self.colour = data.colour;
            updates.push(ChannelChangedProperty::Colour);
        }

        if data.volume_a != self.volumes[Mix::A] {
            self.volumes[Mix::A] = data.volume_a;
            updates.push(ChannelChangedProperty::Volumes(Mix::A));
        }
        if data.volume_b != self.volumes[Mix::B] {
            self.volumes[Mix::B] = data.volume_b;
            updates.push(ChannelChangedProperty::Volumes(Mix::B));
        }

        self.diff_mute_state(
            MuteTarget::TargetA,
            data.mute_a,
            data.mute_a_to_all,
            &mut updates,
        );
        self.diff_mute_state(
            MuteTarget::TargetB,
            data.mute_b,
            data.mute_b_to_all,
            &mut updates,
        );

        Ok(updates)
    }

    pub fn update_from_target_device_value(
        &mut self,
        device: &Value,
    ) -> Result<Vec<ChannelChangedProperty>> {
        let data = ParsedTargetDevice::parse(device)?;
        let mut updates = vec![];

        if data.name != self.title {
            self.title = data.name;
            updates.push(ChannelChangedProperty::Title);
        }

        if self.colour != data.colour {
            self.colour = data.colour;
            updates.push(ChannelChangedProperty::Colour);
        }

        // For targets, we have a single volume
        if self.volumes[Mix::A] != data.volume {
            self.volumes[Mix::A] = data.volume;
            updates.push(ChannelChangedProperty::Volumes(Mix::A));
        }

        self.diff_mute_state(MuteTarget::TargetA, data.is_muted, true, &mut updates);

        Ok(updates)
    }

    /// Updates a single mute target's state in place, pushing at most one
    /// `MuteState` update even if both `is_active` and `is_mute_to_all`
    /// changed at once.
    fn diff_mute_state(
        &mut self,
        target: MuteTarget,
        is_active: bool,
        is_mute_to_all: bool,
        updates: &mut Vec<ChannelChangedProperty>,
    ) {
        let state = &mut self.mute_states[target];
        let mut changed = false;

        if state.is_active != is_active {
            state.is_active = is_active;
            changed = true;
        }
        if state.is_mute_to_all != is_mute_to_all {
            state.is_mute_to_all = is_mute_to_all;
            changed = true;
        }

        if changed {
            updates.push(ChannelChangedProperty::MuteState(target));
        }
    }

    pub fn full_render(&self, active_mix: Mix) -> BeacnImage {
        // Firstly, lets grab some fixed dimensions
        let (w, h) = CHANNEL_DIMENSIONS;

        // Draw all the elements
        let mut base = ImageBuffer::from_pixel(w, h, BG_COLOUR);
        let content = self.draw_content_box();
        let header = self.draw_header();
        let header_bar = self.draw_bar(HEADER_BAR_POSITION);
        let mute_bar = self.draw_bar(MUTE_BAR_POSITION);
        let mute_bg = self.draw_mute_background();
        let dial = self.draw_volume(active_mix);
        let mute_a = self.draw_mute_box(MuteTarget::TargetA);

        // Composite all the elements together
        DrawingUtils::composite_from_pos(&mut base, &content.image, content.position);
        DrawingUtils::composite_from_pos(&mut base, &header.image, header.position);
        DrawingUtils::composite_from_pos(&mut base, &header_bar.image, header_bar.position);
        DrawingUtils::composite_from_pos(&mut base, &mute_bar.image, mute_bar.position);
        DrawingUtils::composite_from_pos(&mut base, &mute_bg.image, mute_bg.position);
        DrawingUtils::composite_from_pos(&mut base, &dial.image, dial.position);
        DrawingUtils::composite_from_pos(&mut base, &mute_a.image, mute_a.position);

        if self.beacn_type == DeviceType::BeacnMixCreate {
            let mute_b = self.draw_mute_box(MuteTarget::TargetB);
            DrawingUtils::composite_from_pos(&mut base, &mute_b.image, mute_b.position);
        }

        // Return the result
        BeacnImage {
            position: (0, 0),
            image: base,
        }
    }

    pub fn tick_meter(&mut self, delta_secs: f32) -> u8 {
        const DECAY: f32 = 3.0;
        const ATTACK: f32 = 10.0;

        let target = self.meter_target;
        let current = self.meter as f32;

        self.meter = if target >= current {
            let factor = 1.0 - (-ATTACK * delta_secs).exp();

            (current + (target - current) * factor).round() as u8
        } else {
            let factor = (-DECAY * delta_secs).exp();

            let next = (target + (current - target) * factor).round() as u8;

            // Prevent quantization lock
            if next >= self.meter && self.meter > 0 {
                self.meter - 1
            } else {
                next
            }
        };

        self.meter
    }

    pub fn get_volume(&self, mix: Mix) -> Result<RawImage> {
        let volume = self.volumes[mix];
        let meter = Self::scale_meter(self.volumes[mix], self.meter);
        let raw_image = DIAL_VOLUME_JPEG[mix]
            .get(&volume)
            .and_then(|m| m.get(&meter))
            .ok_or(anyhow!("Image Missing"))?;

        Ok(RawImage {
            position: VOLUME_POSITION,
            image: raw_image.clone(),
        })
    }

    pub fn draw_volume(&self, mix: Mix) -> BeacnImage {
        let volume = self.volumes[mix];
        let meter = Self::scale_meter(self.volumes[mix], self.meter);
        if let Some(jpeg_data) = DIAL_VOLUME_JPEG[mix]
            .get(&volume)
            .and_then(|m| m.get(&meter))
            && let Ok(img) = load_from_memory(jpeg_data)
        {
            return BeacnImage {
                position: VOLUME_POSITION,
                image: img.into_rgba8(),
            };
        }
        panic!("Unable to Load Volume Image for Mix: {mix:?}");
    }

    fn scale_meter(volume: u8, meter: u8) -> u8 {
        // Meter needs to be relative to the volume, so scale it.
        (meter as f32 / 100.0 * volume as f32).round() as u8
    }

    fn draw_content_box(&self) -> BeacnImage {
        let channel_inner = match self.channel_type {
            ChannelType::Source => match self.beacn_type {
                DeviceType::BeacnMixCreate => CHANNEL_INNER_DIMENSIONS,
                DeviceType::BeacnMix => CHANNEL_INNER_DIMENSIONS_MIX,
                _ => panic!("Bad Device Type"),
            },
            ChannelType::Target => CHANNEL_INNER_DIMENSIONS_MIX,
        };

        BeacnImage {
            position: CHANNEL_INNER_POSITION,
            image: DrawingUtils::draw_box(
                channel_inner.0,
                channel_inner.1,
                CHANNEL_INNER_BORDER,
                CHANNEL_INNER_RADIUS,
                CHANNEL_BORDER_COLOUR,
                BG_COLOUR,
                CHANNEL_INNER_COLOUR,
            ),
        }
    }

    pub fn draw_header(&self) -> BeacnImage {
        let mut colour = self.colour;
        colour[3] = 100;

        let (width, height) = HEADER_DIMENSIONS;
        let (text_width, text_height) = HEADER_TEXT_DIMENSIONS;
        let mut base = DrawingUtils::draw_gradient(width, height, colour, TopToBottom);
        let text = DrawingUtils::draw_text(
            self.title.to_string(),
            text_width,
            text_height,
            HEADER_FONT,
            HEADER_FONT_SIZE,
            TEXT_COLOUR,
            TextAlign::Center,
        );

        // Draw the text over the gradient
        DrawingUtils::composite_from(&mut base, &text, 0, 0);

        // Return it
        BeacnImage {
            position: HEADER_POSITION,
            image: base,
        }
    }

    fn draw_bar(&self, position: Position) -> BeacnImage {
        BeacnImage {
            position,
            image: ImageBuffer::from_pixel(BAR_DIMENSIONS.0, BAR_DIMENSIONS.1, self.colour),
        }
    }

    fn draw_mute_background(&self) -> BeacnImage {
        let (w, h) = MUTE_AREA_DIMENSIONS;
        let (m1, h1) = MUTE_AREA_DIMENSIONS_MIX;

        let mut colour = self.colour;
        colour[3] = 120;

        let mut gradient_base = DrawingUtils::draw_gradient(w, h, colour, BottomToTop);
        let gradient = match self.channel_type {
            ChannelType::Source => match self.beacn_type {
                DeviceType::BeacnMixCreate => gradient_base,
                DeviceType::BeacnMix => crop(&mut gradient_base, 0, 0, m1, h1).to_image(),
                _ => panic!("Bad Device Type"),
            },
            ChannelType::Target => crop(&mut gradient_base, 0, 0, m1, h1).to_image(),
        };

        BeacnImage {
            position: MUTE_AREA_POSITION,
            image: gradient,
        }
    }

    pub fn draw_mute_box(&self, target: MuteTarget) -> BeacnImage {
        // Ok, first we need the mute background
        let mut background = self.draw_mute_background().image;
        let text = match self.channel_type {
            ChannelType::Source => match self.mute_states[target].is_mute_to_all {
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

        let (colour, icon) = match self.mute_states[target].is_active {
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
            colour,
        );

        let relative_position = match target {
            MuteTarget::TargetA => MUTE_LOCAL_POSITION_A,
            MuteTarget::TargetB => MUTE_LOCAL_POSITION_B,
        };
        let (x, y) = relative_position;

        // Draw the box onto the background
        DrawingUtils::composite_from(&mut background, &mute_box, x, y);

        // The text size needs to be shrunk based on the icon size
        let (mut text_width, text_height) = MUTE_TEXT_DIMENSIONS;
        text_width = text_width - icon.width() - (ICON_MARGIN * 2);

        // Draw the text
        let text = DrawingUtils::draw_text(
            text.to_string(),
            text_width,
            text_height,
            MUTE_FONT,
            MUTE_FONT_SIZE,
            TEXT_COLOUR,
            TextAlign::Left,
        );

        let (_, h) = MUTE_BUTTON_DIMENSIONS;
        let middle = h / 2;
        let text_middle = text.height() / 2;
        let icon_middle = icon.height() / 2;

        let text_y = middle - text_middle + y + border_draw.0;
        let icon_y = middle - icon_middle + y + border_draw.0;

        let text_x = icon.width() + (ICON_MARGIN * 2);
        let icon_x = ICON_MARGIN;

        // Find the Middle position
        DrawingUtils::composite_from(&mut background, &text, text_x, text_y);
        DrawingUtils::composite_from(&mut background, icon, icon_x, icon_y);

        // Grab the specific area from the Mute Box
        let cropped = crop_imm(&background, x, y, width, height).to_image();

        let position = match target {
            MuteTarget::TargetA => MUTE_POSITION_A,
            MuteTarget::TargetB => MUTE_POSITION_B,
        };

        BeacnImage {
            image: cropped,
            position,
        }
    }
}
