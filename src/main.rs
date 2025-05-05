use crate::pages::about::About;
use crate::pages::config::Configuration;
use crate::pages::lighting::Lighting;
use crate::state::BeacnMicState;
use anyhow::{Result, anyhow};
use beacn_mic_lib::device::BeacnMic;
use beacn_mic_lib::messages::Message;
use eframe::Frame;
use egui::{Color32, Context, Response};
use log::{LevelFilter, debug};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use std::cell::RefCell;
use std::rc::Rc;

mod numbers;
mod pages;
mod state;
mod widgets;

#[derive(PartialEq)]
enum Page {
    Configuration,
    Lighting,
    About,
}

pub struct BeacnMicApp {
    active_page: Page,

    // Page early instantiations
    configuration_page: Configuration,
    lighting_page: Lighting,
    about_page: About,
}

impl BeacnMicApp {
    pub fn new(mic: BeacnMic, state: BeacnMicState) -> Self {
        let state = Rc::new(RefCell::new(state));
        let mic = Rc::new(mic);

        let configuration_page = Configuration::new(mic.clone(), state.clone());
        let lighting_page = Lighting::new(mic.clone(), state.clone());
        let about_page = About::new(mic.clone(), state.clone());

        Self {
            active_page: Page::Configuration,

            configuration_page,
            lighting_page,
            about_page,
        }
    }
}

impl eframe::App for BeacnMicApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::SidePanel::left("left_panel")
            .resizable(false)
            .default_width(80.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if round_nav_button(ui, "🔵", self.active_page == Page::Configuration).clicked()
                    {
                        self.active_page = Page::Configuration;
                    }
                    if round_nav_button(ui, "🟢", self.active_page == Page::Lighting).clicked() {
                        self.active_page = Page::Lighting;
                    }
                    if round_nav_button(ui, "🔴", self.active_page == Page::About).clicked() {
                        self.active_page = Page::About;
                    }
                })
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            match &mut self.active_page {
                Page::Configuration => self.configuration_page.ui(ui),
                Page::Lighting => self.lighting_page.ui(ui),
                Page::About => self.about_page.ui(ui),
            };
        });
    }
}

fn main() -> Result<()> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])?;

    // Before we do anything, open a connection to the mic, and attempt to obtain a state.

    let mic = BeacnMic::open()?;
    let state = BeacnMicState::load_settings(&mic)?;
    debug!("{:#?}", state);

    Message::generate_fetch_message();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100., 520.]),
        ..Default::default()
    };

    eframe::run_native(
        "Beacn Mic Configuration",
        options,
        Box::new(|cc| Ok(Box::new(BeacnMicApp::new(mic, state)))),
    )
    .map_err(|e| anyhow!("Failed: {}", e))?;

    Ok(())
}

fn round_nav_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    let (fill_color, text_color) = if active {
        (Color32::LIGHT_BLUE, Color32::BLACK)
    } else {
        (Color32::DARK_GRAY, Color32::WHITE)
    };

    ui.scope(|ui| {
        ui.style_mut().visuals.override_text_color = Some(text_color);

        ui.add(
            egui::Button::new(label)
                .min_size(egui::vec2(40.0, 40.0))
                .fill(fill_color)
                .corner_radius(20.0),
        )
    })
    .inner
}
