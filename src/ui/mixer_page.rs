//! Mixer UI page for Pipeweaver audio routing.
//!
//! Renders source channel strips, channel management, application routing,
//! output routing matrix, and a header with connection status / mix selection.

use crate::ui::states::pipeweaver_state::SharedPipeweaverState;
use egui::{Color32, ComboBox, Grid, RichText, ScrollArea, Ui, Vec2};
use pipeweaver_ipc::commands::{APICommand, Application, PhysicalDevice};
use pipeweaver_shared::{AppDefinition, AppTarget, DeviceType, Mix, MuteTarget, NodeType};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use ulid::Ulid;

fn pipeweaver_autostart_path() -> Option<PathBuf> {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.config")))
        .ok()?;
    Some(PathBuf::from(format!(
        "{config_dir}/autostart/io.github.pipeweaver.pipeweaver.desktop"
    )))
}

pub fn pipeweaver_autostart_enabled() -> bool {
    pipeweaver_autostart_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

fn set_pipeweaver_autostart(enabled: bool) {
    let Some(path) = pipeweaver_autostart_path() else {
        return;
    };

    if !enabled {
        let _ = std::fs::remove_file(&path);
        return;
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let exe = which_pipeweaver_daemon();
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Pipeweaver\n\
         Comment=Audio Control and Routing\nExec={exe} --background\nTerminal=false\n"
    );
    let _ = std::fs::write(&path, content);
}

fn which_pipeweaver_daemon() -> String {
    if let Ok(output) = std::process::Command::new("which").arg("pipeweaver").output() {
        if output.status.success() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                let trimmed = s.trim().to_string();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
        }
    }
    "pipeweaver".to_string()
}

pub struct MixerPageState {
    pub active_mix: Mix,
    pub autostart_enabled: bool,
    pub new_channel_name: String,
    pub selected_new_channel_input: Option<u32>,
    pub selected_manage_channel: Option<Ulid>,
    pub selected_manage_input: Option<u32>,
}

impl Default for MixerPageState {
    fn default() -> Self {
        Self {
            active_mix: Mix::default(),
            autostart_enabled: pipeweaver_autostart_enabled(),
            new_channel_name: String::new(),
            selected_new_channel_input: None,
            selected_manage_channel: None,
            selected_manage_input: None,
        }
    }
}

pub fn mixer_ui(ui: &mut Ui, state: &SharedPipeweaverState, page_state: &mut MixerPageState) {
    let snap = state.snapshot();

    draw_header(ui, state, page_state, snap.connected);

    ui.separator();

    if let Some(ref status) = snap.status {
        let profile = &status.audio.profile;
        let sources = &profile.devices.sources;
        let targets = &profile.devices.targets;
        let routes = &profile.routes;
        let apps = &status.audio.applications;
        let physical_source_devices = &status.audio.devices[DeviceType::Source];

        let source_channels = build_source_channels(sources, apps);

        draw_channel_management(
            ui,
            state,
            page_state,
            &source_channels,
            physical_source_devices,
        );

        ui.separator();
        draw_source_strips(ui, state, page_state, &source_channels);

        ui.separator();
        draw_application_routing(ui, state, apps, &source_channels);

        ui.separator();
        draw_output_routing(ui, state, targets, routes, &source_channels);

        ui.separator();
        draw_footer(ui);
    } else {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("Waiting for Pipeweaver status…")
                    .color(Color32::from_rgb(180, 180, 180)),
            );
        });
    }
}

fn draw_header(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    page_state: &mut MixerPageState,
    connected: bool,
) {
    ui.horizontal(|ui| {
        let (dot_colour, status_text) = if connected {
            (Color32::from_rgb(80, 200, 80), "Connected")
        } else {
            (Color32::from_rgb(220, 60, 60), "Disconnected")
        };

        let snap = state.snapshot();
        let error_suffix = snap
            .error
            .as_deref()
            .map(|e| format!(" — {e}"))
            .unwrap_or_default();

        ui.label(RichText::new("●").color(dot_colour).size(14.0));
        ui.label(
            RichText::new(format!("{status_text}{error_suffix}"))
                .color(Color32::from_rgb(200, 200, 200)),
        );

        ui.add_space(16.0);

        ui.label(RichText::new("Mix:").color(Color32::from_rgb(160, 160, 160)));

        let mix_a_active = page_state.active_mix == Mix::A;
        if ui
            .add(
                egui::Button::new(RichText::new("A").color(if mix_a_active {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(140, 140, 140)
                }))
                .fill(if mix_a_active {
                    Color32::from_rgb(50, 100, 180)
                } else {
                    Color32::from_rgb(40, 40, 40)
                }),
            )
            .clicked()
        {
            page_state.active_mix = Mix::A;
        }

        let mix_b_active = page_state.active_mix == Mix::B;
        if ui
            .add(
                egui::Button::new(RichText::new("B").color(if mix_b_active {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(140, 140, 140)
                }))
                .fill(if mix_b_active {
                    Color32::from_rgb(50, 100, 180)
                } else {
                    Color32::from_rgb(40, 40, 40)
                }),
            )
            .clicked()
        {
            page_state.active_mix = Mix::B;
        }

        ui.add_space(16.0);

        let mut auto = page_state.autostart_enabled;
        if ui.checkbox(&mut auto, "Autostart Pipeweaver").changed() {
            page_state.autostart_enabled = auto;
            set_pipeweaver_autostart(auto);
        }
    });
}

#[derive(Clone)]
struct PhysicalSourceOption {
    node_id: u32,
    label: String,
}

fn build_physical_source_options(devices: &[PhysicalDevice]) -> Vec<PhysicalSourceOption> {
    devices
        .iter()
        .filter(|dev| dev.is_usable)
        .map(|dev| PhysicalSourceOption {
            node_id: dev.node_id,
            label: dev
                .description
                .clone()
                .or_else(|| dev.name.clone())
                .unwrap_or_else(|| format!("Node {}", dev.node_id)),
        })
        .collect()
}

fn draw_channel_management(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    page_state: &mut MixerPageState,
    channels: &[SourceChannel],
    physical_source_devices: &[PhysicalDevice],
) {
    ui.label(
        RichText::new("Channel Management")
            .size(13.0)
            .color(Color32::from_rgb(200, 200, 200)),
    );
    ui.label(
        RichText::new(
            "Create virtual source channels like Guitar or Music, then optionally attach a physical input.",
        )
        .size(10.0)
        .color(Color32::from_rgb(140, 140, 140)),
    );
    ui.add_space(8.0);

    let physical_inputs = build_physical_source_options(physical_source_devices);
    let virtual_channels: Vec<&SourceChannel> = channels.iter().filter(|c| c.is_virtual).collect();

    ui.group(|ui| {
        ui.label(RichText::new("Create Channel").strong());

        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut page_state.new_channel_name);

            let selected_input_label = page_state
                .selected_new_channel_input
                .and_then(|node_id| {
                    physical_inputs
                        .iter()
                        .find(|dev| dev.node_id == node_id)
                        .map(|dev| dev.label.clone())
                })
                .unwrap_or_else(|| "No input".to_string());

            ComboBox::from_id_salt("new_channel_input")
                .selected_text(selected_input_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut page_state.selected_new_channel_input,
                        None,
                        "No input",
                    );
                    for dev in &physical_inputs {
                        ui.selectable_value(
                            &mut page_state.selected_new_channel_input,
                            Some(dev.node_id),
                            dev.label.as_str(),
                        );
                    }
                });

            let trimmed = page_state.new_channel_name.trim().to_string();
            let create_enabled = !trimmed.is_empty();
            if ui
                .add_enabled(create_enabled, egui::Button::new("Create"))
                .clicked()
            {
                state.send_command(APICommand::CreateNode(
                    NodeType::VirtualSource,
                    trimmed.clone(),
                ));

                if let Some(node_id) = page_state.selected_new_channel_input {
                    state.send_command(APICommand::AttachPhysicalNodeByName(
                        trimmed.clone(),
                        node_id,
                    ));
                }

                page_state.new_channel_name.clear();
                page_state.selected_new_channel_input = None;
            }
        });
    });

    ui.add_space(8.0);

    ui.group(|ui| {
        ui.label(RichText::new("Manage Existing Channels").strong());

        ui.horizontal(|ui| {
            let selected_channel_label = page_state
                .selected_manage_channel
                .and_then(|id| {
                    virtual_channels
                        .iter()
                        .find(|channel| channel.id == id)
                        .map(|channel| channel.name.clone())
                })
                .unwrap_or_else(|| "Select channel".to_string());

            ComboBox::from_id_salt("manage_channel")
                .selected_text(selected_channel_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut page_state.selected_manage_channel,
                        None,
                        "Select channel",
                    );
                    for channel in &virtual_channels {
                        ui.selectable_value(
                            &mut page_state.selected_manage_channel,
                            Some(channel.id),
                            channel.name.as_str(),
                        );
                    }
                });

            let selected_manage_input_label = page_state
                .selected_manage_input
                .and_then(|node_id| {
                    physical_inputs
                        .iter()
                        .find(|dev| dev.node_id == node_id)
                        .map(|dev| dev.label.clone())
                })
                .unwrap_or_else(|| "Attach input".to_string());

            ComboBox::from_id_salt("manage_channel_input")
                .selected_text(selected_manage_input_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut page_state.selected_manage_input, None, "Attach input");
                    for dev in &physical_inputs {
                        ui.selectable_value(
                            &mut page_state.selected_manage_input,
                            Some(dev.node_id),
                            dev.label.as_str(),
                        );
                    }
                });

            let selected_virtual_channel = page_state
                .selected_manage_channel
                .and_then(|id| virtual_channels.iter().find(|channel| channel.id == id).copied());

            if ui
                .add_enabled(
                    selected_virtual_channel.is_some() && page_state.selected_manage_input.is_some(),
                    egui::Button::new("Attach Input"),
                )
                .clicked()
            {
                if let (Some(channel), Some(node_id)) =
                    (selected_virtual_channel, page_state.selected_manage_input)
                {
                    state.send_command(APICommand::AttachPhysicalNodeByName(
                        channel.name.clone(),
                        node_id,
                    ));
                }
            }

            if ui
                .add_enabled(selected_virtual_channel.is_some(), egui::Button::new("Delete"))
                .clicked()
            {
                if let Some(channel) = selected_virtual_channel {
                    state.send_command(APICommand::RemoveNode(channel.id));
                    page_state.selected_manage_channel = None;
                    page_state.selected_manage_input = None;
                }
            }
        });
    });
}

struct SourceChannel {
    id: Ulid,
    name: String,
    colour: Color32,
    volume_a: u8,
    volume_b: u8,
    muted: bool,
    app_count: usize,
    is_virtual: bool,
}

fn build_source_channels(
    sources: &pipeweaver_profile::SourceDevices,
    apps: &enum_map::EnumMap<DeviceType, HashMap<String, HashMap<String, Vec<Application>>>>,
) -> Vec<SourceChannel> {
    let mut app_counts: HashMap<Ulid, usize> = HashMap::new();

    for streams in apps[DeviceType::Source].values() {
        for app_list in streams.values() {
            for app in app_list {
                if let Some(AppTarget::Managed(id)) = app.target.as_ref() {
                    *app_counts.entry(*id).or_insert(0) += 1;
                }
            }
        }
    }

    let mut channels: Vec<SourceChannel> = Vec::new();

    for dev in &sources.physical_devices {
        channels.push(SourceChannel {
            id: dev.description.id,
            name: dev.description.name.clone(),
            colour: colour32(&dev.description.colour),
            volume_a: dev.volumes.volume[Mix::A],
            volume_b: dev.volumes.volume[Mix::B],
            muted: dev.mute_states.mute_state.contains(&MuteTarget::TargetA)
                || dev.mute_states.mute_state.contains(&MuteTarget::TargetB),
            app_count: app_counts.get(&dev.description.id).copied().unwrap_or(0),
            is_virtual: false,
        });
    }

    for dev in &sources.virtual_devices {
        channels.push(SourceChannel {
            id: dev.description.id,
            name: dev.description.name.clone(),
            colour: colour32(&dev.description.colour),
            volume_a: dev.volumes.volume[Mix::A],
            volume_b: dev.volumes.volume[Mix::B],
            muted: dev.mute_states.mute_state.contains(&MuteTarget::TargetA)
                || dev.mute_states.mute_state.contains(&MuteTarget::TargetB),
            app_count: app_counts.get(&dev.description.id).copied().unwrap_or(0),
            is_virtual: true,
        });
    }

    channels
}

fn draw_source_strips(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    page_state: &MixerPageState,
    channels: &[SourceChannel],
) {
    ui.label(RichText::new("Sources").size(13.0).color(Color32::from_rgb(200, 200, 200)));

    if channels.is_empty() {
        ui.label(
            RichText::new("No source channels configured.")
                .color(Color32::from_rgb(140, 140, 140)),
        );
        return;
    }

    ScrollArea::horizontal()
        .id_salt("source_strips_scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for ch in channels {
                    draw_single_strip(ui, state, page_state, ch);
                }
            });
        });
}

fn draw_single_strip(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    page_state: &MixerPageState,
    ch: &SourceChannel,
) {
    let strip_width = 120.0_f32;
    let strip_height = 200.0_f32;

    ui.allocate_ui(Vec2::new(strip_width, strip_height), |ui| {
        ui.vertical(|ui| {
            let (rect, _response) = ui.allocate_exact_size(
                Vec2::new(strip_width, 22.0),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(rect, 3.0, ch.colour);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &ch.name,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );

            ui.add_space(4.0);

            let current_volume = match page_state.active_mix {
                Mix::A => ch.volume_a,
                Mix::B => ch.volume_b,
            };
            let mut vol = current_volume as f32;

            ui.label(
                RichText::new(format!("{current_volume}%"))
                    .size(10.0)
                    .color(Color32::from_rgb(180, 180, 180)),
            );

            let slider = egui::Slider::new(&mut vol, 0.0..=100.0)
                .vertical()
                .show_value(false);
            if ui.add(slider).changed() {
                state.send_command(APICommand::SetSourceVolume(
                    ch.id,
                    page_state.active_mix,
                    vol as u8,
                ));
            }

            ui.add_space(4.0);

            let mute_colour = if ch.muted {
                Color32::from_rgb(200, 60, 60)
            } else {
                Color32::from_rgb(60, 60, 60)
            };
            let mute_label = RichText::new(if ch.muted { "MUTED" } else { "MUTE" })
                .size(10.0)
                .color(if ch.muted {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(160, 160, 160)
                });
            if ui
                .add(
                    egui::Button::new(mute_label)
                        .fill(mute_colour)
                        .min_size(Vec2::new(strip_width - 8.0, 20.0)),
                )
                .clicked()
            {
                if ch.muted {
                    state.send_command(APICommand::DelSourceMuteTarget(ch.id, MuteTarget::TargetA));
                } else {
                    state.send_command(APICommand::AddSourceMuteTarget(ch.id, MuteTarget::TargetA));
                }
            }

            if ch.is_virtual {
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Virtual")
                        .size(9.0)
                        .color(Color32::from_rgb(120, 120, 120)),
                );
            }

            if ch.app_count > 0 {
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!("{} app(s)", ch.app_count))
                        .size(9.0)
                        .color(Color32::from_rgb(120, 120, 120)),
                );
            }
        });
    });

    ui.separator();
}

fn draw_application_routing(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    apps: &enum_map::EnumMap<DeviceType, HashMap<String, HashMap<String, Vec<Application>>>>,
    channels: &[SourceChannel],
) {
    ui.label(
        RichText::new("Application Routing")
            .size(13.0)
            .color(Color32::from_rgb(200, 200, 200)),
    );

    let source_apps = &apps[DeviceType::Source];

    if source_apps.is_empty() {
        ui.label(
            RichText::new("No applications detected.")
                .color(Color32::from_rgb(140, 140, 140)),
        );
        return;
    }

    ScrollArea::vertical()
        .id_salt("app_routing_scroll")
        .max_height(220.0)
        .show(ui, |ui| {
            let mut route_options: Vec<(Option<Ulid>, String)> = vec![(None, "Unrouted".to_owned())];
            for ch in channels {
                route_options.push((Some(ch.id), ch.name.clone()));
            }

            for (process_name, streams) in source_apps {
                for (stream_name, app_list) in streams {
                    for app in app_list {
                        ui.horizontal(|ui| {
                            let display_name = app.title.as_deref().unwrap_or(app.name.as_str());
                            ui.label(
                                RichText::new(format!("{process_name} / {stream_name}"))
                                    .size(10.0)
                                    .color(Color32::from_rgb(140, 140, 140)),
                            );
                            ui.label(
                                RichText::new(display_name)
                                    .size(11.0)
                                    .color(Color32::from_rgb(210, 210, 210)),
                            );

                            ui.add_space(8.0);

                            let current_route: Option<Ulid> =
                                app.target.as_ref().and_then(|t| match t {
                                    AppTarget::Managed(id) => Some(*id),
                                    AppTarget::Unmanaged(_) => None,
                                });

                            let current_label = route_options
                                .iter()
                                .find(|(id, _)| *id == current_route)
                                .map(|(_, name)| name.as_str())
                                .unwrap_or("Unrouted");

                            let node_id = app.node_id;
                            let app_def = AppDefinition {
                                device_type: DeviceType::Source,
                                process: process_name.clone(),
                                name: stream_name.clone(),
                            };

                            ComboBox::from_id_salt(format!("app_route_{node_id}"))
                                .selected_text(current_label)
                                .show_ui(ui, |ui| {
                                    for (opt_id, opt_name) in &route_options {
                                        let selected = *opt_id == current_route;
                                        if ui.selectable_label(selected, opt_name.as_str()).clicked()
                                        {
                                            match opt_id {
                                                Some(channel_id) => state.send_command(
                                                    APICommand::SetApplicationRoute(
                                                        app_def.clone(),
                                                        *channel_id,
                                                    ),
                                                ),
                                                None => state.send_command(
                                                    APICommand::ClearApplicationRoute(
                                                        app_def.clone(),
                                                    ),
                                                ),
                                            }
                                        }
                                    }
                                });

                            ui.add_space(8.0);

                            let mut vol = app.volume as f32;
                            let vol_slider = egui::Slider::new(&mut vol, 0.0..=100.0)
                                .text("vol")
                                .clamping(egui::SliderClamping::Always);
                            if ui.add(vol_slider).changed() {
                                state.send_command(APICommand::SetApplicationVolume(
                                    node_id,
                                    vol as u8,
                                ));
                            }

                            ui.add_space(4.0);

                            let mut muted = app.muted;
                            if ui.checkbox(&mut muted, "Mute").changed() {
                                state.send_command(APICommand::SetApplicationMute(node_id, muted));
                            }
                        });
                    }
                }
            }
        });
}

fn draw_output_routing(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    targets: &pipeweaver_profile::TargetDevices,
    routes: &HashMap<Ulid, HashSet<Ulid>>,
    channels: &[SourceChannel],
) {
    ui.label(
        RichText::new("Output Routing")
            .size(13.0)
            .color(Color32::from_rgb(200, 200, 200)),
    );

    if channels.is_empty() {
        ui.label(
            RichText::new("No source channels to route.")
                .color(Color32::from_rgb(140, 140, 140)),
        );
        return;
    }

    struct TargetRow {
        id: Ulid,
        name: String,
    }

    let mut target_rows: Vec<TargetRow> = Vec::new();
    for dev in &targets.physical_devices {
        target_rows.push(TargetRow {
            id: dev.description.id,
            name: dev.description.name.clone(),
        });
    }
    for dev in &targets.virtual_devices {
        target_rows.push(TargetRow {
            id: dev.description.id,
            name: dev.description.name.clone(),
        });
    }

    if target_rows.is_empty() {
        ui.label(
            RichText::new("No output targets configured.")
                .color(Color32::from_rgb(140, 140, 140)),
        );
        return;
    }

    ScrollArea::both()
        .id_salt("output_routing_scroll")
        .max_height(200.0)
        .show(ui, |ui| {
            Grid::new("routing_matrix")
                .striped(true)
                .spacing([6.0, 4.0])
                .show(ui, |ui| {
                    ui.label("");
                    for ch in channels {
                        ui.label(RichText::new(&ch.name).size(10.0).color(ch.colour));
                    }
                    ui.end_row();

                    for target in &target_rows {
                        ui.label(
                            RichText::new(&target.name)
                                .size(11.0)
                                .color(Color32::from_rgb(200, 200, 200)),
                        );
                        for ch in channels {
                            let enabled = routes
                                .get(&ch.id)
                                .map(|targets_set| targets_set.contains(&target.id))
                                .unwrap_or(false);

                            let mut checked = enabled;
                            if ui.checkbox(&mut checked, "").changed() {
                                state.send_command(APICommand::SetRoute(
                                    ch.id,
                                    target.id,
                                    checked,
                                ));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

fn draw_footer(ui: &mut Ui) {
    ui.label(
        RichText::new("Changes are automatically saved by Pipeweaver.")
            .size(10.0)
            .color(Color32::from_rgb(120, 120, 120)),
    );
}

fn colour32(c: &pipeweaver_shared::Colour) -> Color32 {
    Color32::from_rgb(c.red, c.green, c.blue)
}
