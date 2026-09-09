use anyhow::{Context, Result, bail};
use beacn_lib::audio::messages::Message;
use beacn_lib::audio::messages::bass_enhancement::{BassAmount, BassEnhancement};
use beacn_lib::audio::messages::compressor::{
    Compressor, CompressorMode, CompressorRatio, CompressorThreshold,
};
use beacn_lib::audio::messages::deesser::DeEsser;
use beacn_lib::audio::messages::eq_common::{EQBand, EQBandType, EQFrequency, EQGain, EQQ};
use beacn_lib::audio::messages::eq_microphone::{EQMicrophone, EQMode};
use beacn_lib::audio::messages::expander::{Expander, ExpanderMode, ExpanderRatio};
use beacn_lib::audio::messages::suppressor::Suppressor;
use beacn_lib::types::{MakeUpGain, Percent, TimeFrame};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::PathBuf;
use strum::IntoEnumIterator;

use crate::get_config_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioProfile {
    pub schema_version: u32,
    pub name: String,
    pub settings: Vec<Message>,
}

impl Default for AudioProfile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            name: "Default".to_string(),
            settings: vec![],
        }
    }
}

pub struct ProfileManager;

impl ProfileManager {
    /// Directory containing all profiles: ~/.config/beacn-utility/profiles/
    pub fn get_profiles_dir() -> Result<PathBuf> {
        let base = get_config_path()?;
        let profiles = base.join("profiles");
        fs::create_dir_all(&profiles)?;
        Ok(profiles)
    }

    /// Profile directory: ~/.config/beacn-utility/profiles/<profile_name>/
    pub fn get_profile_dir(profile_name: &str) -> Result<PathBuf> {
        let clean_name = sanitize_filename(profile_name);
        let dir = Self::get_profiles_dir()?.join(&clean_name);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Ensure standard factory default presets exist on disk
    pub fn ensure_default_profiles() {
        type FactoryDef<'a> = (
            &'a str,
            Vec<(EQBand, EQBandType, f32, f32, f32)>,
            (bool, f32, f32, f32, f32, f32),
            (bool, i8, f32, f32, f32),
            (bool, f32),
            (bool, f32),
            (bool, f32),
        );
        let default_defs: [FactoryDef<'_>; 8] = [
            (
                "Broadcast",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 80.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 250.0, -2.5, 1.2),
                    (EQBand::Band3, EQBandType::BellBand, 3500.0, 3.0, 1.0),
                    (EQBand::Band4, EQBandType::HighShelf, 10000.0, 2.0, 0.7),
                ],
                (true, -20.0, 3.5, 8.0, 80.0, 2.5),
                (true, -38, 2.5, 8.0, 80.0),
                (true, 40.0),
                (true, 4.0),
                (true, 35.0),
            ),
            (
                "Gaming & Discord",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 100.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 300.0, -3.0, 1.5),
                    (EQBand::Band3, EQBandType::BellBand, 2800.0, 4.0, 1.2),
                    (EQBand::Band4, EQBandType::HighShelf, 8000.0, 1.5, 0.7),
                ],
                (true, -22.0, 4.0, 5.0, 60.0, 3.0),
                (true, -34, 3.0, 5.0, 60.0),
                (true, 55.0),
                (false, 0.0),
                (true, 40.0),
            ),
            (
                "Warm Voiceover",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 70.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 160.0, 2.0, 1.0),
                    (EQBand::Band3, EQBandType::BellBand, 500.0, -2.0, 1.4),
                    (EQBand::Band4, EQBandType::BellBand, 4500.0, 2.5, 0.9),
                ],
                (true, -16.0, 2.2, 15.0, 120.0, 1.5),
                (true, -42, 2.0, 10.0, 100.0),
                (true, 25.0),
                (true, 3.0),
                (true, 30.0),
            ),
            (
                "High Clarity",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 80.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 200.0, 1.0, 1.0),
                    (EQBand::Band3, EQBandType::BellBand, 6500.0, -4.0, 2.5),
                    (EQBand::Band4, EQBandType::HighShelf, 12000.0, 1.0, 0.7),
                ],
                (true, -18.0, 2.8, 10.0, 90.0, 2.0),
                (true, -38, 2.5, 8.0, 80.0),
                (true, 40.0),
                (false, 0.0),
                (true, 50.0),
            ),
            (
                "Podcast Warmth",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 75.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 220.0, 1.5, 1.1),
                    (EQBand::Band3, EQBandType::BellBand, 450.0, -2.5, 1.4),
                    (EQBand::Band4, EQBandType::BellBand, 3800.0, 2.5, 1.0),
                    (EQBand::Band5, EQBandType::HighShelf, 10000.0, 1.5, 0.7),
                ],
                (true, -18.0, 3.0, 10.0, 80.0, 2.0),
                (true, -40, 2.2, 8.0, 90.0),
                (true, 30.0),
                (true, 2.5),
                (true, 35.0),
            ),
            (
                "Streaming & Esports",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 100.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 320.0, -3.0, 1.4),
                    (EQBand::Band3, EQBandType::BellBand, 3000.0, 3.5, 1.1),
                    (EQBand::Band4, EQBandType::HighShelf, 8500.0, 1.0, 0.7),
                ],
                (true, -22.0, 4.0, 5.0, 60.0, 3.0),
                (true, -35, 3.0, 5.0, 60.0),
                (true, 50.0),
                (false, 0.0),
                (true, 45.0),
            ),
            (
                "Flat Studio Reference",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 80.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 250.0, 0.0, 1.0),
                    (EQBand::Band3, EQBandType::BellBand, 2500.0, 0.0, 1.0),
                    (EQBand::Band4, EQBandType::HighShelf, 10000.0, 0.0, 0.7),
                ],
                (false, -18.0, 2.0, 15.0, 100.0, 0.0),
                (false, -45, 1.5, 10.0, 100.0),
                (false, 0.0),
                (false, 0.0),
                (false, 0.0),
            ),
            (
                "ASMR & Intimate Vocal",
                vec![
                    (EQBand::Band1, EQBandType::HighPassFilter, 60.0, 0.0, 0.7),
                    (EQBand::Band2, EQBandType::BellBand, 150.0, -1.5, 1.2),
                    (EQBand::Band3, EQBandType::BellBand, 4000.0, 1.5, 0.8),
                    (EQBand::Band4, EQBandType::HighShelf, 12000.0, 3.0, 0.7),
                ],
                (true, -14.0, 1.8, 20.0, 150.0, 1.0),
                (false, -48, 1.5, 15.0, 120.0),
                (true, 15.0),
                (true, 1.5),
                (true, 40.0),
            ),
        ];

        for (name, eq_cfgs, comp, exp, supp, bass, deess) in default_defs {
            if let Ok(dir) = Self::get_profile_dir(name) {
                let profile_file = dir.join("profile.json");
                if profile_file.exists() {
                    continue;
                }

                let mut settings = Vec::new();

                // Mic EQ settings
                settings.push(Message::EQMicrophone(EQMicrophone::Mode(EQMode::Advanced)));
                for &(band, b_type, freq, gain, q) in &eq_cfgs {
                    settings.push(Message::EQMicrophone(EQMicrophone::Type(
                        EQMode::Advanced,
                        band,
                        b_type,
                    )));
                    settings.push(Message::EQMicrophone(EQMicrophone::Frequency(
                        EQMode::Advanced,
                        band,
                        EQFrequency(freq),
                    )));
                    settings.push(Message::EQMicrophone(EQMicrophone::Gain(
                        EQMode::Advanced,
                        band,
                        EQGain(gain),
                    )));
                    settings.push(Message::EQMicrophone(EQMicrophone::Q(
                        EQMode::Advanced,
                        band,
                        EQQ(q),
                    )));
                    settings.push(Message::EQMicrophone(EQMicrophone::Enabled(
                        EQMode::Advanced,
                        band,
                        true,
                    )));
                }
                for band in EQBand::iter() {
                    if !eq_cfgs.iter().any(|(b, ..)| *b == band) {
                        settings.push(Message::EQMicrophone(EQMicrophone::Enabled(
                            EQMode::Advanced,
                            band,
                            false,
                        )));
                    }
                }

                // Compressor
                let (c_en, c_th, c_rat, c_att, c_rel, c_mk) = comp;
                settings.push(Message::Compressor(Compressor::Mode(
                    CompressorMode::Advanced,
                )));
                settings.push(Message::Compressor(Compressor::Enabled(
                    CompressorMode::Advanced,
                    c_en,
                )));
                settings.push(Message::Compressor(Compressor::Threshold(
                    CompressorMode::Advanced,
                    CompressorThreshold(c_th),
                )));
                settings.push(Message::Compressor(Compressor::Ratio(
                    CompressorMode::Advanced,
                    CompressorRatio(c_rat),
                )));
                settings.push(Message::Compressor(Compressor::Attack(
                    CompressorMode::Advanced,
                    TimeFrame(c_att),
                )));
                settings.push(Message::Compressor(Compressor::Release(
                    CompressorMode::Advanced,
                    TimeFrame(c_rel),
                )));
                settings.push(Message::Compressor(Compressor::MakeupGain(
                    CompressorMode::Advanced,
                    MakeUpGain(c_mk),
                )));

                // Expander
                let (e_en, e_th, e_rat, e_att, e_rel) = exp;
                settings.push(Message::Expander(Expander::Mode(ExpanderMode::Advanced)));
                settings.push(Message::Expander(Expander::Enabled(
                    ExpanderMode::Advanced,
                    e_en,
                )));
                settings.push(Message::Expander(Expander::Threshold(
                    ExpanderMode::Advanced,
                    e_th.into(),
                )));
                settings.push(Message::Expander(Expander::Ratio(
                    ExpanderMode::Advanced,
                    ExpanderRatio(e_rat),
                )));
                settings.push(Message::Expander(Expander::Attack(
                    ExpanderMode::Advanced,
                    TimeFrame(e_att),
                )));
                settings.push(Message::Expander(Expander::Release(
                    ExpanderMode::Advanced,
                    TimeFrame(e_rel),
                )));

                // Suppressor
                let (s_en, s_amt) = supp;
                settings.push(Message::Suppressor(Suppressor::Enabled(s_en)));
                settings.push(Message::Suppressor(Suppressor::Amount(Percent(s_amt))));

                // Bass Enhancement
                let (b_en, b_amt) = bass;
                settings.push(Message::BassEnhancement(BassEnhancement::Enabled(b_en)));
                settings.push(Message::BassEnhancement(BassEnhancement::Amount(
                    BassAmount(b_amt.clamp(0.0, 10.0)),
                )));

                // De-Esser
                let (d_en, d_amt) = deess;
                settings.push(Message::DeEsser(DeEsser::Enabled(d_en)));
                settings.push(Message::DeEsser(DeEsser::Amount(Percent(d_amt))));

                let profile = AudioProfile {
                    schema_version: 1,
                    name: name.to_string(),
                    settings,
                };

                let _ = Self::save_profile(name, &profile);
                info!("Initialized factory preset profile: '{name}'");
            }
        }
    }

    /// List all existing profile names
    pub fn list_profiles() -> Vec<String> {
        let mut list = Vec::new();
        if let Ok(dir) = Self::get_profiles_dir()
            && let Ok(entries) = fs::read_dir(dir)
        {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_dir()
                    && let Some(name) = entry.file_name().to_str()
                {
                    list.push(name.to_string());
                }
            }
        }
        if !list.iter().any(|s| s.eq_ignore_ascii_case("Default")) {
            list.push("Default".to_string());
        }
        list.sort();
        list
    }

    /// Returns the active profile name, defaulting to "Default"
    pub fn get_active_profile_name() -> String {
        if let Ok(base) = get_config_path() {
            let active_file = base.join("active_profile.txt");
            if let Ok(content) = fs::read_to_string(active_file) {
                let name = content.trim().to_string();
                if !name.is_empty() {
                    return name;
                }
            }
        }
        "Default".to_string()
    }

    /// Persist the active profile name
    pub fn set_active_profile_name(name: &str) -> Result<()> {
        let base = get_config_path()?;
        let active_file = base.join("active_profile.txt");
        let clean_name = sanitize_filename(name);
        fs::write(active_file, clean_name)?;
        Ok(())
    }

    /// Load the profile from disk
    pub fn load_profile(profile_name: &str) -> Result<Option<AudioProfile>> {
        let dir = Self::get_profile_dir(profile_name)?;
        let profile_file = dir.join("profile.json");
        if !profile_file.exists() {
            return Ok(None);
        }

        let file = File::open(&profile_file)
            .with_context(|| format!("Failed to open profile file: {profile_file:?}"))?;
        let profile: AudioProfile = serde_json::from_reader(file)
            .with_context(|| format!("Failed to parse profile JSON: {profile_file:?}"))?;
        debug!(
            "Loaded audio profile: '{}' ({profile_file:?})",
            profile.name
        );
        Ok(Some(profile))
    }

    /// Save profile to disk
    pub fn save_profile(profile_name: &str, profile: &AudioProfile) -> Result<()> {
        let dir = Self::get_profile_dir(profile_name)?;
        let profile_file = dir.join("profile.json");
        let file = File::create(&profile_file)?;
        serde_json::to_writer_pretty(file, profile)?;
        debug!("Saved audio profile to: {profile_file:?}");
        Ok(())
    }

    /// Delete a profile directory
    pub fn delete_profile(profile_name: &str) -> Result<()> {
        if profile_name.eq_ignore_ascii_case("Default") {
            bail!("Cannot delete the Default profile");
        }
        let dir = Self::get_profile_dir(profile_name)?;
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_serialization() {
        let profile = AudioProfile {
            schema_version: 1,
            name: "Test Profile".to_string(),
            settings: vec![],
        };
        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: AudioProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "Test Profile");
        assert_eq!(deserialized.schema_version, 1);
    }

    #[test]
    fn test_profile_manager_basic() {
        ProfileManager::ensure_default_profiles();
        let profiles = ProfileManager::list_profiles();
        assert!(!profiles.is_empty());
        assert!(profiles.contains(&"Default".to_string()));

        ProfileManager::set_active_profile_name("Broadcast").unwrap();
        assert_eq!(ProfileManager::get_active_profile_name(), "Broadcast");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Valid_Name-123"), "Valid_Name-123");
        assert_eq!(
            sanitize_filename("Invalid/Name:Test*?"),
            "Invalid_Name_Test__"
        );
        assert_eq!(sanitize_filename("Name with spaces"), "Name with spaces");
    }
}
