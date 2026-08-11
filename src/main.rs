use crate::managers::ipc::{handle_active_instance, handle_ipc};
use crate::ui_egui::app::BeacnMicApp;
use crate::ui_egui::window_handle::{App, UserEvent, WindowRunner, send_user_event};
use anyhow::bail;
use anyhow::{Result, anyhow};
use beacn_lib::flume::{Receiver, unbounded};
use egui::{Context, Id};
use egui_winit::winit::dpi::LogicalSize;
use egui_winit::winit::event_loop::EventLoop;

#[cfg(windows)]
use egui_winit::winit::platform::windows::EventLoopBuilderExtWindows;

#[cfg(unix)]
use egui_winit::winit::platform::x11::{EventLoopBuilderExtX11, WindowAttributesExtX11};

use directories::BaseDirs;
use egui_winit::winit::window::{Icon, Window};
use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use iced::font::{Family, Weight};
use iced::window::settings::PlatformSpecific;
use iced::{Font, Size, Task, window};
use log::{LevelFilter, debug, error, info, warn};
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger,
};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use std::{env, fs, thread};
use tokio::runtime::{Handle, Runtime};

use crate::devices::manager::{DeviceMessage, spawn_device_manager};
use crate::ui_iced::app::{BeacnUtility, Flags, Message};
use crate::ui_iced::runtime::SharedTokioExecutor;
use crate::ui_iced::widgets::theme::build_beacn_theme;
use tokio::{join, task};

pub mod devices;
mod integrations;
mod managers;
mod ui_egui;
pub mod ui_iced;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const HASH: &str = env!("GIT_HASH");

const BACKGROUND_PARAM: &str = "--background";
const LEGACY_BACKGROUND_PARAM: &str = "--startup";

const APP_TLD: &str = "io.github.beacn_on_linux";
const APP_NAME: &str = "beacn-utility";
const APP_TITLE: &str = "Beacn Utility";
const AUTO_START_KEY: &str = "autostart";
const ICON: &[u8] = include_bytes!("../resources/icons/beacn-utility-large.png");

static TOKIO_RUNTIME: OnceLock<Handle> = OnceLock::new();

pub fn runtime() -> &'static Handle {
    TOKIO_RUNTIME.get_or_init(Handle::current)
}
pub fn run_async_blocking<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

fn main() -> Result<()> {
    let tokio_rt = Runtime::new().expect("Failed to create Tokio Runtime");
    let _guard = tokio_rt.enter();

    // Configure the static runtime as this runtime
    runtime();

    println!("Initialising Logging...");
    let mut log_targets: Vec<Box<dyn SharedLogger>> = vec![];

    let mut config = ConfigBuilder::new();
    // The tracing package, when used, will output to INFO from zbus every second..
    config.add_filter_ignore_str("tracing");
    config.add_filter_ignore_str("winit::event_loop");
    config.add_filter_ignore_str("winit::window");
    config.add_filter_ignore_str("zbus");
    config.add_filter_ignore_str("nusb::platform::linux_usbfs");
    config.add_filter_ignore_str("nusb::platform::windows_winusb");
    config.add_filter_ignore_str("naga");
    config.add_filter_ignore_str("iced_wgpu");
    config.add_filter_ignore_str("iced_winit");
    config.add_filter_ignore_str("wgpu_hal");
    config.add_filter_ignore_str("wgpu_core");
    config.add_filter_ignore_str("cosmic_text");
    config.add_filter_ignore_str("sctk");

    // Setup Console Logging
    log_targets.push(TermLogger::new(
        LevelFilter::Debug,
        config.build(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    ));

    // Try to establish a log file in the XDG data directory
    match get_logs_path() {
        Ok(path) => {
            let log_file = path.join("beacn-utility.log");
            println!("Logging to file: {log_file:?}");

            let file_rotate = FileRotate::new(
                log_file,
                AppendCount::new(5),
                ContentLimit::Bytes(1024 * 1024 * 2),
                Compression::OnRotate(1),
                None,
            );
            log_targets.push(WriteLogger::new(
                LevelFilter::Debug,
                config.build(),
                file_rotate,
            ));
        }

        Err(e) => warn!("Log file directory creation failed, File Logging Disabled: {e}"),
    }

    CombinedLogger::init(log_targets)?;

    info!("Starting {} v{} - {}", APP_NAME, VERSION, HASH);

    // Install a PANIC logger, to hopefully drop info if something breaks
    log_panics::init();

    let args: Vec<String> = env::args().collect();
    let hide_initial = args.contains(&BACKGROUND_PARAM.to_string())
        || args.contains(&LEGACY_BACKGROUND_PARAM.to_string());

    // Firstly, create a message bus which allows threads to message back to here
    let (main_tx, main_rx) = unbounded();

    // Check whether an existing instance is running, and bail if so
    if tokio_rt.block_on(handle_active_instance()) {
        return Ok(());
    }

    // Spawn up the IPC handler
    let (ipc_tx, ipc_rx) = unbounded();
    let ipc_main_tx = main_tx.clone();
    let ipc = task::spawn(handle_ipc(ipc_rx, ipc_main_tx));

    // Ok, spawn up the Tray Handler
    #[cfg_attr(not(unix), allow(unused))]
    let (tray_tx, tray_rx) = unbounded();

    #[cfg_attr(not(unix), allow(unused))]
    let tray_main_tx = main_tx.clone();
    let tray = task::spawn(async move {
        #[cfg(unix)]
        {
            use managers::tray::handle_tray;
            if let Err(e) = handle_tray(tray_rx, tray_main_tx).await {
                error!("Failed to Spawn Tray: {e}");
            }
        }
    });

    // Ok, we need to spawn up the device manager, first lets create some channels
    // The first channel is for us to be able to tell the manager to shut down, or reconfigure
    let (manage_tx, manage_rx) = unbounded();

    // This one sends and receives messages when devices are attached and removed
    let (device_tx, device_rx) = unbounded();
    let dev_main_tx = main_tx.clone();
    let device_manager = task::spawn(spawn_device_manager(manage_rx, dev_main_tx, device_tx));

    // Under KDE at least, it expects the window class to be both the TLD and the name in order
    // to look for the icon in the right place.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut window_attributes = Window::default_attributes()
        .with_title(APP_TITLE)
        .with_window_icon(Some(load_icon(ICON)))
        .with_inner_size(LogicalSize::new(1024, 500))
        .with_min_inner_size(LogicalSize::new(1024, 500));

    #[cfg(unix)]
    {
        let resource_class = format!("{APP_TLD}.{APP_NAME}");
        window_attributes = window_attributes.with_name(resource_class, APP_NAME);
    }

    // Ok, spawn up the thread responsible for the UI
    let device_rx_inner = device_rx.clone();
    let window_main_tx = main_tx.clone();

    // Wait for a message to do stuff
    debug!("Running Message Handler...");
    let (window_tx, window_rx) = unbounded();

    // Honestly, we might not need this loop, the UI can read and manage its own channels
    task::spawn(async move {
        loop {
            tokio::select! {
                msg = main_rx.recv_async() => {
                    match msg {
                        Ok(ToMainMessages::SpawnWindow) => {
                            let _ = window_tx.send_async(WindowMessage::OpenWindow).await;
                        }

                        Ok(ToMainMessages::Quit) => {
                            let _ = window_tx.send_async(WindowMessage::Quit).await;
                            break
                        },

                        Err(e) => {
                            error!("Main Loop Broken, bailing: {e}");
                            break;
                        }
                    }
                }

                _ = shutdown_signal() => {
                    let _ = window_tx.send_async(WindowMessage::Quit).await;
                    break;
                }
            }
        }
    });

    spawn_iced_window(device_rx, window_rx)?;

    debug!("Shutdown Triggered - Waiting for Threads to Terminate..");
    let _ = manage_tx.send(ManagerMessages::Quit);
    let _ = ipc_tx.send(ManagerMessages::Quit);
    let _ = tray_tx.send(ManagerMessages::Quit);

    // Join on the remaining tasks
    let _ = tokio_rt.block_on(async { join!(ipc, tray, device_manager) });

    debug!("Shutdown Complete");

    Ok(())
}

fn spawn_iced_window(
    device_rx: Receiver<DeviceMessage>,
    window_rx: Receiver<WindowMessage>,
) -> Result<()> {
    const BOLD_FONT: &[u8] = include_bytes!("../resources/fonts/noto/NotoSans-Bold.ttf");
    const REGULAR_FONT: &[u8] = include_bytes!("../resources/fonts/noto/NotoSans-Regular.ttf");
    const SEMI_BOLD_FONT: &[u8] = include_bytes!("../resources/fonts/noto/NotoSans-SemiBold.ttf");

    let settings = iced::Settings {
        default_font: Font {
            family: Family::Name("Noto Sans"),
            weight: Weight::Semibold,
            ..Default::default()
        },
        fonts: vec![REGULAR_FONT.into(), SEMI_BOLD_FONT.into(), BOLD_FONT.into()],
        default_text_size: 12.0.into(),
        ..Default::default()
    };

    // Initial Window Settings and size
    let mut initial_window_settings = window::Settings {
        exit_on_close_request: false, // Intercepts the X button manually
        size: Size::new(1024., 500.),
        min_size: Some(Size::new(1024., 500.)),
        ..Default::default()
    };

    #[cfg(target_os = "linux")]
    {
        initial_window_settings.platform_specific = PlatformSpecific {
            application_id: "io.github.beacn_on_linux.beacn-utility".into(),
            ..Default::default()
        };
    }

    iced::daemon(
        move || {
            let device_rx = device_rx.clone();
            let window_rx = window_rx.clone();

            let (mut app_state, boot) = BeacnUtility::new(Flags {
                window_settings: initial_window_settings.clone(),
                device_rx,
                window_rx,
            });

            let (initial_id, open_task) = window::open(initial_window_settings.clone());
            app_state.active_id = Some(initial_id);

            let mapped_open_task = open_task.map(move |_| Message::WindowOpened(initial_id));
            let combined_task = Task::batch(vec![boot, mapped_open_task]);

            // Track your initial_id in your state here if needed, then yield:
            (app_state, combined_task)
        },
        BeacnUtility::update,
        BeacnUtility::view,
    )
    .title(BeacnUtility::title)
    .subscription(BeacnUtility::subscription)
    .theme(|_state: &BeacnUtility, _window_id: window::Id| build_beacn_theme())
    .settings(settings)
    .executor::<SharedTokioExecutor>()
    .run()
    .map_err(anyhow::Error::from)
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

pub fn get_logs_path() -> Result<PathBuf> {
    let log_path = get_data_path()?.join("logs");
    fs::create_dir_all(&log_path)?;

    Ok(log_path)
}

pub fn get_data_path() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or(anyhow!("Failed to find Base Directories"))?;
    let data = base.data_dir().join(APP_NAME);

    // This is a migration to move from ~/.local/share/io.github.beacn_on_linux to
    // ~/.local/share/beacn-utility - This is to match the cache and config behaviours.
    let old_data_path = base.data_dir().join(APP_TLD);
    if old_data_path.exists() && !data.exists() {
        println!("Migrating Log Directory from {old_data_path:?} to {data:?}");
        fs::rename(&old_data_path, &data)?;
    } else if old_data_path.exists() && data.exists() {
        fs::remove_dir_all(&old_data_path)?;
    }

    match fs::create_dir_all(&data) {
        Ok(()) => Ok(data),
        Err(e) => {
            bail!("Failed to create config directory: {e}");
        }
    }
}

pub fn get_config_path() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or(anyhow!("Failed to find Base Directories"))?;
    let config = base.config_dir().join(APP_NAME);

    match fs::create_dir_all(&config) {
        Ok(()) => Ok(config),
        Err(e) => {
            bail!("Failed to create config directory: {e}");
        }
    }
}

pub fn get_cache_path() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or(anyhow!("Failed to find Base Directories"))?;
    let cache = base.cache_dir().join(APP_NAME);

    match fs::create_dir_all(&cache) {
        Ok(()) => Ok(cache),
        Err(e) => {
            bail!("Failed to create config directory: {e}");
        }
    }
}

pub fn get_autostart_file() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or(anyhow!("Failed to find Base Directories"))?;
    let config_dir = base.config_dir();

    // This is how flatpaks will create the file, so we need to match it
    let autostart_file = format!("{APP_TLD}.{APP_NAME}.desktop");
    let path = config_dir.join("autostart").join(autostart_file);

    let legacy_path = config_dir
        .join("autostart")
        .join(format!("{APP_TLD}.desktop"));
    if legacy_path.exists() {
        if !path.exists() {
            debug!("Migrating Legacy Autostart File from {legacy_path:?} to {path:?}");
            fs::rename(&legacy_path, &path)?;
        } else {
            debug!("Removing Legacy Autostart File at {legacy_path:?} as new file exists",);
            fs::remove_file(&legacy_path)?;
        }
    }

    Ok(path)
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = signal(SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = sigint.recv() => println!("Caught Ctrl+C"),
        _ = sigterm.recv() => println!("Caught SIGTERM"),
    }
}

#[cfg(windows)]
async fn shutdown_signal() {
    use tokio::signal::windows;

    let mut ctrl_c = windows::ctrl_c().unwrap();
    let mut ctrl_break = windows::ctrl_break().unwrap();
    let mut ctrl_close = windows::ctrl_close().unwrap();
    let mut ctrl_logoff = windows::ctrl_logoff().unwrap();
    let mut ctrl_shutdown = windows::ctrl_shutdown().unwrap();

    tokio::select! {
        _ = ctrl_c.recv() => println!("Caught Ctrl+C"),
        _ = ctrl_break.recv() => println!("Caught Ctrl+Break"),
        _ = ctrl_close.recv() => println!("Console closing"),
        _ = ctrl_logoff.recv() => println!("User logging off"),
        _ = ctrl_shutdown.recv() => println!("System shutting down"),
    }
}

// This enum is passed into various 'Helper' threads and settings (such as the
// tray handler, device manager, socket listener) to allow them to keep track and
// trigger events on the UI
pub enum ManagerMessages {
    Quit,
}

// This is a dupe of ToMainMessages for now, until I know whether main()
// needs to maintain anything special, or can just wait for the window to close.
pub enum WindowMessage {
    OpenWindow,
    Quit,
}

pub enum ToMainMessages {
    SpawnWindow,
    // RequestRedraw,
    // UpdateContext(Context),
    Quit,
}
