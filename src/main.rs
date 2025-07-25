use crate::device_manager::spawn_device_manager;
use crate::managers::ipc::{handle_active_instance, handle_ipc};
use crate::ui::app::BeacnMicApp;
use crate::window_handle::{App, UserEvent, WindowRunner};
use anyhow::Result;
use anyhow::bail;
use beacn_lib::crossbeam::{channel, select};
use egui::{Context, Id};
use egui_winit::winit::dpi::LogicalSize;
use egui_winit::winit::event_loop::EventLoop;
use egui_winit::winit::platform::x11::WindowAttributesExtX11;
use egui_winit::winit::window::{Icon, Window};
use log::{LevelFilter, debug, error};
use managers::tray::handle_tray;
use once_cell::sync::Lazy;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use std::path::PathBuf;
use std::{env, thread};
use tokio::runtime::{Builder, Runtime};

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

// We need a minimum tokio runtime, so we can use libs that utilise async inside our sync code
static TOKIO: Lazy<Runtime> = Lazy::new(|| {
    debug!("Spawning tokio runtime..");
    Builder::new_current_thread()
        .enable_io()
        .build()
        .expect("Failed to Create tokio Runtime")
});
pub fn run_async<F: Future>(future: F) -> F::Output {
    TOKIO.block_on(future)
}

fn main() -> Result<()> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])?;

    let args: Vec<String> = env::args().collect();
    let hide_initial = args.contains(&"--startup".to_string());
    let mut first_run = true;

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
    let device_manager = thread::spawn(|| spawn_device_manager(manage_rx, device_tx));

    // Create the event loop, an egui context, and the initial app state
    let mut event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let mut app: Box<dyn App> = Box::new(BeacnMicApp::new(device_rx.clone()));

    // Under KDE at least, it expects the window class to be both the TLD and the name in order
    // to look for the icon in the right place.
    let resource_class = format!("{APP_TLD}.{APP_NAME}");

    let mut window_attributes = Window::default_attributes()
        .with_title(APP_TITLE)
        .with_window_icon(Some(load_icon(ICON)))
        .with_inner_size(LogicalSize::new(1024, 500))
        .with_name(resource_class, APP_NAME)
        .with_min_inner_size(LogicalSize::new(1024, 500));

    'mainloop: loop {
        if !hide_initial || !first_run {
            // Spawn up a new egui context
            let mut context = Context::default();
            prepare_context(&mut context);

            // app is a Box<dyn App>, we need to downcast it back to a Box<BeacnMicApp>
            if let Some(app) = app.as_mut().as_any().downcast_mut::<BeacnMicApp>() {
                // Attach the new context to the app
                app.with_context(&context);
            }

            // Send the new context off to our threads, this needs to be done because this thread
            // will be locked into the Window event loop, so if an update or behaviour change needs
            // to be triggered, the threads will have to call it themselves.
            let _ = manage_tx.send(ManagerMessages::SetContext(Some(context.clone())));
            let _ = ipc_tx.send(ManagerMessages::SetContext(Some(context.clone())));
            let _ = tray_tx.send(ManagerMessages::SetContext(Some(context.clone())));

            // Create a window runner
            let runner = WindowRunner::new(app, context, window_attributes.clone());

            // Run the event loop, this will block until the Window is closed
            match runner.run(&mut event_loop) {
                // The window runner will return the app (and window attributes) to us, so we can
                // store them, and use them the next time the window needs to be open.
                Ok((a, w)) => {
                    app = a;
                    window_attributes = w
                }
                Err(e) => bail!("Error: {}", e),
            }
        }
        first_run = false;

        // Clear the Context from our threads
        let _ = manage_tx.send(ManagerMessages::SetContext(None));
        let _ = ipc_tx.send(ManagerMessages::SetContext(None));
        let _ = tray_tx.send(ManagerMessages::SetContext(None));

        // Wait for a message to do stuff
        debug!("Window Closed, awaiting new Messages");
        loop {
            select! {
                recv(main_rx) -> msg => {
                    match msg {
                        Ok(msg) => {
                            match msg {
                                ToMainMessages::SpawnWindow => {
                                    // Window Re-Open requested
                                    continue 'mainloop;
                                }
                                ToMainMessages::Quit => {
                                    // Break out and Close
                                    break 'mainloop;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Main Loop Broken, bailing: {e}");
                            break 'mainloop;
                        }
                    }
                }
                recv(device_rx) -> msg => {
                    match msg {
                        Ok(msg) => {
                            // A device message has come in, while we don't have an active window
                            // we can still pass this into the App to update its state
                            if let Some(app) = app.as_mut().as_any().downcast_mut::<BeacnMicApp>() {
                                app.handle_device_message(msg);
                            }
                        }
                        Err(e) => {
                            error!("Device Handler Broken, bailing: {e}");
                            break 'mainloop;
                        }
                    }
                }
            }
        }
    }

    debug!("Waiting for Threads to Terminate..");
    let _ = manage_tx.send(ManagerMessages::Quit);
    let _ = ipc_tx.send(ManagerMessages::Quit);
    let _ = tray_tx.send(ManagerMessages::Quit);

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
    SetContext(Option<Context>),
    Quit,
}

pub enum ToMainMessages {
    SpawnWindow,
    Quit,
}
