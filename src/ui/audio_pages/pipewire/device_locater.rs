use anyhow::{Context, Result};
use log::debug;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn find_pipewire_nodes_for_usb(bus: u8, address: u8) -> Result<Vec<PipeWireNode>> {
    let card = match find_alsa_card(bus, address)? {
        Some(card) => card,
        None => return Ok(Vec::new()),
    };
    find_pipewire_nodes_for_card(card)
}

fn find_alsa_card(bus: u8, address: u8) -> Result<Option<u8>> {
    let root = Path::new("/sys/bus/usb/devices");

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        let Some(busnum) = read_u8(path.join("busnum")) else {
            continue;
        };

        let Some(devnum) = read_u8(path.join("devnum")) else {
            continue;
        };

        if busnum != bus || devnum != address {
            continue;
        }

        debug!("Matched USB device: {}", path.display());
        if let Some(card) = find_card(&path)? {
            debug!("Matched ALSA card {} at {}", card, path.display());
            return Ok(Some(card));
        }

        // Some devices expose sound/ below interface directories
        for child in fs::read_dir(&path)? {
            let child = child?;

            if !child.file_type()?.is_dir() {
                continue;
            }

            if let Some(card) = find_card(&child.path())? {
                debug!("Matched ALSA card {} at {}", card, child.path().display());
                return Ok(Some(card));
            }
        }
    }

    Ok(None)
}

fn find_card(path: &Path) -> Result<Option<u8>> {
    find_card_number(&path.join("sound"))
}

fn find_card_number(path: &Path) -> Result<Option<u8>> {
    if !path.is_dir() {
        return Ok(None);
    }

    if let Some(number) = read_u8(path.join("number")) {
        return Ok(Some(number));
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;

        if entry.file_type()?.is_dir() {
            if let Some(card) = find_card_number(&entry.path())? {
                return Ok(Some(card));
            }
        }
    }

    Ok(None)
}

fn find_pipewire_nodes_for_card(card: u8) -> Result<Vec<PipeWireNode>> {
    let output = Command::new("pw-dump")
        .output()
        .context("failed to run pw-dump")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let objects: Vec<Value> = serde_json::from_slice(&output.stdout)?;

    // Find PipeWire Device IDs matching the ALSA card
    let device_ids: Vec<u64> = objects
        .iter()
        .filter(|o| o["type"] == "PipeWire:Interface:Device")
        .filter_map(|o| {
            let props = o.pointer("/info/props")?;

            let alsa_card = props.get("api.alsa.card")?;

            let matches = match alsa_card {
                Value::Number(n) => n.as_u64() == Some(card as u64),
                Value::String(s) => s.parse::<u8>().ok() == Some(card),
                _ => false,
            };

            if matches { o["id"].as_u64() } else { None }
        })
        .collect();

    let mut result = Vec::new();

    // Find Nodes belonging to those Devices
    for object in objects {
        if object["type"] != "PipeWire:Interface:Node" {
            continue;
        }

        let Some(props) = object.pointer("/info/props") else {
            continue;
        };

        let Some(device_id) = props.get("device.id").and_then(Value::as_u64) else {
            continue;
        };

        if !device_ids.contains(&device_id) {
            continue;
        }

        // Skip UCM SplitPCM child nodes; only the split "parent" node
        // (or an ordinary non-split node) should be surfaced.
        if is_split_child_node(props) {
            continue;
        }

        let Some(name) = props.get("node.name").and_then(Value::as_str) else {
            continue;
        };

        let Some(media_class) = props.get("media.class").and_then(Value::as_str) else {
            continue;
        };

        let Some(channels) = props.get("audio.channels").and_then(Value::as_u64) else {
            continue;
        };

        let node_type = if media_class.starts_with("Audio/Source") {
            PipeWireNodeType::Source
        } else if media_class.starts_with("Audio/Sink") {
            PipeWireNodeType::Sink
        } else {
            continue;
        };

        result.push(PipeWireNode {
            name: name.to_string(),
            node_type,
            channels,
        });
    }

    Ok(result)
}

fn is_split_child_node(props: &Value) -> bool {
    let is_split_parent = props
        .get("api.alsa.split.parent")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if is_split_parent {
        return false;
    }

    props.get("api.alsa.split.position").is_some()
}

fn read_u8(path: PathBuf) -> Option<u8> {
    let text = fs::read_to_string(path).ok()?;
    text.trim().parse().ok()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PipeWireNodeType {
    Source,
    Sink,
}

#[derive(Debug)]
pub struct PipeWireNode {
    pub name: String,
    pub node_type: PipeWireNodeType,
    pub channels: u64,
}
