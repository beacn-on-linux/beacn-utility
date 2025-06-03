use anyhow::bail;
use egui_winit::winit::dpi::LogicalSize;
use egui_winit::winit::event_loop::EventLoop;
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use crate::app::BeacnMicApp;
use crate::window_handle::WindowRunner;

mod audio_pages;
mod controller_pages;
mod numbers;
mod states;
mod widgets;
mod window_handle;
mod app;

fn main() -> anyhow::Result<()> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])?;

    let mut event_loop = EventLoop::new()?;

    loop {
        let initial_size = LogicalSize::new(1024, 500);
        let minimum_size = LogicalSize::new(1024, 500);
        let title = String::from("Beacn App");

        let context = WindowRunner::get_egui_context();
        let app = Box::new(BeacnMicApp::new(&context));
        let runner = WindowRunner::new(app, context.clone(), initial_size, minimum_size, title);
        if let Err(e) = runner.run(&mut event_loop) {
            bail!("Error: {}", e);
        }
        break Ok(());
    }
}