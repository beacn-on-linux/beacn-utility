use anyhow::Result;

use beacn_mic_lib::device::BeacnMic;
use beacn_mic_lib::manager::DeviceType;
use beacn_mic_lib::messages::bass_enhancement::BassPreset;
use beacn_mic_lib::messages::compressor::CompressorMode;
use beacn_mic_lib::messages::equaliser::{EQBand, EQBandType, EQMode};
use beacn_mic_lib::messages::expander::ExpanderMode;
use beacn_mic_lib::messages::headphone_equaliser::HPEQType;
use beacn_mic_lib::messages::headphones::HeadphoneTypes;
use beacn_mic_lib::messages::lighting::{
    LightingMeterSource, LightingMode, LightingMuteMode, LightingSuspendMode,
};
use beacn_mic_lib::messages::suppressor::SuppressorStyle;
use beacn_mic_lib::messages::Message;
use beacn_mic_lib::types::ToInner;
use enum_map::EnumMap;

use beacn_mic_lib::messages::bass_enhancement::BassEnhancement as MicBaseEnhancement;
use beacn_mic_lib::messages::compressor::Compressor as MicCompressor;
use beacn_mic_lib::messages::deesser::DeEsser as MicDeEsser;
use beacn_mic_lib::messages::equaliser::Equaliser as MicEqualiser;
use beacn_mic_lib::messages::exciter::Exciter as MicExciter;
use beacn_mic_lib::messages::expander::Expander as MicExpander;
use beacn_mic_lib::messages::headphone_equaliser::HeadphoneEQ as MicHeadphoneEQ;
use beacn_mic_lib::messages::headphones::Headphones as MicHeadphones;
use beacn_mic_lib::messages::lighting::Lighting as MicLighting;
use beacn_mic_lib::messages::subwoofer::Subwoofer as MicSubwoofer;
use beacn_mic_lib::messages::suppressor::Suppressor as MicSuppressor;
use beacn_mic_lib::messages::mic_setup::MicSetup as MicMicSetup;

type Rgb = [u8; 3];

#[derive(Debug, Default, Copy, Clone)]
pub struct BeacnMicState {
    pub device_type: DeviceType,

    pub headphones: Headphones,
    pub lighting: Lighting,
    pub equaliser: Equaliser,
    pub headphone_eq: HeadphoneEq,
    pub bass_enhancement: BassEnhancement,
    pub compressor: Compressor,
    pub de_esser: DeEsser,
    pub exciter: Exciter,
    pub expander: Expander,
    pub suppressor: Suppressor,
    pub mic_setup: MicSetup,
    pub subwoofer: Subwoofer,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Headphones {
    pub level: f32,       // [-70.0...=0.0]
    pub mic_monitor: f32, // [-100.0..=6.0]
    pub linked: bool,
    pub output_gain: f32, // f32[0.0..=12.0]
    pub headphone_type: HeadphoneTypes,
    pub fx_enabled: bool,
    pub studio_driverless: bool,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Lighting {
    pub mode: LightingMode,
    pub colour1: Rgb,
    pub colour2: Rgb,
    pub speed: i32,
    pub brightness: i32,
    pub source: LightingMeterSource,
    pub sensitivity: f32,
    pub mute_mode: LightingMuteMode,
    pub mute_colour: Rgb,
    pub suspend_mode: LightingSuspendMode,
    pub suspend_brightness: u32,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Equaliser {
    pub mode: EQMode,
    pub bands: EnumMap<EQMode, EnumMap<EQBand, EqualiserBand>>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct EqualiserBand {
    pub enabled: bool,
    pub band_type: EQBandType,
    pub frequency: u32, // [0..=20000]Hz
    pub gain: f32,      // [-12.0..=12.0]dB
    pub q: f32,         // [0.1..=10.0]
}

#[derive(Debug, Default, Copy, Clone)]
pub struct HeadphoneEq {
    pub eq: EnumMap<HPEQType, HeadphoneEQValue>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct HeadphoneEQValue {
    pub enabled: bool,
    pub amount: f32, // [-12.0..=12.0]
}

// We don't need any additional values here, when the preset changes we just
// grab and apply the values from the lib
#[derive(Debug, Default, Copy, Clone)]
pub struct BassEnhancement {
    pub enabled: bool,
    pub preset: BassPreset,
    pub amount: i8, // [0..=10]
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Compressor {
    pub mode: CompressorMode,
    pub values: EnumMap<CompressorMode, CompressorValue>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct CompressorValue {
    pub enabled: bool,
    pub attack: u16,   // [1..=2000]ms
    pub release: u16,  // [1..=2000]ms
    pub threshold: i8, // [-90..=0]db
    pub ratio: f32,    // [0.0..=10.0]:1
    pub makeup: f32,   // [0.0..=12.0]dB
}

#[derive(Debug, Default, Copy, Clone)]
pub struct DeEsser {
    pub enabled: bool,
    pub amount: u8, // [0..=100]
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Exciter {
    pub enabled: bool,
    pub amount: u8, // [0..=100]
    pub freq: u16,  // [600..=5000]
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Expander {
    pub mode: ExpanderMode,
    pub values: EnumMap<ExpanderMode, ExpanderValue>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct ExpanderValue {
    pub enabled: bool,
    pub attack: u16,   // [0..=2000]ms
    pub release: u16,  // [0..=2000]ms
    pub threshold: i8, // [-90..=0]dB
    pub ratio: f32,    // [0.0..=10.0]:1
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Suppressor {
    pub enabled: bool,
    pub amount: u8, // [0..=100]%
    pub style: SuppressorStyle,
    pub sense: u8, // [0..=100]%
}

#[derive(Debug, Default, Copy, Clone)]
pub struct MicSetup {
    pub gain: u8,      // [3..=20]dB
    pub phantom: bool, // Phantom Power (Studio)
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Subwoofer {
    pub enabled: bool,
    pub amount: u8, // [0..=10]
}

impl BeacnMicState {
    pub fn load_settings(mic: &BeacnMic, device_type: DeviceType) -> Result<Self> {
        let mut state = Self::default();
        state.device_type = device_type;

        // Ok, grab all the variables from the mic
        let messages = Message::generate_fetch_message(device_type);
        for message in messages {
            let value = mic.fetch_value(message)?;

            match value {
                Message::BassEnhancement(b) => match b {
                    MicBaseEnhancement::Enabled(v) => state.bass_enhancement.enabled = v,
                    MicBaseEnhancement::Preset(v) => state.bass_enhancement.preset = v,
                    MicBaseEnhancement::Amount(v) => {
                        state.bass_enhancement.amount = v.to_inner() as i8
                    }
                    _ => {}
                },
                Message::Compressor(c) => match c {
                    MicCompressor::Mode(mode) => state.compressor.mode = mode,
                    MicCompressor::Attack(mode, value) => {
                        state.compressor.values[mode].attack = value.to_inner() as u16
                    }
                    MicCompressor::Release(mode, value) => {
                        state.compressor.values[mode].release = value.to_inner() as u16
                    }
                    MicCompressor::Threshold(mode, value) => {
                        state.compressor.values[mode].threshold = value.to_inner() as i8
                    }
                    MicCompressor::Ratio(mode, value) => {
                        state.compressor.values[mode].ratio = value.to_inner()
                    }
                    MicCompressor::MakeupGain(mode, value) => {
                        state.compressor.values[mode].makeup = value.to_inner()
                    }
                    MicCompressor::Enabled(mode, value) => {
                        state.compressor.values[mode].enabled = value
                    }
                    _ => {}
                },
                Message::DeEsser(d) => match d {
                    MicDeEsser::Amount(value) => state.de_esser.amount = value.to_inner() as u8,
                    MicDeEsser::Enabled(value) => state.de_esser.enabled = value,
                    _ => {}
                },
                Message::Equaliser(e) => match e {
                    MicEqualiser::Mode(mode) => state.equaliser.mode = mode,
                    MicEqualiser::Type(mode, band, value) => {
                        state.equaliser.bands[mode][band].band_type = value
                    }
                    MicEqualiser::Gain(mode, band, value) => {
                        state.equaliser.bands[mode][band].gain = value.to_inner()
                    }
                    MicEqualiser::Frequency(mode, band, value) => {
                        state.equaliser.bands[mode][band].frequency = value.to_inner() as u32
                    }
                    MicEqualiser::Q(mode, band, value) => {
                        state.equaliser.bands[mode][band].q = value.to_inner()
                    }
                    MicEqualiser::Enabled(mode, band, value) => {
                        state.equaliser.bands[mode][band].enabled = value
                    }
                    _ => {}
                },
                Message::Exciter(e) => match e {
                    MicExciter::Amount(value) => state.exciter.amount = value.to_inner() as u8,
                    MicExciter::Frequency(value) => state.exciter.freq = value.to_inner() as u16,
                    MicExciter::Enabled(value) => state.exciter.enabled = value,
                    _ => {}
                },
                Message::Expander(e) => match e {
                    MicExpander::Mode(mode) => state.expander.mode = mode,
                    MicExpander::Threshold(mode, value) => {
                        state.expander.values[mode].threshold = value.to_inner() as i8
                    }
                    MicExpander::Ratio(mode, value) => {
                        state.expander.values[mode].ratio = value.to_inner()
                    }
                    MicExpander::Enabled(mode, value) => {
                        state.expander.values[mode].enabled = value
                    }
                    MicExpander::Attack(mode, value) => {
                        state.expander.values[mode].attack = value.to_inner() as u16
                    }
                    MicExpander::Release(mode, value) => {
                        state.expander.values[mode].release = value.to_inner() as u16
                    }
                    _ => {}
                },
                Message::HeadphoneEQ(h) => match h {
                    MicHeadphoneEQ::Amount(eq_type, value) => {
                        state.headphone_eq.eq[eq_type].amount = value.to_inner()
                    }
                    MicHeadphoneEQ::Enabled(eq_type, value) => {
                        state.headphone_eq.eq[eq_type].enabled = value
                    }
                    _ => {}
                },
                Message::Headphones(h) => match h {
                    MicHeadphones::HeadphoneLevel(v) => state.headphones.level = v.to_inner(),
                    MicHeadphones::MicMonitor(v) => state.headphones.mic_monitor = v.to_inner(),
                    MicHeadphones::StudioMicMonitor(v) => {
                        state.headphones.mic_monitor = v.to_inner()
                    }
                    MicHeadphones::MicChannelsLinked(b) => state.headphones.linked = b,
                    MicHeadphones::StudioChannelsLinked(b) => state.headphones.linked = b,
                    MicHeadphones::MicOutputGain(v) => state.headphones.output_gain = v.to_inner(),
                    MicHeadphones::HeadphoneType(t) => state.headphones.headphone_type = t,
                    MicHeadphones::FXEnabled(t) => state.headphones.fx_enabled = t,
                    MicHeadphones::StudioDriverless(t) => state.headphones.studio_driverless = t,
                    _ => {}
                },
                Message::Lighting(l) => match l {
                    MicLighting::Mode(m) => state.lighting.mode = m,
                    MicLighting::Colour1(c) => state.lighting.colour1 = [c.red, c.green, c.blue],
                    MicLighting::Colour2(c) => state.lighting.colour2 = [c.red, c.green, c.blue],
                    MicLighting::Speed(v) => state.lighting.speed = v.to_inner(),
                    MicLighting::Brightness(v) => state.lighting.brightness = v.to_inner(),
                    MicLighting::MeterSource(v) => state.lighting.source = v,
                    MicLighting::MeterSensitivity(s) => state.lighting.sensitivity = s.to_inner(),
                    MicLighting::MuteMode(m) => state.lighting.mute_mode = m,
                    MicLighting::MuteColour(c) => {
                        state.lighting.mute_colour = [c.red, c.green, c.blue]
                    }
                    MicLighting::SuspendMode(m) => state.lighting.suspend_mode = m,
                    MicLighting::SuspendBrightness(b) => {
                        state.lighting.suspend_brightness = b.to_inner()
                    }
                    _ => {}
                },
                Message::MicSetup(m) => match m {
                    MicMicSetup::MicGain(g) => state.mic_setup.gain = g.to_inner() as u8,
                    MicMicSetup::StudioMicGain(g) => state.mic_setup.gain = g.to_inner() as u8,
                    MicMicSetup::StudioPhantomPower(p) => state.mic_setup.phantom = p,
                    _ => {}
                },
                Message::Subwoofer(s) => match s {
                    MicSubwoofer::Enabled(e) => state.subwoofer.enabled = e,
                    MicSubwoofer::Amount(a) => state.subwoofer.amount = a.to_inner() as u8,
                    _ => {}
                },
                Message::Suppressor(s) => match s {
                    MicSuppressor::Enabled(e) => state.suppressor.enabled = e,
                    MicSuppressor::Amount(a) => state.suppressor.amount = a.to_inner() as u8,
                    MicSuppressor::Style(s) => state.suppressor.style = s,
                    MicSuppressor::Sensitivity(s) => {
                        // Convert this to a percent
                        let percent = ((s.to_inner() + 120.0) / 60.0) * 100.0;
                        state.suppressor.sense = percent as u8
                    }
                    _ => {}
                },
            }
        }

        Ok(state)
    }
}
