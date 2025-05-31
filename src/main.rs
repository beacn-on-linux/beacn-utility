use crate::pages::MicPage;
use crate::pages::about::About;
use crate::pages::config::Configuration;
use crate::pages::error::ErrorPage;
use crate::pages::lighting::LightingPage;
use crate::state::{BeacnMicState, LoadState};
use anyhow::{Result, anyhow};
use beacn_mic_lib::manager::{
    DeviceLocation, DeviceType, HotPlugMessage, HotPlugThreadManagement, spawn_mic_hotplug_handler,
};
use eframe::Frame;
use egui::ahash::HashMap;
use egui::{Color32, Context, ImageButton, ImageSource, Response, Ui, include_image, vec2};
use log::{LevelFilter, debug, error};
use once_cell::sync::Lazy;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use std::sync::{mpsc, Arc};
use std::sync::mpsc::TryRecvError;
use std::thread;
use beacn_mic_lib::audio::{open_audio_device, BeacnAudioDevice};
use eframe::icon_data::from_png_bytes;

mod numbers;
mod pages;
mod state;
mod widgets;

// Main Window Icon
static ICON: &[u8] = include_bytes!("../resources/com.github.beacn-on-linux.png");

// SVG Images
pub static SVG: Lazy<HashMap<&'static str, ImageSource>> = Lazy::new(|| {
    let mut map = HashMap::default();
    map.insert("mic", include_image!("../resources/icons/microphone.svg"));
    map.insert("bulb", include_image!("../resources/icons/lightbulb.svg"));
    map.insert("gear", include_image!("../resources/icons/gear.svg"));
    map.insert("error", include_image!("../resources/icons/error.svg"));

    // EQ Modes
    map.insert("eq_bell", include_image!("../resources/eq/bell.svg"));
    map.insert("eq_high_pass", include_image!("../resources/eq/high_pass.svg"));
    map.insert("eq_high_shelf", include_image!("../resources/eq/high_shelf.svg"));
    map.insert("eq_low_pass", include_image!("../resources/eq/low_pass.svg"));
    map.insert("eq_low_shelf", include_image!("../resources/eq/low_shelf.svg"));
    map.insert("eq_notch", include_image!("../resources/eq/notch.svg"));
    map
});

pub struct MicConfiguration {
    pub mic: Box<dyn BeacnAudioDevice>,
    pub state: BeacnMicState,
}

impl MicConfiguration {
    pub fn new(mic: Box<dyn BeacnAudioDevice>, state: BeacnMicState) -> Self {
        Self { mic, state }
    }
}

pub struct BeacnMicApp {
    devices: HashMap<DeviceLocation, MicConfiguration>,
    active_device: Option<DeviceLocation>,
    type_map: HashMap<DeviceLocation, DeviceType>,

    hotplug_recv: mpsc::Receiver<HotPlugMessage>,
    hotplug_send: mpsc::Sender<HotPlugThreadManagement>,

    active_page: usize,
    pages: Vec<Box<dyn MicPage>>,
    // Used for icons
    //textures: HashMap<String, Image>,
}

impl BeacnMicApp {
    pub fn new(context: &Context) -> Self {
        egui_extras::install_image_loaders(context);

        // We need to spawn up the hotplug handler to get mic hotplug info
        let (plug_tx, plug_rx) = mpsc::channel();
        let (manage_tx, manage_rx) = mpsc::channel();
        let (proxy_tx, proxy_rx) = mpsc::channel();

        spawn_mic_hotplug_handler(plug_tx, manage_rx).expect("Failed to Spawn HotPlug Handler");

        // We need to proxy messages between the hotplug handler and the main context, egui will
        // not redraw if the mouse isn't inside the window, so we need to grab the messages, forward
        // them, then force a redraw.
        let context_inner = context.clone();
        thread::spawn(move || {
            loop {
                match plug_rx.recv() {
                    Ok(m) => {
                        let _ = proxy_tx.send(m);
                        context_inner.request_repaint();

                        if m == HotPlugMessage::ThreadStopped {
                            break;
                        }
                    }
                    Err(e) => {
                        // The message channel has been disconnected
                        error!("Error Received: {}", e);
                        let _ = proxy_tx.send(HotPlugMessage::ThreadStopped);
                        context_inner.request_repaint();
                        break;
                    }
                }
            }
        });

        Self {
            devices: Default::default(),
            active_device: None,
            type_map: Default::default(),

            hotplug_recv: proxy_rx,
            hotplug_send: manage_tx,

            active_page: 0,
            pages: vec![
                Box::new(Configuration::new()),
                Box::new(LightingPage::new()),
                Box::new(About::new()),
                Box::new(ErrorPage::new()),
            ],
            //textures: svgs,
        }
    }
}

impl eframe::App for BeacnMicApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        // Here is where we should deal with any messages from the Mic :p
        match self.hotplug_recv.try_recv() {
            Ok(msg) => {
                match msg {
                    HotPlugMessage::DeviceAttached(location, device_type) => {
                        // Device has been found / attached, lets handle it.
                        let device = open_audio_device(location).expect("Unable to open Device");
                        let state = BeacnMicState::load_settings(&device, device_type);

                        // Add to our type map
                        self.type_map.insert(location, state.device_type);

                        // Add to state
                        self.devices
                            .insert(location, MicConfiguration::new(device, state));
                        if self.active_device.is_none() {
                            self.active_device = Some(location);
                        }


                    }
                    HotPlugMessage::DeviceRemoved(d) => {
                        // Device removed, update our states
                        self.devices.remove(&d);
                        self.type_map.remove(&d);
                        if self.active_device == Some(d) {
                            if self.devices.iter().len() == 0 {
                                self.active_device = None;
                            } else {
                                // Switch to the first device
                                let dev = self.devices.keys().next().unwrap();
                                self.active_device = Some(*dev)
                            }
                        }
                    }
                    HotPlugMessage::ThreadStopped => {
                        debug!("HotPlug Thread Stopped..");
                    }
                }
            }
            Err(e) => match e {
                TryRecvError::Empty => {}
                TryRecvError::Disconnected => {}
            },
        }

        // Do we have an active device? (Should be no if there are no devices)
        if self.active_device.is_none() {
            egui::CentralPanel::default().show(ctx, |ui: &mut Ui| {
                ui.add_sized(ui.available_size(), |ui: &mut Ui| {
                    ui.label("No Devices Detected")
                });
            });
            return;
        }



        // Grab the active device and its settings
        let device_keys: Vec<DeviceLocation> = self.devices.keys().cloned().collect();
        let active_device = &self.active_device.unwrap();
        //

        egui::SidePanel::left("left_panel")
            .resizable(false)
            .default_width(80.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // We need to iterate between devices and pages
                    for device in device_keys {
                        let device_state = self.devices.get(&device).unwrap();

                        ui.add_space(5.0);
                        if let Some(state) = self.type_map.get(&device) {
                            match state {
                                DeviceType::BeacnMic => ui.label("Mic"),
                                DeviceType::BeacnStudio => ui.label("Studio"),
                            };
                        };

                        for (index, page) in self.pages.iter().enumerate() {
                            let selected = active_device == &device && self.active_page == index;
                            let error = &device_state.state.device_state.state == &LoadState::ERROR;

                            if page.show_on_error() == error {
                                if round_nav_button(ui, page.icon(), selected).clicked() {
                                    self.active_device = Some(device);
                                    self.active_page = index;
                                }
                            }
                        }
                        ui.separator();
                    }
                })
            });

        let settings = self.devices.get_mut(&active_device).unwrap();

        // If we're in error, we need to force our index to an error page.
        if settings.state.device_state.state == LoadState::ERROR {
            if let Some(page) = self.pages.iter().position(|p| p.show_on_error()) {
                self.active_page = page;
            }

        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.pages[self.active_page].ui(ui, &settings.mic, &mut settings.state);
        });
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.hotplug_send.send(HotPlugThreadManagement::Quit);
    }
}

fn main() -> Result<()> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])?;

    let viewport = egui::ViewportBuilder::default();
    let viewport = viewport.with_inner_size([1024.0, 500.0]);
    let mut viewport = viewport.with_min_inner_size([1024.0, 500.0]);

    // This is only for X11, on Wayland the icon is inherited from the .desktop file
    if let Ok(icon) = from_png_bytes(ICON) {
        viewport.icon = Some(Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Beacn Mic Configuration",
        options,
        Box::new(|cc| Ok(Box::new(BeacnMicApp::new(&cc.egui_ctx)))),
    )
    .map_err(|e| anyhow!("Failed: {}", e))?;

    Ok(())
}

fn round_nav_button(ui: &mut Ui, img: &str, active: bool) -> Response {
    let tint_colour = if active {
        Color32::WHITE
    } else {
        Color32::from_rgb(120, 120, 120)
    };

    // We might need to do caching here..
    let image = SVG.get(img).unwrap().clone();

    ui.scope(|ui| {
        ui.style_mut().spacing.button_padding = vec2(10.0, 10.0);
        ui.add_sized(
            [40.0, 40.0],
            ImageButton::new(image)
                .corner_radius(5.0)
                .tint(tint_colour)
                .selected(active),
        )
    })
    .inner
}

