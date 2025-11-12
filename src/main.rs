use crate::device_manager::spawn_device_manager;
use crate::managers::ipc::{handle_active_instance, handle_ipc};
use crate::ui::app::BeacnMicApp;
use crate::window_handle::{App, UserEvent, WindowRunner, send_user_event};
use anyhow::Result;
use anyhow::bail;
use beacn_lib::crossbeam::{channel, select};
use egui::{Context, Id};
use egui_winit::winit::dpi::LogicalSize;
use egui_winit::winit::event_loop::EventLoop;
use egui_winit::winit::platform::x11::{EventLoopBuilderExtX11, WindowAttributesExtX11};
use egui_winit::winit::window::{Icon, Window};
use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use log::{LevelFilter, debug, error, info};
use managers::tray::handle_tray;
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger,
};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::{env, thread};
use tokio::runtime::{Builder, Runtime};
use xdg::BaseDirectories;

mod device_manager;
mod integrations;
mod managers;
mod ui;
mod window_handle;

const APP_TLD: &str = "io.github.beacn_on_linux";
const APP_NAME: &str = "beacn-utility";
const APP_TITLE: &str = "Beacn Utility";
const AUTO_START_KEY: &str = "autostart";
const ICON: &[u8] = include_bytes!("../resources/icons/beacn-utility-large.png");

static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();
pub fn runtime() -> &'static Runtime {
    TOKIO_RUNTIME.get_or_init(|| Builder::new_multi_thread().enable_all().build().unwrap())
}
pub fn run_async_blocking<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

fn main() -> Result<()> {
    println!("Initialising Logging...");
    let mut log_targets: Vec<Box<dyn SharedLogger>> = vec![];

    let mut config = ConfigBuilder::new();
    // The tracing package, when used, will output to INFO from zbus every second..
    config.add_filter_ignore_str("tracing");
    config.add_filter_ignore_str("winit::event_loop");
    config.add_filter_ignore_str("winit::window");
    config.add_filter_ignore_str("zbus");

    // Setup Console Logging
    log_targets.push(TermLogger::new(
        LevelFilter::Debug,
        config.build(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    ));

    // Try to establish a log file in the XDG data directory
    let xdg_dirs = BaseDirectories::with_prefix(APP_TLD);
    let log_path = xdg_dirs.create_data_directory(PathBuf::from("logs"));
    if let Ok(path) = log_path {
        let log_file = path.join("beacn-utility.log");
        println!("Logging to file: {log_file:?}");

        let file_rotate = FileRotate::new(
            log_file,
            AppendCount::new(5),
            ContentLimit::Bytes(1024 * 1024 * 2),
            Compression::OnRotate(1),
            #[cfg(unix)]
            None,
        );
        log_targets.push(WriteLogger::new(
            LevelFilter::Debug,
            config.build(),
            file_rotate,
        ));
    }

    CombinedLogger::init(log_targets)?;
    info!("Logger initialized");

    // Install a PANIC logger, to hopefully drop info if something breaks
    log_panics::init();

    let args: Vec<String> = env::args().collect();
    let hide_initial = args.contains(&"--startup".to_string());

    // Firstly, create a message bus which allows threads to message back to here
    let (main_tx, main_rx) = channel::unbounded();

    // Check whether an existing instance is running, and bail if so
    if handle_active_instance() {
        return Ok(());
    }

    // Spawn up the IPC handler
    let (ipc_tx, ipc_rx) = channel::unbounded();
    let ipc_main_tx = main_tx.clone();
    let ipc = thread::spawn(|| handle_ipc(ipc_rx, ipc_main_tx));

    // Ok, spawn up the Tray Handler
    let (tray_tx, tray_rx) = channel::unbounded();
    let tray_main_tx = main_tx.clone();
    let tray = thread::spawn(|| handle_tray(tray_rx, tray_main_tx));

    // Ok, we need to spawn up the device manager, first lets create some channels
    // The first channel is for us to be able to tell the manager to shut down, or reconfigure
    let (manage_tx, manage_rx) = channel::unbounded();

    // This one sends and receives messages when devices are attached and removed
    let (device_tx, device_rx) = channel::unbounded();
    let dev_main_tx = main_tx.clone();
    let device_manager = thread::spawn(|| spawn_device_manager(manage_rx, dev_main_tx, device_tx));

    // Under KDE at least, it expects the window class to be both the TLD and the name in order
    // to look for the icon in the right place.
    let resource_class = format!("{APP_TLD}.{APP_NAME}");

    let window_attributes = Window::default_attributes()
        .with_title(APP_TITLE)
        .with_window_icon(Some(load_icon(ICON)))
        .with_inner_size(LogicalSize::new(1024, 500))
        .with_name(resource_class, APP_NAME)
        .with_min_inner_size(LogicalSize::new(1024, 500));

    // Ok, spawn up the thread responsible for the UI
    let device_rx_inner = device_rx.clone();
    let window_main_tx = main_tx.clone();
    let window = thread::spawn(move || {
        let app: Box<dyn App> = Box::new(BeacnMicApp::new(device_rx_inner));

        // Create the event loop, an egui context, and the initial app state
        let mut event_loop = EventLoop::<UserEvent>::with_user_event()
            // This is a Linux tool, so we're safe to run the UI in a separate thread
            .with_any_thread(true)
            .build()
            .expect("Failed to create event loop");
        let runner = WindowRunner::new(app, window_main_tx, window_attributes.clone());
        runner.run(&mut event_loop, hide_initial).expect("UI Crash");
    });

    // Wait for a message to do stuff
    debug!("Running Message Handler...");
    let mut context = Context::default();
    loop {
        select! {
            recv(main_rx) -> msg => {
                match msg {
                    Ok(msg) => {
                        match msg {
                            ToMainMessages::UpdateContext(new_ctx) => {
                                debug!("Context Updated");
                                // Context Update
                                context = new_ctx;
                            }
                            ToMainMessages::SpawnWindow => {
                                // Window Re-Open requested
                                send_user_event(&context, UserEvent::FocusWindow);
                            }
                            ToMainMessages::RequestRedraw => {
                                // Repaint requested
                                send_user_event(&context, UserEvent::RequestRedraw);
                            }
                            ToMainMessages::Quit => {
                                // Break out and Close
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Main Loop Broken, bailing: {e}");
                        break;
                    }
                }
            }
            recv(device_rx) -> msg => {
                match msg {
                    Ok(msg) => {
                        // Pump this to the UI
                        send_user_event(&context, UserEvent::DeviceMessage(msg))
                    }
                    Err(e) => {
                        error!("Device Handler Broken, bailing: {e}");
                        break;
                    }
                }
            }
        }
    }

    debug!("Waiting for Threads to Terminate..");
    send_user_event(&context, UserEvent::Quit);
    let _ = manage_tx.send(ManagerMessages::Quit);
    let _ = ipc_tx.send(ManagerMessages::Quit);
    let _ = tray_tx.send(ManagerMessages::Quit);

    let _ = window.join();
    let _ = tray.join();
    let _ = device_manager.join();
    let _ = ipc.join();

    Ok(())
}

fn prepare_context(ctx: &mut Context) {
    let auto_start_key = Id::new(AUTO_START_KEY);

    let auto_start = match has_autostart() {
        Ok(present) => {
            debug!("File State: {present}");
            Some(present)
        }
        Err(e) => {
            debug!("Error Getting State: {e}");
            None
        }
    };
    debug!("Setting Value: {auto_start:?}");

    ctx.memory_mut(|mem| {
        mem.data.insert_temp(auto_start_key, auto_start);
    })
}

fn load_icon(bytes: &[u8]) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}

fn has_autostart() -> Result<bool> {
    let autostart_file = get_autostart_file()?;

    debug!("Checking: {autostart_file:?}");
    Ok(autostart_file.exists())
}

pub fn get_autostart_file() -> Result<PathBuf> {
    let config_dir = if let Ok(config) = env::var("XDG_CONFIG_HOME") {
        config
    } else if let Ok(home) = env::var("HOME") {
        format!("{home}/.config")
    } else {
        bail!("Unable to obtain XDG Config Directory")
    };
    Ok(PathBuf::from(format!(
        "{config_dir}/autostart/{APP_TLD}.desktop"
    )))
}

// This enum is passed into various 'Helper' threads and settings (such as the
// tray handler, device manager, socket listener) to allow them to keep track and
// trigger events on the UI
pub enum ManagerMessages {
    Quit,
}

pub enum ToMainMessages {
    SpawnWindow,
    RequestRedraw,
    UpdateContext(Context),
    Quit,
}
