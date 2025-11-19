use anyhow::{Result, bail};
use beacn_lib::audio::LinkedApp;
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::bass_enhancement::BassPreset;
use beacn_lib::audio::messages::compressor::CompressorMode;
use beacn_lib::audio::messages::equaliser::{EQBand, EQBandType, EQMode};
use beacn_lib::audio::messages::expander::ExpanderMode;
use beacn_lib::audio::messages::headphone_equaliser::HPEQType;
use beacn_lib::audio::messages::headphones::HeadphoneTypes;
use beacn_lib::audio::messages::lighting::{
    LightingMeterSource, LightingMode, LightingMuteMode, LightingSuspendMode, StudioLightingMode,
};
use beacn_lib::audio::messages::suppressor::SuppressorStyle;
use beacn_lib::types::ToInner;
use enum_map::EnumMap;

use crate::device_manager::{
    AudioMessage, DefinitionState, DeviceDefinition, ErrorType, LinkedCommands,
};
use crate::ui::states::{DeviceState, ErrorMessage, LoadState};
use beacn_lib::audio::messages::bass_enhancement::BassEnhancement as MicBaseEnhancement;
use beacn_lib::audio::messages::compressor::Compressor as MicCompressor;
use beacn_lib::audio::messages::deesser::DeEsser as MicDeEsser;
use beacn_lib::audio::messages::equaliser::Equaliser as MicEqualiser;
use beacn_lib::audio::messages::exciter::Exciter as MicExciter;
use beacn_lib::audio::messages::expander::Expander as MicExpander;
use beacn_lib::audio::messages::headphone_equaliser::HeadphoneEQ as MicHeadphoneEQ;
use beacn_lib::audio::messages::headphones::Headphones as MicHeadphones;
use beacn_lib::audio::messages::lighting::Lighting as MicLighting;
use beacn_lib::audio::messages::mic_setup::MicSetup as MicMicSetup;
use beacn_lib::audio::messages::subwoofer::Subwoofer as MicSubwoofer;
use beacn_lib::audio::messages::suppressor::Suppressor as MicSuppressor;
use beacn_lib::crossbeam::channel::Sender;
use beacn_lib::manager::DeviceType;
use log::debug;

type Rgb = [u8; 3];

#[derive(Debug, Default, Clone)]
pub struct BeacnAudioState {
    pub device_definition: DeviceDefinition,
    pub device_state: DeviceState,
    pub device_sender: Option<Sender<AudioMessage>>,

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

    pub linked: Option<Vec<LinkedApp>>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Headphones {
    pub level: f32,       // [-70.0...=0.0]
    pub mic_monitor: f32, // [-100.0..=6.0]
    pub linked: bool,
    pub output_gain: f32, // f32[0.0..=12.0]
    pub headphone_type: HeadphoneTypes,
    pub fx_enabled: bool,

    // NOTE: The following values should *NOT* be persisted, or saved / loaded from profiles
    pub studio_driverless: Option<bool>, // This is backwards at the moment, need to fix that
    pub mic_class_compliant: Option<bool>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Lighting {
    pub mic_mode: LightingMode,
    pub studio_mode: StudioLightingMode,
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

impl BeacnAudioState {
    pub fn handle_message(&mut self, message: Message) -> Result<Message> {
        let (tx, rx) = oneshot::channel();
        let message = AudioMessage::Handle(message, tx);

        match &self.device_sender {
            Some(sender) => {
                // Send the message, return the response (or fail).
                sender.send(message)?;
                let message = rx.recv()?;

                // Quickly intercept the message, and set our local value
                if let Ok(message) = message {
                    self.set_local_value(message);
                }
                Ok(message?)
            }
            None => bail!("Device Sender not Ready"),
        }
    }

    pub fn get_linked(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let message = AudioMessage::Linked(LinkedCommands::GetLinked(tx));

        match &self.device_sender {
            Some(sender) => {
                // Send the message, return the response (or fail).
                sender.send(message)?;
                let message = rx.recv()?;

                debug!("Result: {message:?}");

                // TODO: Should probably better error handle here.. :D
                if let Ok(apps) = message {
                    self.linked = apps;
                } else {
                    self.linked = None;
                }
            }
            None => bail!("Device Sender not Ready"),
        }
        Ok(())
    }

    pub fn set_link(&mut self, app: LinkedApp) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let message = AudioMessage::Linked(LinkedCommands::SetLinked(app, tx));
        match &self.device_sender {
            Some(sender) => {
                // Send the message, return the response (or fail).
                sender.send(message)?;
                let message = rx.recv()?;

                debug!("Result: {message:?}");
            }
            None => bail!("Device Sender not Ready"),
        }

        Ok(())
    }

    pub fn load_settings(definition: DeviceDefinition, sender: Sender<AudioMessage>) -> Self {
        let device_type = definition.device_type;

        let mut state = BeacnAudioState {
            device_definition: definition,
            device_state: DeviceState {
                state: LoadState::Loading,
                ..Default::default()
            },
            device_sender: Some(sender),
            ..Default::default()
        };

        // let mut state = Self::default();
        // state.device_definition = definition;
        // state.device_sender = Some(sender);
        // state.device_state.state = LoadState::LOADING;

        // Before we do anything else, is this definition in an error state?
        if let DefinitionState::Error(error) = &state.device_definition.state {
            match error {
                ErrorType::PermissionDenied => {
                    state.device_state.state = LoadState::PermissionDenied
                }
                ErrorType::ResourceBusy => state.device_state.state = LoadState::ResourceBusy,
                ErrorType::Other(s) => {
                    state.device_state.state = LoadState::Error;
                    state.device_state.errors.push(ErrorMessage {
                        error_text: Some(format!("Device Definition Error: {s}")),
                        failed_message: None,
                    });
                }
                ErrorType::Unknown => {
                    state.device_state.state = LoadState::Error;
                    state.device_state.errors.push(ErrorMessage {
                        error_text: Some("Unknown Error".to_string()),
                        failed_message: None,
                    });
                }
            }
            return state;
        }

        // Ok, grab all the variables from the mic
        let messages = Message::generate_fetch_message(device_type);
        for message in messages {
            // Skip this message if it's not valid for this version
            if message.get_message_minimum_version() > state.device_definition.device_info.version {
                continue;
            }

            let value = state.handle_message(message);
            match value {
                Ok(value) => state.set_local_value(value),
                Err(value) => {
                    // fetch_value didn't panic, but it did error
                    state.device_state.state = LoadState::Error;
                    state.device_state.errors.push(ErrorMessage {
                        error_text: Some(format!("{value:?}")),
                        failed_message: Some(message),
                    })
                }
            }
        }

        if state.device_definition.device_type == DeviceType::BeacnStudio {
            let _ = state.get_linked();
        }
        state.device_state.state = LoadState::Running;
        state
    }

    pub(crate) fn set_local_value(&mut self, value: Message) {
        match value {
            Message::BassEnhancement(b) => match b {
                MicBaseEnhancement::Enabled(v) => self.bass_enhancement.enabled = v,
                MicBaseEnhancement::Preset(v) => self.bass_enhancement.preset = v,
                MicBaseEnhancement::Amount(v) => self.bass_enhancement.amount = v.to_inner() as i8,
                _ => {}
            },
            Message::Compressor(c) => match c {
                MicCompressor::Mode(mode) => self.compressor.mode = mode,
                MicCompressor::Attack(mode, value) => {
                    self.compressor.values[mode].attack = value.to_inner() as u16
                }
                MicCompressor::Release(mode, value) => {
                    self.compressor.values[mode].release = value.to_inner() as u16
                }
                MicCompressor::Threshold(mode, value) => {
                    self.compressor.values[mode].threshold = value.to_inner() as i8
                }
                MicCompressor::Ratio(mode, value) => {
                    self.compressor.values[mode].ratio = value.to_inner()
                }
                MicCompressor::MakeupGain(mode, value) => {
                    self.compressor.values[mode].makeup = value.to_inner()
                }
                MicCompressor::Enabled(mode, value) => self.compressor.values[mode].enabled = value,
                _ => {}
            },
            Message::DeEsser(d) => match d {
                MicDeEsser::Amount(value) => self.de_esser.amount = value.to_inner() as u8,
                MicDeEsser::Enabled(value) => self.de_esser.enabled = value,
                _ => {}
            },
            Message::Equaliser(e) => match e {
                MicEqualiser::Mode(mode) => self.equaliser.mode = mode,
                MicEqualiser::Type(mode, band, value) => {
                    self.equaliser.bands[mode][band].band_type = value
                }
                MicEqualiser::Gain(mode, band, value) => {
                    self.equaliser.bands[mode][band].gain = value.to_inner()
                }
                MicEqualiser::Frequency(mode, band, value) => {
                    self.equaliser.bands[mode][band].frequency = value.to_inner() as u32
                }
                MicEqualiser::Q(mode, band, value) => {
                    self.equaliser.bands[mode][band].q = value.to_inner()
                }
                MicEqualiser::Enabled(mode, band, value) => {
                    self.equaliser.bands[mode][band].enabled = value
                }
                _ => {}
            },
            Message::Exciter(e) => match e {
                MicExciter::Amount(value) => self.exciter.amount = value.to_inner() as u8,
                MicExciter::Frequency(value) => self.exciter.freq = value.to_inner() as u16,
                MicExciter::Enabled(value) => self.exciter.enabled = value,
                _ => {}
            },
            Message::Expander(e) => match e {
                MicExpander::Mode(mode) => self.expander.mode = mode,
                MicExpander::Threshold(mode, value) => {
                    self.expander.values[mode].threshold = value.to_inner() as i8
                }
                MicExpander::Ratio(mode, value) => {
                    self.expander.values[mode].ratio = value.to_inner()
                }
                MicExpander::Enabled(mode, value) => self.expander.values[mode].enabled = value,
                MicExpander::Attack(mode, value) => {
                    self.expander.values[mode].attack = value.to_inner() as u16
                }
                MicExpander::Release(mode, value) => {
                    self.expander.values[mode].release = value.to_inner() as u16
                }
                _ => {}
            },
            Message::HeadphoneEQ(h) => match h {
                MicHeadphoneEQ::Amount(eq_type, value) => {
                    self.headphone_eq.eq[eq_type].amount = value.to_inner()
                }
                MicHeadphoneEQ::Enabled(eq_type, value) => {
                    self.headphone_eq.eq[eq_type].enabled = value
                }
                _ => {}
            },
            Message::Headphones(h) => match h {
                MicHeadphones::HeadphoneLevel(v) => self.headphones.level = v.to_inner(),
                MicHeadphones::MicMonitor(v) => self.headphones.mic_monitor = v.to_inner(),
                MicHeadphones::StudioMicMonitor(v) => self.headphones.mic_monitor = v.to_inner(),
                MicHeadphones::MicChannelsLinked(b) => self.headphones.linked = b,
                MicHeadphones::StudioChannelsLinked(b) => self.headphones.linked = b,
                MicHeadphones::MicOutputGain(v) => self.headphones.output_gain = v.to_inner(),
                MicHeadphones::HeadphoneType(t) => self.headphones.headphone_type = t,
                MicHeadphones::FXEnabled(t) => self.headphones.fx_enabled = t,
                MicHeadphones::StudioDriverless(t) => self.headphones.studio_driverless = Some(t),
                MicHeadphones::MicClassCompliant(t) => {
                    self.headphones.mic_class_compliant = Some(t)
                }
                _ => {}
            },
            Message::Lighting(l) => match l {
                MicLighting::Mode(m) => self.lighting.mic_mode = m,
                MicLighting::StudioMode(m) => self.lighting.studio_mode = m,
                MicLighting::Colour1(c) => self.lighting.colour1 = [c.red, c.green, c.blue],
                MicLighting::Colour2(c) => self.lighting.colour2 = [c.red, c.green, c.blue],
                MicLighting::Speed(v) => self.lighting.speed = v.to_inner(),
                MicLighting::Brightness(v) => self.lighting.brightness = v.to_inner(),
                MicLighting::MeterSource(v) => self.lighting.source = v,
                MicLighting::MeterSensitivity(s) => self.lighting.sensitivity = s.to_inner(),
                MicLighting::MuteMode(m) => self.lighting.mute_mode = m,
                MicLighting::MuteColour(c) => self.lighting.mute_colour = [c.red, c.green, c.blue],
                MicLighting::SuspendMode(m) => self.lighting.suspend_mode = m,
                MicLighting::SuspendBrightness(b) => {
                    self.lighting.suspend_brightness = b.to_inner()
                }
                _ => {}
            },
            Message::MicSetup(m) => match m {
                MicMicSetup::MicGain(g) => self.mic_setup.gain = g.to_inner() as u8,
                MicMicSetup::StudioMicGain(g) => self.mic_setup.gain = g.to_inner() as u8,
                MicMicSetup::StudioPhantomPower(p) => self.mic_setup.phantom = p,
                _ => {}
            },
            Message::Subwoofer(s) => match s {
                MicSubwoofer::Enabled(e) => self.subwoofer.enabled = e,
                MicSubwoofer::Amount(a) => self.subwoofer.amount = a.to_inner() as u8,
                _ => {}
            },
            Message::Suppressor(s) => match s {
                MicSuppressor::Enabled(e) => self.suppressor.enabled = e,
                MicSuppressor::Amount(a) => self.suppressor.amount = a.to_inner() as u8,
                MicSuppressor::Style(s) => self.suppressor.style = s,
                MicSuppressor::Sensitivity(s) => {
                    // Convert this to a percent
                    let percent = ((s.to_inner() + 120.0) / 60.0) * 100.0;
                    self.suppressor.sense = percent as u8
                }
                _ => {}
            },
        }
    }
}
