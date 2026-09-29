// This struct is responsible for all the drawing, messaging, and updating of a channel on
// the Mix / Mix Create display

use crate::integrations::pipeweaver::ChannelType;
use crate::integrations::pipeweaver::helpers::{Mix, MuteTarget};
use crate::integrations::pipeweaver::layout::DIAL_VOLUME_JPEG;
use crate::integrations::pipeweaver::pieces::{Jpeg, MuteFace, PieceKey, Plate, Rgb3};
use anyhow::{Result, anyhow};
use beacn_lib::manager::DeviceType;
use enum_map::{EnumMap, enum_map};
use image::Rgba;
use serde_json::Value;

/// Used for Meter Animation
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MeterSettings {
    pub fps: u32,
    pub attack: f32,
    pub decay: f32,
}

impl Default for MeterSettings {
    fn default() -> Self {
        // These feel about right :D
        Self {
            fps: 30,
            attack: 10.0,
            decay: 3.0,
        }
    }
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
    beacn_type: DeviceType,

    pub(crate) title: String,
    pub(crate) colour: Rgba<u8>,

    pub(crate) volumes: EnumMap<Mix, u8>,

    // The target is the latest value from the daemon, the value is the smoothed float we're
    // animating towards it, and the meter is that value rounded to what we actually draw.
    pub(crate) meter: u8,
    pub(crate) meter_target: f32,
    meter_value: f32,

    pub(crate) channel_type: ChannelType,

    pub(crate) mute_states: EnumMap<MuteTarget, MuteState>,
}

pub(crate) struct MuteState {
    pub(crate) is_active: bool,
    pub(crate) is_mute_to_all: bool,
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
            meter_value: 0.0,
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
            meter_value: 0.0,
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

    /// Tick the meter towards its target
    pub(crate) fn tick_meter(&mut self, delta_time: f32, settings: &MeterSettings) {
        let target = self.meter_target.clamp(0.0, 100.0);
        let current = self.meter_value;

        let rate = if target >= current {
            settings.attack
        } else {
            settings.decay
        };

        let factor = 1.0 - (-rate.max(0.0) * delta_time).exp();
        let mut next = current + (target - current) * factor;

        // An exponential never quite arrives, so close the last sliver
        if (target - next).abs() < 0.05 {
            next = target;
        }

        self.meter_value = next;
        self.meter = next.round().clamp(0.0, 100.0) as u8;
    }

    /// True once the meter hits its target value
    pub(crate) fn meter_settled(&self) -> bool {
        self.meter_value == self.meter_target.clamp(0.0, 100.0)
    }

    /// The pre-generated dial JPEG for the current volume / meter
    pub fn get_volume(&self, mix: Mix) -> Result<Jpeg> {
        let (_, volume, meter) = self.dial_key(mix);
        DIAL_VOLUME_JPEG[mix]
            .get(&volume)
            .and_then(|m| m.get(&meter))
            .cloned()
            .ok_or(anyhow!("Image Missing"))
    }

    /// Identifies exactly which dial image is showing, so we can skip redundant redraws
    pub(crate) fn dial_key(&self, mix: Mix) -> (Mix, u8, u8) {
        let volume = self.volumes[mix];
        (mix, volume, Self::scale_meter(volume, self.meter))
    }

    fn scale_meter(volume: u8, meter: u8) -> u8 {
        // Meter needs to be relative to the volume, so scale it.
        (meter as f32 / 100.0 * volume as f32).round() as u8
    }

    // ------
    // Some General Cache Key helpers, used when verifying whether a redraw is needed
    // ------

    /// Only Sources on a Mix Create have a second mute button (and so the taller layout)
    fn has_second_mute(&self) -> bool {
        self.channel_type == ChannelType::Source && self.beacn_type == DeviceType::BeacnMixCreate
    }

    pub(crate) fn plate(&self) -> Plate {
        if self.has_second_mute() {
            Plate::Tall
        } else {
            Plate::Short
        }
    }

    fn rgb(&self) -> Rgb3 {
        [self.colour[0], self.colour[1], self.colour[2]]
    }

    fn face(&self, target: MuteTarget, active: bool) -> MuteFace {
        MuteFace {
            active,
            to_all: self.mute_states[target].is_mute_to_all,
        }
    }

    pub(crate) fn top_key(&self) -> PieceKey {
        PieceKey::Top {
            colour: self.rgb(),
            title: self.title.clone(),
        }
    }

    fn mute_key_with(&self, a_active: bool, b_active: bool) -> PieceKey {
        PieceKey::Mute {
            kind: self.channel_type,
            colour: self.rgb(),
            a: self.face(MuteTarget::TargetA, a_active),
            b: self
                .has_second_mute()
                .then(|| self.face(MuteTarget::TargetB, b_active)),
        }
    }

    pub(crate) fn mute_key(&self) -> PieceKey {
        self.mute_key_with(
            self.mute_states[MuteTarget::TargetA].is_active,
            self.mute_states[MuteTarget::TargetB].is_active,
        )
    }

    /// Every piece this channel can show without its colour, title or mute targets changing,
    /// which is what we pre-render so that toggling a mute never has to draw anything.
    pub(crate) fn all_keys(&self) -> Vec<PieceKey> {
        let mut keys = vec![self.top_key()];
        for a in [false, true] {
            if self.has_second_mute() {
                for b in [false, true] {
                    keys.push(self.mute_key_with(a, b));
                }
            } else {
                keys.push(self.mute_key_with(a, false));
            }
        }
        keys
    }
}
