// Ok, simple function here, connects to pipewire, find the ports, links everything together
// and starts processing samples. Caller can connect any number of ports from any number of devices
// and is responsible for actually processing them.
//
// AtomicBool is passed in to stop the mainloop when it changes to true.
//
// I might, in future, wrap this into a struct, so a clean drop will take care of shutdown, rather
// than having to infer it from the outside.

use crate::ui::utility::pipewire::TO_U32;
use crate::ui::utility::pipewire::ffi::{
    CaptureBuffer, PW_DIRECTION_INPUT, PW_ID_ANY, PW_TYPE_INTERFACE_LINK, PW_TYPE_INTERFACE_NODE,
    PW_TYPE_INTERFACE_PORT, PipeWire, PortInfo, PwCore, PwNodeProxy, PwPortProxy, PwProperties,
    PwProxy, PwStream, StreamCallbacks, stream_flags,
};
use crate::ui::utility::pipewire::pod::build_audio_pod;
use anyhow::Result;
use log::debug;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

type ChannelMap = HashMap<String, u32>;
pub type Process = Box<dyn FnMut(&[f32]) + Send + Sync>;

// This is a bit of a hack, but it's the only way I can think of to get a unique ID for each
// instance of this function.
fn next_instance_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct ChannelStream {
    pub channel_id: u32,
    pub process: Process,
}

pub fn get_audio(link: Vec<ChannelStream>, stop: Arc<AtomicBool>) -> Result<()> {
    debug!("Initialising Pipewire..");
    let pw = PipeWire::load()?;
    pw.init();

    // OK, we have everything we need from the source side, so we need to connect to pipewire,
    // sync everything, then wait for our stream ports to appear.
    let pw = Rc::new(RefCell::new(pw));
    let pw_inner = pw.clone();

    let main_loop = Rc::new(RefCell::new(pw.borrow().main_loop_new()?));
    let mainloop_inner = main_loop.clone();

    let context = Rc::new(RefCell::new(pw.borrow().context_new(&*main_loop.borrow())?));

    let core = Rc::new(RefCell::new(context.borrow().connect()?));
    let core_inner = core.clone();

    let registry = Rc::new(RefCell::new(core.borrow().get_registry()?));
    let registry_inner = registry.clone();

    let node_proxies = Rc::new(RefCell::new(Vec::<PwNodeProxy>::new()));
    let node_proxies_inner = node_proxies.clone();

    let port_proxies = Rc::new(RefCell::new(Vec::<PwPortProxy>::new()));
    let port_proxies_inner = port_proxies.clone();

    let link_proxies = Rc::new(RefCell::new(Vec::<PwProxy>::new()));
    let link_proxies_inner = link_proxies.clone();

    let instance = next_instance_id();
    let pid = std::process::id();

    let local_name = Rc::new(RefCell::new(format!("pipewire-test-{pid}-{instance}")));
    let local_name_inner = local_name.clone();

    // Ok, this is the storage for our stream which we've located already
    let stream_node_id = Rc::new(RefCell::<Option<u32>>::new(None));
    let stream_node_id_inner = stream_node_id.clone();

    let port_initial_cache = Rc::new(RefCell::new(Vec::<PortInfo>::new()));
    let port_initial_cache_inner = port_initial_cache.clone();

    let stream_known_ports = Rc::new(RefCell::new(ChannelMap::new()));
    let stream_known_ports_inner = stream_known_ports.clone();

    let stream_wanted_ports = Rc::new(RefCell::new(link));
    let stream_wanted_ports_inner = stream_wanted_ports.clone();

    let node_found = Rc::new(Cell::new(false));
    let node_found_inner = node_found.clone();

    let ports_found = Rc::new(Cell::new(false));
    let ports_found_inner = ports_found.clone();

    let clear_node_proxies_sync = Rc::new(RefCell::new(None));
    let clear_node_proxies_sync_inner = clear_node_proxies_sync.clone();

    let clear_port_proxies_sync = Rc::new(RefCell::new(None));
    let clear_port_proxies_sync_inner = clear_port_proxies_sync.clone();

    debug!("Registries and Storage Configured, Adding Listeners..");
    registry.borrow_mut().add_global_listener(
        move |id, _permissions, type_str, _version, _global_props| {
            let pw = pw_inner.clone();
            let core = core_inner.clone();
            let registry = registry_inner.clone();
            let local_name = local_name_inner.clone();

            let node_proxies = node_proxies_inner.clone();
            let port_proxies = port_proxies_inner.clone();
            let link_proxies = link_proxies_inner.clone();

            let stream_node_id = stream_node_id_inner.clone();
            let port_initial_cache = port_initial_cache_inner.clone();

            let stream_known_ports = stream_known_ports_inner.clone();
            let stream_wanted_ports = stream_wanted_ports_inner.clone();

            let node_found = node_found_inner.clone();
            let ports_found = ports_found_inner.clone();

            let clear_node_proxies_sync = clear_node_proxies_sync_inner.clone();
            let clear_port_proxies_sync = clear_port_proxies_sync_inner.clone();

            match type_str {
                PW_TYPE_INTERFACE_NODE => {
                    // If we're ready, we don't need to watch for anything anymore.
                    if node_found.get() {
                        return;
                    }

                    let Ok(proxy) = registry.borrow().bind(id, PW_TYPE_INTERFACE_NODE, 3) else {
                        return;
                    };
                    let mut node_proxy = PwNodeProxy::from_proxy(proxy);
                    let node_proxies_inner = node_proxies.clone();
                    let _ = node_proxy.add_info_listener(move |info| {
                        let Some(name) = info.props.get("node.name") else {
                            return;
                        };

                        if name == local_name.borrow().as_str() {
                            debug!("Stream Node has Appeared: {}", info.id);

                            // Store this node and detach all the proxies.
                            stream_node_id.borrow_mut().replace(info.id);
                            clear_node_proxies_sync
                                .borrow_mut()
                                .replace(core.borrow().sync(0));

                            // We need to validate any ports received before this point, and see
                            // if they belong to our node, if so, store them.
                            let known = stream_known_ports.clone();
                            let cache = port_initial_cache.clone();
                            handle_port_cache(info.id, known.borrow_mut(), cache.borrow());

                            // Check whether the above resulted in all ports arriving (shouldn't
                            // happen really, but just in case).
                            let wanted = stream_wanted_ports.clone();
                            let known = stream_known_ports.clone();
                            if wanted.borrow().len() == known.borrow().len() {
                                ports_found.set(true);
                                clear_port_proxies_sync
                                    .borrow_mut()
                                    .replace(core.borrow().sync(0));

                                let wanted = stream_wanted_ports.clone();
                                let known = stream_known_ports.clone();
                                debug!("Stream Node ready after Node Arrival");
                                do_links(
                                    link_proxies.borrow_mut(),
                                    wanted.borrow(),
                                    known.borrow(),
                                    pw.borrow(),
                                    core.borrow_mut(),
                                );
                            }
                        }
                    });

                    node_proxies.borrow_mut().push(node_proxy);
                }
                PW_TYPE_INTERFACE_PORT => {
                    if ports_found.get() {
                        return;
                    }

                    let Ok(proxy) = registry.borrow().bind(id, PW_TYPE_INTERFACE_PORT, 3) else {
                        return;
                    };
                    let mut port_proxy = PwPortProxy::from_proxy(proxy);
                    let _ = port_proxy.add_info_listener(move |info| {
                        let Some(parent_id) = stream_node_id.borrow().as_ref().copied() else {
                            // We have no way of pre-emptively knowing if this port is bound to our
                            // stream node or not, so we'll store it for now and parse it later.
                            if !info.direction_is_output {
                                port_initial_cache.borrow_mut().push(info);
                            }
                            return;
                        };

                        if stream_node_id.borrow().is_none() && !info.direction_is_output {
                            return;
                        }

                        // We are *ONLY* interested in input ports for our device
                        if info.direction_is_output {
                            return;
                        }
                        handle_port(parent_id, &mut stream_known_ports.borrow_mut(), &info);

                        let wanted = stream_wanted_ports.clone();
                        let known = stream_known_ports.clone();
                        if wanted.borrow().len() == known.borrow().len() {
                            clear_port_proxies_sync
                                .borrow_mut()
                                .replace(core.borrow().sync(0));
                            ports_found.set(true);

                            let wanted = stream_wanted_ports.clone();
                            let known = stream_known_ports.clone();
                            debug!("Stream Node ready after Port Arrival");
                            do_links(
                                link_proxies.borrow_mut(),
                                wanted.borrow(),
                                known.borrow(),
                                pw.borrow(),
                                core.borrow_mut(),
                            );
                        }
                    });
                    port_proxies.borrow_mut().push(port_proxy);
                }

                _ => {}
            }
        },
    )?;

    debug!("Listeners Configured, preparing Stream");
    // Map all incoming links to AUX channels
    let channel_count = stream_wanted_ports.borrow().len();
    let channels: Vec<String> = (0..channel_count).map(|i| format!("AUX{}", i)).collect();

    let mut props = PwProperties::new(&pw.borrow())?;
    props.set("node.name", local_name.borrow().as_str())?;
    props.set("node.description", "beacn-utility")?;
    props.set("media.type", "Audio")?;
    props.set("media.category", "Capture")?;
    props.set("media.role", "Monitor")?;
    props.set("audio.channels", &channel_count.to_string())?;
    props.set("audio.position", &format!("{}", channels.join(", ")))?;

    let stream = {
        let core_ref = core.borrow();
        Rc::new(RefCell::new(PwStream::new(
            &pw.borrow(),
            &core_ref,
            local_name.borrow().as_str(),
            props,
        )?))
    };

    let stream_wanted_ports_inner = stream_wanted_ports.clone();
    stream.borrow_mut().add_listener(StreamCallbacks {
        process: Some(Box::new(move |buf: CaptureBuffer| {
            for ch in 0..buf.channel_count() {
                let samples = buf.channel_samples(ch);
                if !samples.is_empty() {
                    // Ok, send this across to the processor..
                    let process = &mut stream_wanted_ports_inner.borrow_mut()[ch].process;
                    process(&samples);
                }
            }
        })),
        ..Default::default()
    })?;

    let pod_bytes = build_audio_pod(48000, &channels);
    stream.borrow().connect(
        PW_DIRECTION_INPUT,
        PW_ID_ANY,
        stream_flags::MAP_BUFFERS,
        &[&pod_bytes],
    )?;

    debug!("Stream Configured..");
    let port_proxies_inner = port_proxies.clone();
    let node_proxies_inner = node_proxies.clone();
    core.borrow_mut().add_done_listener(move |_, seq| {
        if clear_port_proxies_sync.borrow().as_ref() == Some(&seq) {
            debug!("Clearing Port Proxies");
            port_proxies_inner.borrow_mut().clear();
        }
        if clear_node_proxies_sync.borrow().as_ref() == Some(&seq) {
            debug!("Clearing Node Proxies");
            node_proxies_inner.borrow_mut().clear();
        }
    })?;

    debug!("Blocking on Pipewire Main Loop..");

    main_loop.borrow_mut().quit_when(stop, 20)?;
    main_loop.borrow().run();

    // Theoretically, we just drop everything here, and it'll clean up.
    debug!("Pipewire Stopped..");
    Ok(())
}

fn handle_port_cache(node: u32, mut known: RefMut<ChannelMap>, cache: Ref<Vec<PortInfo>>) {
    for port in cache.iter() {
        handle_port(node, &mut known, port);
    }
}

fn handle_port(find_node: u32, known: &mut RefMut<ChannelMap>, info: &PortInfo) {
    // Pull out some props we'll need to check
    let node_id = info.props.get("node.id").and_then(TO_U32);
    let id = info.props.get("object.id").and_then(TO_U32);
    let location = info.props.get("audio.channel").map(String::from);

    let Some(node_id) = node_id else {
        return;
    };

    let Some(id) = id else {
        return;
    };

    let Some(location) = location else {
        return;
    };

    if node_id != find_node {
        return;
    }

    if known.contains_key(&location) {
        return;
    }
    known.insert(location, id);
}

fn do_links(
    mut proxies: RefMut<Vec<PwProxy>>,
    wanted: Ref<Vec<ChannelStream>>,
    received: Ref<ChannelMap>,
    pw: Ref<PipeWire>,
    core: RefMut<PwCore>,
) {
    // We need to make sure the received ports are correctly indexed to the wanted ports, so this
    // is gonna be a little hacky. We can probably solve this later by resolving AUX to the internal
    // ID, then referencing against that.. Until then..
    for (index, src) in wanted.iter().enumerate() {
        let find_port = format!("AUX{}", index);
        if let Some(target) = received.get(&find_port) {
            let Ok(mut props) = PwProperties::new(&pw) else {
                continue;
            };
            let output = src.channel_id.to_string();
            let input = target.to_string();

            props.set("link.output.port", &output).unwrap();
            props.set("link.input.port", &input).unwrap();

            let factory = "link-factory";
            if let Ok(link) = core.create_object(factory, PW_TYPE_INTERFACE_LINK, 3, &props) {
                proxies.push(link);
            }
        }
    }
}
