use std::thread;
use crate::ui::app::BeacnMicApp;
use crate::window_handle::{App, UserEvent, WindowRunner};
use anyhow::bail;
use egui_winit::winit::dpi::LogicalSize;
use egui_winit::winit::event_loop::EventLoop;
use egui_winit::winit::window::{Window, WindowAttributes};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use beacn_lib::crossbeam::channel;
use crate::device_manager::{spawn_device_manager, ManagerMessages};

mod numbers;
mod widgets;
mod window_handle;
mod device_manager;
mod ui;

fn main() -> anyhow::Result<()> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])?;

    // Ok, we need to spawn up the device manager, first lets create some channels
    // The first channel is for us to be able to tell the manager to shut down, or reconfigure
    let (manage_tx, manage_rx) = channel::unbounded();

    // This one sends and receives messages when devices are attached and removed
    let (device_tx, device_rx) = channel::unbounded();

    thread::spawn(|| spawn_device_manager(manage_rx, device_tx));

    // Create the event loop, an egui context, and the initial app state
    let mut event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let mut app: Box<dyn App> = Box::new(BeacnMicApp::new(device_rx));

    let mut window_attributes = Window::default_attributes()
        .with_title("Beacn App")
        .with_inner_size(LogicalSize::new(1024, 500))
        .with_min_inner_size(LogicalSize::new(1024, 500));

    loop {
        // Spawn up a new egui context
        let context = egui::Context::default();

        // app is a Box<dyn App>, we need to downcast it back to a Box<BeacnMicApp>
        if let Some(app) = app.as_mut().as_any().downcast_mut::<BeacnMicApp>() {
            // Attach the new context to the app
            app.with_context(&context);
        }
        let _ = manage_tx.send(ManagerMessages::SetContext(Some(context.clone())));

        // Create a window runner
        let runner = WindowRunner::new(app, context, window_attributes.clone());

        // Run the event loop, this will return when the window is closed.
        match runner.run(&mut event_loop) {
            // The window runner will return the app (and window attributes) to us, so we can
            // store them, and use them the next time the window needs to be open.
            Ok((a, w)) => {
                app = a;
                window_attributes = w
            }
            Err(e) => bail!("Error: {}", e),
        }
        //sleep(Duration::from_secs(5));
        break;
    }

    let _ = manage_tx.send(ManagerMessages::Quit);

    Ok(())
}
