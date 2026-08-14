// This struct is responsible for all the drawing, messaging, and updating of a channel on
// the Mix / Mix Create display

use crate::integrations::pipeweaver::ChannelType;
use crate::integrations::pipeweaver::layout::GradientDirection::{BottomToTop, TopToBottom};
use crate::integrations::pipeweaver::layout::*;
use anyhow::{Result, anyhow};
use beacn_lib::manager::DeviceType;
use enum_map::{EnumMap, enum_map};
use image::imageops::{crop, crop_imm};
use image::{ImageBuffer, Rgba, RgbaImage, load_from_memory};
//use pipeweaver_shared::{Mix, MuteTarget};
use crate::integrations::pipeweaver::helpers::{Mix, MuteTarget};
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

impl ChannelRenderer {
    pub fn from_source_device_value(device: &Value) -> Self {
        // Description Reading
        let id = device["description"]["id"].as_str().unwrap();
        let name = device["description"]["name"].as_str().unwrap();
        let colour_r = device["description"]["colour"]["red"].as_u64().unwrap() as u8;
        let colour_g = device["description"]["colour"]["green"].as_u64().unwrap() as u8;
        let colour_b = device["description"]["colour"]["blue"].as_u64().unwrap() as u8;

        // Volume Reading
        let volume_mix_a = device["volumes"]["volume"]["A"].as_u64().unwrap() as u8;
        let volume_mix_b = device["volumes"]["volume"]["B"].as_u64().unwrap() as u8;

        // Mute States
        let mute = &device["mute_states"];
        let mute_state = mute["mute_state"].as_array().unwrap();
        let target_a = mute_state.iter().any(|v| v == "TargetA");
        let target_b = mute_state.iter().any(|v| v == "TargetB");

        let mute_targets = &mute["mute_targets"];
        let target_a_to_all = mute_targets["TargetA"].as_array().unwrap().is_empty();
        let target_b_to_all = mute_targets["TargetB"].as_array().unwrap().is_empty();

        Self {
            beacn_type: DeviceType::BeacnMixCreate,
            title: name.to_owned(),
            colour: Rgba([colour_r, colour_g, colour_b, 255]),
            volumes: enum_map! { Mix::A => volume_mix_a, Mix::B => volume_mix_b },
            meter: 0,
            meter_target: 0.0,
            channel_type: ChannelType::Source,
            mute_states: enum_map! {
                MuteTarget::TargetA => MuteState {
                    is_active: target_a,
                    is_mute_to_all: target_a_to_all,
                },
                MuteTarget::TargetB => MuteState {
                    is_active: target_b,
                    is_mute_to_all: target_b_to_all,
                }
            },
        }
    }

    pub fn from_target_device_value(device: &Value) -> Self {
        // Description Reading
        let id = device["description"]["id"].as_str().unwrap();
        let name = device["description"]["name"].as_str().unwrap();
        let colour_r = device["description"]["colour"]["red"].as_u64().unwrap() as u8;
        let colour_g = device["description"]["colour"]["green"].as_u64().unwrap() as u8;
        let colour_b = device["description"]["colour"]["blue"].as_u64().unwrap() as u8;

        // Volume Reading
        let volume = device["volume"].as_u64().unwrap() as u8;
        let is_muted = device["mute_state"].as_str().unwrap() == "Muted";

        Self {
            beacn_type: DeviceType::BeacnMixCreate,
            title: name.to_owned(),
            colour: Rgba([colour_r, colour_g, colour_b, 255]),
            volumes: enum_map! { Mix::A => volume, Mix::B => 0 },
            meter: 0,
            meter_target: 0.0,
            channel_type: ChannelType::Target,
            mute_states: enum_map! {
                MuteTarget::TargetA => MuteState {
                    is_active: is_muted,
                    is_mute_to_all: true,
                },
                MuteTarget::TargetB => MuteState {
                    is_active: false,
                    is_mute_to_all: false,
                }
            },
        }
    }

    pub fn set_beacn_device(&mut self, device_type: DeviceType) {
        self.beacn_type = device_type;
    }

    pub fn update_from_source_device_value(
        &mut self,
        device: &Value,
    ) -> Vec<ChannelChangedProperty> {
        // Description Reading
        let id = device["description"]["id"].as_str().unwrap();
        let name = device["description"]["name"].as_str().unwrap();
        let colour_r = device["description"]["colour"]["red"].as_u64().unwrap() as u8;
        let colour_g = device["description"]["colour"]["green"].as_u64().unwrap() as u8;
        let colour_b = device["description"]["colour"]["blue"].as_u64().unwrap() as u8;

        // Volume Reading
        let volume_mix_a = device["volumes"]["volume"]["A"].as_u64().unwrap() as u8;
        let volume_mix_b = device["volumes"]["volume"]["B"].as_u64().unwrap() as u8;

        // Mute States
        let mute = &device["mute_states"];
        let mute_state = mute["mute_state"].as_array().unwrap();
        let target_a = mute_state.iter().any(|v| v == "TargetA");
        let target_b = mute_state.iter().any(|v| v == "TargetB");

        let mute_targets = &mute["mute_targets"];
        let target_a_to_all = mute_targets["TargetA"].as_array().unwrap().is_empty();
        let target_b_to_all = mute_targets["TargetB"].as_array().unwrap().is_empty();

        let mut updates = vec![];
        if name != self.title {
            self.title = name.to_owned();
            updates.push(ChannelChangedProperty::Title);
        }

        let colour = Rgba([colour_r, colour_g, colour_b, 255]);
        if self.colour != colour {
            self.colour = colour;
            updates.push(ChannelChangedProperty::Colour);
        }

        if volume_mix_a != self.volumes[Mix::A] {
            self.volumes[Mix::A] = volume_mix_a;
            updates.push(ChannelChangedProperty::Volumes(Mix::A));
        }
        if volume_mix_b != self.volumes[Mix::B] {
            self.volumes[Mix::B] = volume_mix_b;
            updates.push(ChannelChangedProperty::Volumes(Mix::B));
        }

        if target_a != self.mute_states[MuteTarget::TargetA].is_active {
            self.mute_states[MuteTarget::TargetA].is_active = target_a;
            updates.push(ChannelChangedProperty::MuteState(MuteTarget::TargetA));
        }
        if target_b != self.mute_states[MuteTarget::TargetB].is_active {
            self.mute_states[MuteTarget::TargetB].is_active = target_b;
            updates.push(ChannelChangedProperty::MuteState(MuteTarget::TargetB));
        }

        if target_a_to_all != self.mute_states[MuteTarget::TargetA].is_mute_to_all {
            self.mute_states[MuteTarget::TargetA].is_mute_to_all = target_a_to_all;
            if !updates.contains(&ChannelChangedProperty::MuteState(MuteTarget::TargetA)) {
                updates.push(ChannelChangedProperty::MuteState(MuteTarget::TargetA));
            }
        }
        if target_b_to_all != self.mute_states[MuteTarget::TargetB].is_mute_to_all {
            self.mute_states[MuteTarget::TargetB].is_mute_to_all = target_b_to_all;
            if !updates.contains(&ChannelChangedProperty::MuteState(MuteTarget::TargetB)) {
                updates.push(ChannelChangedProperty::MuteState(MuteTarget::TargetB));
            }
        }

        updates
    }

    pub fn update_from_target_device_value(
        &mut self,
        device: &Value,
    ) -> Vec<ChannelChangedProperty> {
        // Description Reading
        let id = device["description"]["id"].as_str().unwrap();
        let name = device["description"]["name"].as_str().unwrap();
        let colour_r = device["description"]["colour"]["red"].as_u64().unwrap() as u8;
        let colour_g = device["description"]["colour"]["green"].as_u64().unwrap() as u8;
        let colour_b = device["description"]["colour"]["blue"].as_u64().unwrap() as u8;

        // Volume Reading
        let volume = device["volume"].as_u64().unwrap() as u8;
        let is_muted = device["mute_state"].as_str().unwrap() == "Muted";

        let mut updates = vec![];
        if name != self.title {
            self.title = name.to_owned();
            updates.push(ChannelChangedProperty::Title);
        }

        let colour = Rgba([colour_r, colour_g, colour_b, 255]);
        if self.colour != colour {
            self.colour = colour;
            updates.push(ChannelChangedProperty::Colour);
        }

        // For targets, we have a single volume
        if self.volumes[Mix::A] != volume {
            self.volumes[Mix::A] = volume;
            updates.push(ChannelChangedProperty::Volumes(Mix::A));
        }

        if self.mute_states[MuteTarget::TargetA].is_active != is_muted {
            self.mute_states[MuteTarget::TargetA].is_active = is_muted;
            updates.push(ChannelChangedProperty::MuteState(MuteTarget::TargetA));
        }

        updates
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
