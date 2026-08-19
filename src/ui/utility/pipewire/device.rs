use crate::ui::utility::pipewire::ffi::{
    PW_TYPE_INTERFACE_DEVICE, PW_TYPE_INTERFACE_NODE, PW_TYPE_INTERFACE_PORT,
    PW_VERSION_DEVICE_EVENTS, PW_VERSION_NODE_EVENTS, PipeWire, PortInfo, PwDeviceProxy,
    PwNodeProxy, PwPortProxy,
};
use crate::ui::utility::pipewire::{PipeWireNode, PipeWireNodeType, TO_BOOL, TO_U32};
use anyhow::Result;
use log::{debug, error};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub fn find_pipewire_nodes_for_usb(bus: u8, address: u8) -> Result<Vec<PipeWireNode>> {
    let card = match find_alsa_card(bus, address)? {
        Some(card) => card,
        None => return Ok(Vec::new()),
    };
    find_pipewire_nodes_for_card(card)
}

fn find_alsa_card(bus: u8, address: u8) -> Result<Option<u32>> {
    debug!("Searching for ALSA card on bus {} address {}", bus, address);
    let root = Path::new("/sys/bus/usb/devices");

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        let Some(bus_number) = read_u8(path.join("busnum")) else {
            continue;
        };

        let Some(device_number) = read_u8(path.join("devnum")) else {
            continue;
        };

        if bus_number != bus || device_number != address {
            continue;
        }

        debug!("Matched USB device: {}", path.display());
        if let Some(card) = find_card(&path)? {
            debug!("Matched ALSA card {} at {}", card, path.display());
            return Ok(Some(card as u32));
        }

        // Some devices expose sound/ below interface directories
        for child in fs::read_dir(&path)? {
            let child = child?;
            if !child.file_type()?.is_dir() {
                continue;
            }

            if let Some(card) = find_card(&child.path())? {
                debug!("Matched ALSA card {} at {}", card, child.path().display());
                return Ok(Some(card as u32));
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

        if entry.file_type()?.is_dir()
            && let Some(card) = find_card_number(&entry.path())?
        {
            return Ok(Some(card));
        }
    }

    Ok(None)
}

fn read_u8(path: PathBuf) -> Option<u8> {
    let text = fs::read_to_string(path).ok()?;
    text.trim().parse().ok()
}

// Helper object for the upcoming discovery code
struct Node {
    name: String,
    id: u32,
    device_id: u32,
    is_split_child: bool,
    media_class: PipeWireNodeType,
    channels: u32,
}

// Ok, this is relatively large, what we do is establish a connection to pipewire, let a
// full enumeration happen of nodes, ports and devices, then check the results. Previously
// this was done with `pw-dump`, but we're essentially doing the same thing directly.
fn find_pipewire_nodes_for_card(card: u32) -> Result<Vec<PipeWireNode>> {
    let client = PipeWire::load()?;
    client.init();

    let main_loop = Rc::new(RefCell::new(client.main_loop_new()?));
    let context = Rc::new(RefCell::new(client.context_new(&*main_loop.borrow())?));
    debug!("Context Created");

    let core = Rc::new(RefCell::new(context.borrow().connect()?));
    let registry = Rc::new(RefCell::new(core.borrow().get_registry()?));
    debug!("Registry Created");

    let dev_listeners = Rc::new(RefCell::new(HashMap::new()));
    let node_listeners = Rc::new(RefCell::new(HashMap::new()));
    let port_listeners = Rc::new(RefCell::new(HashMap::new()));

    let registry_inner = registry.clone();
    let dev_listeners_inner = dev_listeners.clone();
    let node_listeners_inner = node_listeners.clone();
    let port_listeners_inner = port_listeners.clone();

    let device_cache = Rc::new(RefCell::new(Vec::new()));
    let device_cache_inner = device_cache.clone();

    let node_cache = Rc::new(RefCell::new(Vec::new()));
    let node_cache_inner = node_cache.clone();

    let port_cache = Rc::new(RefCell::new(Vec::new()));
    let port_cache_inner = port_cache.clone();

    let _ = registry
        .borrow_mut()
        .add_global_listener(move |id, _, type_str, _, _| {
            let registry = registry_inner.borrow_mut();
            let mut dev_listeners = dev_listeners_inner.borrow_mut();
            let mut node_listeners = node_listeners_inner.borrow_mut();
            let mut port_listeners = port_listeners_inner.borrow_mut();
            if type_str == PW_TYPE_INTERFACE_DEVICE {
                let Ok(proxy) =
                    registry.bind(id, PW_TYPE_INTERFACE_DEVICE, PW_VERSION_DEVICE_EVENTS)
                else {
                    error!("Failed to bind device {}", id);
                    return;
                };

                let device_cache_inner = device_cache_inner.clone();
                let mut device_proxy = PwDeviceProxy::from_proxy(proxy);
                let _ = device_proxy.add_info_listener(move |info| {
                    let props = &info.props;
                    let alsa_card = props.get("api.alsa.card").and_then(TO_U32);

                    if let Some(alsa_card) = alsa_card
                        && alsa_card == card
                    {
                        device_cache_inner.borrow_mut().push(id);
                    }
                });
                dev_listeners.insert(id, device_proxy);
            }
            if type_str == PW_TYPE_INTERFACE_NODE {
                let Ok(proxy) = registry.bind(id, PW_TYPE_INTERFACE_NODE, PW_VERSION_NODE_EVENTS)
                else {
                    error!("Failed to bind node {}", id);
                    return;
                };

                let mut node_proxy = PwNodeProxy::from_proxy(proxy);
                let node_cache_inner = node_cache_inner.clone();
                let _ = node_proxy.add_info_listener(move |info| {
                    let props = &info.props;

                    let split_parent = "api.alsa.split.parent";
                    let split_position = "api.alsa.split.position";

                    let device_id = props.get("device.id").and_then(TO_U32);
                    let split_parent = props.get(split_parent).and_then(TO_BOOL);
                    let split_position = props.get(split_position).map(String::as_str);
                    let name = props.get("node.name").map(String::as_str);
                    let media_class = props.get("media.class").map(String::as_str);
                    let audio_channels = props.get("audio.channels").and_then(TO_U32);

                    // If this is a UCM Child, we should label it as such.
                    let is_split_child = split_parent != Some(true) && split_position.is_some();

                    let Some(device_id) = device_id else {
                        return;
                    };

                    let Some(name) = name else {
                        return;
                    };

                    let Some(media_class) = media_class else {
                        return;
                    };

                    let Some(channels) = audio_channels else {
                        return;
                    };

                    let media_class = if media_class.starts_with("Audio/Source") {
                        PipeWireNodeType::Source
                    } else if media_class.starts_with("Audio/Sink") {
                        PipeWireNodeType::Sink
                    } else {
                        return;
                    };

                    let node = Node {
                        device_id,
                        name: String::from(name),
                        id,
                        is_split_child,
                        media_class,
                        channels,
                    };
                    node_cache_inner.borrow_mut().push(node);
                });

                // Register this proxy, we'll get called next pass
                node_listeners.insert(id, node_proxy);
            }

            if type_str == PW_TYPE_INTERFACE_PORT {
                let Ok(proxy) = registry.bind(id, "PipeWire:Interface:Port", 3) else {
                    return;
                };

                let mut port_proxy = PwPortProxy::from_proxy(proxy);
                let port_cache = port_cache_inner.clone();
                let _ = port_proxy.add_info_listener(move |info: PortInfo| {
                    port_cache.borrow_mut().push(info);
                });

                port_listeners.insert(id, port_proxy);
            }
        });

    // If done properly, we'd break this into phases, but for now, this is sufficient
    let sync_flush = Rc::new(Cell::new(false));
    let current_sync = Rc::new(Cell::new(core.borrow().sync(0)));
    let current_sync_inner = current_sync.clone();

    let core_inner = core.clone();
    let mainloop_inner = main_loop.clone();

    let _ = core.borrow_mut().add_done_listener(move |_, seq| {
        if seq == current_sync_inner.get() {
            if !sync_flush.get() {
                current_sync_inner.set(core_inner.borrow().sync(0));
                sync_flush.set(true);
            } else {
                mainloop_inner.borrow().quit();
            }
        }
    });

    debug!("Pipewire Ready, starting main loop..");
    main_loop.borrow().run();
    debug!("Main Loop Finished, collating data..");

    // Ok, iterate the nodes, see if they have a device ID in our list, then convert.
    let nodes = node_cache
        .borrow()
        .iter()
        .filter(|node| device_cache.borrow().contains(&node.device_id))
        .filter_map(|node| {
            // Ok, we need to locate the channels for this node, we know how many to expect.
            let mut channels = HashMap::new();
            port_cache.borrow().iter().for_each(|port| {
                let name = port.props.get("audio.channel").map(String::as_str);
                let id = port.props.get("object.id").and_then(TO_U32);
                let is_node = port.props.get("node.id").and_then(TO_U32) == Some(node.id);

                // We only ever want outputs, this will pull in the monitor ports for Sinks which
                // we can connect to.
                let is_direction = port.direction_is_output;

                if let Some(name) = name
                    && is_node
                    && is_direction
                    && id.is_some()
                {
                    channels.insert(name.to_string(), id.unwrap());
                }
            });

            if node.channels != channels.len() as u32 {
                error!(
                    "Node {} has {} channels, expected {}",
                    node.name,
                    channels.len(),
                    node.channels
                );
                return None;
            }

            Some(PipeWireNode {
                name: node.name.clone(),
                id: node.id,
                is_split_child: node.is_split_child,
                node_type: node.media_class,
                channels,
            })
        })
        .collect();

    Ok(nodes)
}
