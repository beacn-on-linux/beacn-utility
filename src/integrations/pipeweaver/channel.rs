// This struct is responsible for all the drawing, messaging, and updating of a channel on
// the Mix / Mix Create display

use crate::integrations::pipeweaver::layout::GradientDirection::{BottomToTop, TopToBottom};
use crate::integrations::pipeweaver::layout::*;
use anyhow::{Result, anyhow};
use enum_map::{EnumMap, enum_map};
use image::imageops::crop_imm;
use image::{ImageBuffer, Rgba, RgbaImage, load_from_memory};
use pipeweaver_profile::{
    DeviceDescription, MuteStates, PhysicalSourceDevice, VirtualSourceDevice, Volumes,
};
use pipeweaver_shared::{Mix, MuteTarget};
use strum::IntoEnumIterator;

// This trait is primarily here to ease the difference between a Physical and Virtual Source. In
// the context of this app, these see identical usage.
pub trait SourceDevice {
    fn description(&self) -> &DeviceDescription;
    fn volumes(&self) -> &Volumes;
    fn mute_states(&self) -> &MuteStates;
}

impl SourceDevice for PhysicalSourceDevice {
    fn description(&self) -> &DeviceDescription {
        &self.description
    }
    fn volumes(&self) -> &Volumes {
        &self.volumes
    }
    fn mute_states(&self) -> &MuteStates {
        &self.mute_states
    }
}

impl SourceDevice for VirtualSourceDevice {
    fn description(&self) -> &DeviceDescription {
        &self.description
    }
    fn volumes(&self) -> &Volumes {
        &self.volumes
    }
    fn mute_states(&self) -> &MuteStates {
        &self.mute_states
    }
}

pub(crate) trait UpdateFrom<T> {
    fn update_from(&mut self, value: T) -> Vec<ChannelChangedProperty>;
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum ChannelChangedProperty {
    Title,
    Colour,

    Volumes(Mix),
    MuteState(MuteTarget),
}

#[allow(unused)]
pub(crate) struct ChannelRenderer {
    pub(crate) title: String,
    pub(crate) colour: Rgba<u8>,

    pub(crate) volumes: EnumMap<Mix, u8>,
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
    fn from_source_device(device: &impl SourceDevice) -> Self {
        let desc = device.description();
        let vols = device.volumes();
        let mutes = device.mute_states();

        Self {
            title: desc.name.clone(),
            colour: Rgba([desc.colour.red, desc.colour.green, desc.colour.blue, 255]),
            volumes: vols.volume,
            mute_states: enum_map! {
                MuteTarget::TargetA => MuteState {
                    is_active: mutes.mute_state.contains(&MuteTarget::TargetA),
                    is_mute_to_all: mutes.mute_targets[MuteTarget::TargetA].is_empty(),
                },
                MuteTarget::TargetB => MuteState {
                    is_active: mutes.mute_state.contains(&MuteTarget::TargetB),
                    is_mute_to_all: mutes.mute_targets[MuteTarget::TargetB].is_empty(),
                }
            },
        }
    }

    pub fn update_from_device(
        &mut self,
        device: &impl SourceDevice,
    ) -> Vec<ChannelChangedProperty> {
        let desc = device.description();
        let vols = device.volumes();
        let mutes = device.mute_states();

        let mut updates = vec![];
        if desc.name != self.title {
            self.title = desc.name.clone();
            updates.push(ChannelChangedProperty::Title);
        }
        let colour = Rgba([desc.colour.red, desc.colour.green, desc.colour.blue, 255]);
        if self.colour != colour {
            self.colour = colour;
            updates.push(ChannelChangedProperty::Colour);
        }

        for mix in Mix::iter() {
            if vols.volume[mix] != self.volumes[mix] {
                self.volumes[mix] = vols.volume[mix];
                updates.push(ChannelChangedProperty::Volumes(mix));
            }
        }

        for target in MuteTarget::iter() {
            let mute_active = mutes.mute_state.contains(&target);
            if mute_active != self.mute_states[target].is_active {
                self.mute_states[target].is_active = mute_active;
                updates.push(ChannelChangedProperty::MuteState(target));
            }
        }

        for target in MuteTarget::iter() {
            let is_mute_to_all = mutes.mute_targets[target].is_empty();
            if is_mute_to_all != self.mute_states[target].is_mute_to_all {
                self.mute_states[target].is_mute_to_all = is_mute_to_all;
                if !updates.contains(&ChannelChangedProperty::MuteState(target)) {
                    updates.push(ChannelChangedProperty::MuteState(target));
                }
            }
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
        let mute_b = self.draw_mute_box(MuteTarget::TargetB);

        // Composite all the elements together
        DrawingUtils::composite_from_pos(&mut base, &content.image, content.position);
        DrawingUtils::composite_from_pos(&mut base, &header.image, header.position);
        DrawingUtils::composite_from_pos(&mut base, &header_bar.image, header_bar.position);
        DrawingUtils::composite_from_pos(&mut base, &mute_bar.image, mute_bar.position);
        DrawingUtils::composite_from_pos(&mut base, &mute_bg.image, mute_bg.position);
        DrawingUtils::composite_from_pos(&mut base, &dial.image, dial.position);
        DrawingUtils::composite_from_pos(&mut base, &mute_a.image, mute_a.position);
        DrawingUtils::composite_from_pos(&mut base, &mute_b.image, mute_b.position);

        // Return the result
        BeacnImage {
            position: POSITION_ROOT,
            image: base,
        }
    }

    pub fn get_volume(&self, mix: Mix) -> Result<RawImage> {
        let volume = self.volumes[mix];
        let raw_image = DIAL_VOLUME_JPEG[mix]
            .get(&volume)
            .ok_or(anyhow!("Image Missing"))?;

        Ok(RawImage {
            position: VOLUME_POSITION,
            image: raw_image.clone(),
        })
    }

    pub fn draw_volume(&self, mix: Mix) -> BeacnImage {
        let volume = self.volumes[mix];
        if let Some(jpeg_data) = DIAL_VOLUME_JPEG[mix].get(&volume)
            && let Ok(img) = load_from_memory(jpeg_data)
        {
            return BeacnImage {
                position: VOLUME_POSITION,
                image: img.into_rgba8(),
            };
        }
        panic!("Unable to Load Volume Image for Mix: {mix:?}");
    }

    fn draw_content_box(&self) -> BeacnImage {
        BeacnImage {
            position: CHANNEL_INNER_POSITION,
            image: DrawingUtils::draw_box(
                CHANNEL_INNER_DIMENSIONS.0,
                CHANNEL_INNER_DIMENSIONS.1,
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

        let mut colour = self.colour;
        colour[3] = 120;

        BeacnImage {
            position: MUTE_AREA_POSITION,
            image: DrawingUtils::draw_gradient(w, h, colour, BottomToTop),
        }
    }

    pub fn draw_mute_box(&self, target: MuteTarget) -> BeacnImage {
        // Ok, first we need the mute background
        let mut background = self.draw_mute_background().image;
        let text = match self.mute_states[target].is_mute_to_all {
            true => "Mute to All",
            false => "Mute To...",
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

impl From<PhysicalSourceDevice> for ChannelRenderer {
    fn from(value: PhysicalSourceDevice) -> Self {
        Self::from_source_device(&value)
    }
}

impl UpdateFrom<PhysicalSourceDevice> for ChannelRenderer {
    fn update_from(&mut self, value: PhysicalSourceDevice) -> Vec<ChannelChangedProperty> {
        self.update_from_device(&value)
    }
}

impl From<VirtualSourceDevice> for ChannelRenderer {
    fn from(value: VirtualSourceDevice) -> Self {
        Self::from_source_device(&value)
    }
}

impl UpdateFrom<VirtualSourceDevice> for ChannelRenderer {
    fn update_from(&mut self, value: VirtualSourceDevice) -> Vec<ChannelChangedProperty> {
        self.update_from_device(&value)
    }
}
