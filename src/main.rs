use crate::managers::ipc::{handle_active_instance, handle_ipc};
use anyhow::bail;
use anyhow::{Result, anyhow};
use beacn_lib::flume::{Receiver, unbounded};

use directories::BaseDirs;
use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use iced::font::{Family, Weight};
use iced::{Font, Size, window};
use log::{LevelFilter, debug, info, warn};
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger,
};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::{env, fs};
use tokio::runtime::{Handle, Runtime};

use crate::devices::manager::{DeviceMessage, spawn_device_manager};
use crate::ui::app::{BeacnUtility, Flags};
use crate::ui::runtime::SharedTokioExecutor;
use crate::ui::widgets::theme::build_beacn_theme;
use tokio::{join, task};

pub mod devices;
mod integrations;
mod managers;
mod ui;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const HASH: &str = env!("GIT_HASH");

const BACKGROUND_PARAM: &str = "--background";
const LEGACY_BACKGROUND_PARAM: &str = "--startup";

const APP_TLD: &str = "io.github.beacn_on_linux";
const APP_NAME: &str = "beacn-utility";
const APP_TITLE: &str = "Beacn Utility";
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
    let (window_tx, window_rx) = unbounded();
    if !hide_initial {
        window_tx.send(WindowMessage::OpenWindow)?;
    }

    // Check whether an existing instance is running, and bail if so
    if tokio_rt.block_on(handle_active_instance()) {
        return Ok(());
    }

    // Spawn up the IPC handler
    let (ipc_tx, ipc_rx) = unbounded();
    let ipc_window_tx = window_tx.clone();
    let ipc = task::spawn(handle_ipc(ipc_rx, ipc_window_tx));

    // Ok, spawn up the Tray Handler
    #[cfg_attr(not(unix), allow(unused))]
    let (tray_tx, tray_rx) = unbounded();

    #[cfg_attr(not(unix), allow(unused))]
    let tray_window_tx = window_tx.clone();
    let tray = task::spawn(async move {
        #[cfg(unix)]
        {
            use log::error;
            use managers::tray::handle_tray;
            if let Err(e) = handle_tray(tray_rx, tray_window_tx).await {
                error!("Failed to Spawn Tray: {e}");
            }
        }
    });

    // Ok, we need to spawn up the device manager, first lets create some channels
    // The first channel is for us to be able to tell the manager to shut down, or reconfigure
    let (manage_tx, manage_rx) = unbounded();

    // This one sends and receives messages when devices are attached and removed
    let (device_tx, device_rx) = unbounded();
    let device_manager = task::spawn(spawn_device_manager(manage_rx, device_tx));

    // Wait for a message to do stuff
    debug!("Running Message Handler...");

    // Honestly, we might not need this loop, the UI can read and manage its own channels
    let (signal_tx, signal_rx) = unbounded();
    let signal = task::spawn(async move {
        loop {
            tokio::select! {
                Ok(ManagerMessages::Quit) = signal_rx.recv_async() => {
                    break;
                }

                _ = shutdown_signal() => {
                    let _ = window_tx.send_async(WindowMessage::Quit).await;
                }
            }
        }
    });

    spawn_iced_window(device_rx, window_rx)?;

    debug!("Shutdown Triggered - Waiting for Threads to Terminate..");
    let _ = signal_tx.send(ManagerMessages::Quit);
    let _ = manage_tx.send(ManagerMessages::Quit);
    let _ = ipc_tx.send(ManagerMessages::Quit);
    let _ = tray_tx.send(ManagerMessages::Quit);

    // Join on the remaining tasks
    let _ = tokio_rt.block_on(async { join!(signal, ipc, tray, device_manager) });

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
    #[allow(unused_mut)]
    let mut window_settings = window::Settings {
        exit_on_close_request: false,
        icon: Some(load_icon_iced(ICON)),
        size: Size::new(1124., 500.),
        min_size: Some(Size::new(1124., 500.)),
        ..Default::default()
    };

    // Allows wayland compositors to set the window icon
    #[cfg(target_os = "linux")]
    {
        let application_id = format!("{APP_TLD}.{APP_NAME}");
        use iced::window::settings::PlatformSpecific;
        window_settings.platform_specific = PlatformSpecific {
            application_id,
            ..Default::default()
        };
    }

    iced::daemon(
        move || {
            let device_rx = device_rx.clone();
            let window_rx = window_rx.clone();

            BeacnUtility::new(Flags {
                window_settings: window_settings.clone(),
                device_rx,
                window_rx,
            })
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

fn load_icon_iced(bytes: &[u8]) -> window::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    window::icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
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

#[cfg(target_os = "linux")]
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

#[cfg(not(target_os = "linux"))]
pub fn get_autostart_file() -> Result<PathBuf> {
    bail!("Autostart is not supported on this platform");
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
