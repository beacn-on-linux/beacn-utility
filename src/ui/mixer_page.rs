//! Mixer UI page for Pipeweaver audio routing.
//!
//! Renders source channel strips, application routing, output routing matrix,
//! and a header with connection status / mix selection.

use crate::ui::states::pipeweaver_state::SharedPipeweaverState;
use egui::{Color32, ComboBox, Grid, RichText, ScrollArea, Ui, Vec2};
use pipeweaver_ipc::commands::APICommand;
use pipeweaver_shared::{AppDefinition, AppTarget, DeviceType, Mix, MuteTarget};
use std::path::PathBuf;
use ulid::Ulid;

// ─── Autostart helpers ───────────────────────────────────────────────────────

/// Path to Pipeweaver's XDG autostart desktop file.
/// Matches the path used by pipeweaver-daemon itself.
fn pipeweaver_autostart_path() -> Option<PathBuf> {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.config")))
        .ok()?;
    Some(PathBuf::from(format!(
        "{config_dir}/autostart/io.github.pipeweaver.pipeweaver.desktop"
    )))
}

/// Returns true if the Pipeweaver autostart .desktop file exists.
pub fn pipeweaver_autostart_enabled() -> bool {
    pipeweaver_autostart_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Creates or removes the Pipeweaver autostart .desktop file.
fn set_pipeweaver_autostart(enabled: bool) {
    let Some(path) = pipeweaver_autostart_path() else {
        return;
    };

    if !enabled {
        let _ = std::fs::remove_file(&path);
        return;
    }

    // Create the autostart directory if needed
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Find the pipeweaver-daemon executable on PATH
    let exe = which_pipeweaver_daemon();
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Pipeweaver\n\
         Comment=Audio Control and Routing\nExec={exe} --background\nTerminal=false\n"
    );
    let _ = std::fs::write(&path, content);
}

fn which_pipeweaver_daemon() -> String {
    // Try $PATH lookup first, fall back to a common install location
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

// ─── Local page state ────────────────────────────────────────────────────────

/// Local UI state for the mixer page (not shared with handler).
pub struct MixerPageState {
    /// Which mix is currently displayed in the channel-strip volume sliders.
    pub active_mix: Mix,
    /// Mirrors whether Pipeweaver's XDG autostart .desktop file exists.
    pub autostart_enabled: bool,
}

impl Default for MixerPageState {
    fn default() -> Self {
        Self {
            active_mix: Mix::default(),
            autostart_enabled: pipeweaver_autostart_enabled(),
        }
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

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

        // Build a combined flat list of source channel info for reuse across sections.
        let source_channels = build_source_channels(sources);

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

// ─── Header ──────────────────────────────────────────────────────────────────

fn draw_header(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    page_state: &mut MixerPageState,
    connected: bool,
) {
    ui.horizontal(|ui| {
        // Connection indicator
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

        // Coloured circle glyph as a cheap status dot
        ui.label(RichText::new("●").color(dot_colour).size(14.0));
        ui.label(
            RichText::new(format!("{status_text}{error_suffix}"))
                .color(Color32::from_rgb(200, 200, 200)),
        );

        ui.add_space(16.0);

        // Mix A / Mix B toggle
        ui.label(RichText::new("Mix:").color(Color32::from_rgb(160, 160, 160)));

        let mix_a_active = page_state.active_mix == Mix::A;
        if ui
            .add(egui::Button::new(
                RichText::new("A").color(if mix_a_active {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(140, 140, 140)
                }),
            ).fill(if mix_a_active {
                Color32::from_rgb(50, 100, 180)
            } else {
                Color32::from_rgb(40, 40, 40)
            }))
            .clicked()
        {
            page_state.active_mix = Mix::A;
        }

        let mix_b_active = page_state.active_mix == Mix::B;
        if ui
            .add(egui::Button::new(
                RichText::new("B").color(if mix_b_active {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(140, 140, 140)
                }),
            ).fill(if mix_b_active {
                Color32::from_rgb(50, 100, 180)
            } else {
                Color32::from_rgb(40, 40, 40)
            }))
            .clicked()
        {
            page_state.active_mix = Mix::B;
        }

        ui.add_space(16.0);

        // Autostart checkbox — creates/removes Pipeweaver's XDG autostart .desktop file
        let mut auto = page_state.autostart_enabled;
        if ui.checkbox(&mut auto, "Autostart Pipeweaver").changed() {
            page_state.autostart_enabled = auto;
            set_pipeweaver_autostart(auto);
        }
    });
}

// ─── Source channel strips ────────────────────────────────────────────────────

/// Lightweight description of a source channel for passing between sections.
struct SourceChannel {
    id: Ulid,
    name: String,
    colour: Color32,
    volume_a: u8,
    volume_b: u8,
    muted: bool,
    /// App node_ids that are currently routed to this channel.
    app_count: usize,
}

fn build_source_channels(
    sources: &pipeweaver_profile::SourceDevices,
) -> Vec<SourceChannel> {
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
            app_count: 0, // filled in below if needed
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
            app_count: 0,
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
            // Coloured header bar with channel name
            let (rect, _response) = ui.allocate_exact_size(
                Vec2::new(strip_width, 22.0),
                egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, 3.0, ch.colour);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &ch.name,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );

            ui.add_space(4.0);

            // Volume slider for the active mix
            let current_volume = match page_state.active_mix {
                Mix::A => ch.volume_a,
                Mix::B => ch.volume_b,
            };
            let mut vol = current_volume as f32;

            ui.label(
                RichText::new(format!("{}%", current_volume))
                    .size(10.0)
                    .color(Color32::from_rgb(180, 180, 180)),
            );

            let slider = egui::Slider::new(&mut vol, 0.0..=100.0)
                .vertical()
                .show_value(false);
            if ui.add(slider).changed() {
                let new_vol = vol as u8;
                state.send_command(APICommand::SetSourceVolume(
                    ch.id,
                    page_state.active_mix,
                    new_vol,
                ));
            }

            ui.add_space(4.0);

            // Mute toggle
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
                .add(egui::Button::new(mute_label).fill(mute_colour).min_size(Vec2::new(strip_width - 8.0, 20.0)))
                .clicked()
            {
                if ch.muted {
                    state.send_command(APICommand::DelSourceMuteTarget(ch.id, MuteTarget::TargetA));
                } else {
                    state.send_command(APICommand::AddSourceMuteTarget(ch.id, MuteTarget::TargetA));
                }
            }

            // App count hint
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

    // Thin vertical separator between strips
    ui.separator();
}

// ─── Application routing ──────────────────────────────────────────────────────

fn draw_application_routing(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    apps: &enum_map::EnumMap<DeviceType, std::collections::HashMap<String, std::collections::HashMap<String, Vec<pipeweaver_ipc::commands::Application>>>>,
    channels: &[SourceChannel],
) {
    ui.label(RichText::new("Application Routing").size(13.0).color(Color32::from_rgb(200, 200, 200)));

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
            // Build the list of (channel_id, channel_name) options for the combo box.
            let mut route_options: Vec<(Option<Ulid>, String)> = vec![(None, "Unrouted".to_owned())];
            for ch in channels {
                route_options.push((Some(ch.id), ch.name.clone()));
            }

            for (process_name, streams) in source_apps {
                for (stream_name, app_list) in streams {
                    for app in app_list {
                        ui.horizontal(|ui| {
                            // App name + optional title
                            let display_name = app
                                .title
                                .as_deref()
                                .unwrap_or(app.name.as_str());
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

                            // Current route selection
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
                            // Use persistent routing (by process + stream name)
                            // so assignments survive app restarts.
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
                                        if ui.selectable_label(selected, opt_name.as_str()).clicked() {
                                            match opt_id {
                                                Some(channel_id) => {
                                                    state.send_command(
                                                        APICommand::SetApplicationRoute(
                                                            app_def.clone(),
                                                            *channel_id,
                                                        ),
                                                    );
                                                }
                                                None => {
                                                    state.send_command(
                                                        APICommand::ClearApplicationRoute(
                                                            app_def.clone(),
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                });

                            ui.add_space(8.0);

                            // Volume slider
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

                            // Mute checkbox
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

// ─── Output routing matrix ────────────────────────────────────────────────────

fn draw_output_routing(
    ui: &mut Ui,
    state: &SharedPipeweaverState,
    targets: &pipeweaver_profile::TargetDevices,
    routes: &std::collections::HashMap<Ulid, std::collections::HashSet<Ulid>>,
    channels: &[SourceChannel],
) {
    ui.label(RichText::new("Output Routing").size(13.0).color(Color32::from_rgb(200, 200, 200)));

    if channels.is_empty() {
        ui.label(
            RichText::new("No source channels to route.")
                .color(Color32::from_rgb(140, 140, 140)),
        );
        return;
    }

    // Combine physical + virtual targets into a flat list
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
            // The routes map is keyed by source_id → HashSet<target_id>
            Grid::new("routing_matrix")
                .striped(true)
                .spacing([6.0, 4.0])
                .show(ui, |ui| {
                    // Header row: blank cell then one cell per source channel
                    ui.label(""); // corner
                    for ch in channels {
                        ui.label(
                            RichText::new(&ch.name)
                                .size(10.0)
                                .color(ch.colour),
                        );
                    }
                    ui.end_row();

                    // One row per target
                    for target in &target_rows {
                        ui.label(
                            RichText::new(&target.name)
                                .size(11.0)
                                .color(Color32::from_rgb(200, 200, 200)),
                        );
                        for ch in channels {
                            // A route from source → target is stored as routes[source_id] containing target_id
                            let enabled = routes
                                .get(&ch.id)
                                .map(|targets_set| targets_set.contains(&target.id))
                                .unwrap_or(false);

                            let mut checked = enabled;
                            let source_id = ch.id;
                            let target_id = target.id;
                            if ui.checkbox(&mut checked, "").changed() {
                                state.send_command(APICommand::SetRoute(
                                    source_id,
                                    target_id,
                                    checked,
                                ));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

// ─── Footer ───────────────────────────────────────────────────────────────────

fn draw_footer(ui: &mut Ui) {
    ui.label(
        RichText::new("Changes are automatically saved by Pipeweaver.")
            .size(10.0)
            .color(Color32::from_rgb(120, 120, 120)),
    );
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Convert a pipeweaver `Colour` to an egui `Color32`.
fn colour32(c: &pipeweaver_shared::Colour) -> Color32 {
    Color32::from_rgb(c.red, c.green, c.blue)
}
