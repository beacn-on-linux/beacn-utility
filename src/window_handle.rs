use crate::{APP_NAME, AUTO_START_KEY, get_autostart_file, run_async};
use anyhow::{Result, anyhow};
use ashpd::WindowIdentifier;
use ashpd::desktop::background::Background;
use egui::Id;
use egui_glow::glow;
use egui_glow::glow::HasContext;
use egui_winit::winit;
use egui_winit::winit::event_loop::EventLoopProxy;
use egui_winit::winit::platform::run_on_demand::EventLoopExtRunOnDemand;
#[allow(deprecated)]
use egui_winit::winit::raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use egui_winit::winit::window::{UserAttentionType, WindowAttributes};
use egui_winit::winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use glutin::prelude::GlSurface;
use ini::Ini;
use log::debug;
use std::any::Any;
use std::sync::Arc;
use std::time::Instant;
use std::{env, fs};

const EVENT_PROXY: &str = "event_proxy";

// These are events we can send into winit to trigger an update
#[derive(Debug, Clone)]
pub enum UserEvent {
    RequestRedraw,
    CloseWindow,
    FocusWindow,
    SetAutoStart(bool),
}

// This is a reference to the Event Proxy, which we can store inside the context
#[derive(Clone)]
struct EventProxy(Arc<EventLoopProxy<UserEvent>>);

pub trait App {
    fn update(&mut self, ctx: &egui::Context);
    fn should_close(&mut self) -> bool;
    fn on_close(&mut self);
    fn as_any(&mut self) -> &mut dyn Any;
}

pub struct WindowRunner {
    app: Box<dyn App>,
    window: Option<Arc<Window>>,
    renderer: Option<GlowRenderer>,
    app_start_time: Instant,
    context: egui::Context,
    event_loop_proxy: Option<EventLoopProxy<UserEvent>>,

    window_attributes: WindowAttributes,
}

struct GlowRenderer {
    gl_context: glutin::context::PossiblyCurrentContext,
    gl_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    winit_state: egui_winit::State,
    painter: egui_glow::Painter,
    gl: Arc<glow::Context>,
}

impl WindowRunner {
    pub fn new(app: Box<dyn App>, context: egui::Context, attributes: WindowAttributes) -> Self {
        Self {
            app,
            window: None,
            renderer: None,
            app_start_time: Instant::now(),

            context,
            event_loop_proxy: None,

            window_attributes: attributes,
        }
    }

    pub fn run(
        mut self,
        event_loop: &mut EventLoop<UserEvent>,
    ) -> Result<(Box<dyn App>, WindowAttributes)> {
        event_loop.set_control_flow(ControlFlow::Wait);

        // Create the event loop proxy
        self.event_loop_proxy = Some(event_loop.create_proxy());

        if let Some(proxy) = &self.event_loop_proxy {
            self.context.data_mut(|data| {
                data.insert_persisted(
                    Id::new(EVENT_PROXY),
                    EventProxy(Arc::new(proxy.clone())),
                );
            });
        }

        // Set a wakeup for a redraw
        let proxy = self.event_loop_proxy.as_ref().unwrap().clone();
        self.context.set_request_repaint_callback(move |_info| {
            let _ = proxy.send_event(UserEvent::RequestRedraw);
        });

        // Use run_app_on_demand instead of run() so it can return when window closes
        event_loop.run_app_on_demand(&mut self)?;

        Ok((self.app, self.window_attributes))
    }

    fn render_frame(&mut self) {
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let mut raw_input = renderer.winit_state.take_egui_input(window);
            raw_input.time = Some(self.app_start_time.elapsed().as_secs_f64());

            let full_output = self.context.run(raw_input, |ctx| {
                self.app.update(ctx);
            });

            renderer
                .winit_state
                .handle_platform_output(window, full_output.platform_output.clone());

            renderer.render_egui(&full_output, &self.context);

            // Swap buffers
            renderer
                .gl_surface
                .swap_buffers(&renderer.gl_context)
                .unwrap();
        }
    }
}

// This is a helper function which lets the app send a UserEvent into the context
pub fn send_user_event(ctx: &egui::Context, event: UserEvent) {
    ctx.data(|data| {
        if let Some(proxy) = data.get_temp::<EventProxy>(Id::new(EVENT_PROXY)) {
            let _ = proxy.0.send_event(event);
        }
    });
}

impl ApplicationHandler<UserEvent> for WindowRunner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attributes = self.window_attributes.clone();
            let window = Arc::new(event_loop.create_window(attributes).unwrap());
            let renderer = GlowRenderer::new(Arc::clone(&window), &self.context);

            self.window = Some(window);
            self.renderer = Some(renderer);
        }
    }

    // ASHPD expects the RawWindowHandle and RawWindowDisplay to present a permission
    // check to the user when adding an autostart entry.
    #[allow(deprecated)]
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::RequestRedraw => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            UserEvent::CloseWindow => {
                self.window = None;
                self.renderer = None;

                event_loop.exit();
            }
            UserEvent::FocusWindow => {
                if let Some(window) = &self.window {
                    if let Some(true) = window.is_minimized() {
                        window.set_minimized(false);
                    }

                    window.focus_window();
                    window.request_user_attention(Some(UserAttentionType::Informational));
                }
            }
            UserEvent::SetAutoStart(create) => {
                let key = Id::new(AUTO_START_KEY);
                if let Some(window) = &self.window {
                    if env::var("FLATPAK_SANDBOX_DIR").is_ok() {
                        println!("Running inside Flatpak, using Background Portal");
                        let window_handle = window.raw_window_handle().unwrap();
                        let display_handle = window.raw_display_handle().ok();

                        let reason = "Manage Beacn Devices on Startup";

                        run_async(async {
                            let identifier = WindowIdentifier::from_raw_handle(
                                &window_handle,
                                display_handle.as_ref(),
                            )
                            .await;

                            let request = Background::request()
                                .identifier(identifier)
                                .reason(reason)
                                .auto_start(create)
                                .dbus_activatable(false)
                                .command::<Vec<_>, String>(vec![
                                    String::from(APP_NAME),
                                    String::from("--startup"),
                                ]);

                            debug!("Requesting Background Access");

                            let result = match request.send().await.and_then(|r| r.response()) {
                                Ok(response) => {
                                    debug!("{response:?}");
                                    Some(response.auto_start())
                                }
                                Err(e) => {
                                    debug!("Failed to request autostart run: {e}");
                                    None
                                }
                            };
                            self.context.memory_mut(|mem| {
                                mem.data.insert_temp(key, result);
                            })
                        });
                    } else {
                        debug!("Running Outside Flatpak, manually handling");
                        // TODO: I have the XDG crate, I can locate this automatically

                        let attempt = match get_autostart_file() {
                            Ok(path) => {
                                if path.exists() && fs::remove_file(path.clone()).is_err() {
                                    Err(anyhow!("Unable to remove existing AutoStart"))
                                } else if create {
                                    if let Ok(exe) = env::current_exe() {
                                        let mut conf = Ini::new();
                                        let exe = exe.to_string_lossy().to_string();

                                        conf.with_section(Some("Desktop Entry"))
                                            .set("Type", "Application")
                                            .set("Name", "Beacn Utility")
                                            .set("Comment", "A Tool for Configuring Beacn Devices")
                                            .set("Exec", format!("{exe:?} --startup"))
                                            .set("Terminal", "false");

                                        match conf.write_to_file(path) {
                                            Ok(()) => Ok(()),
                                            Err(e) => Err(anyhow!("Failed to Write File, {}", e)),
                                        }
                                    } else {
                                        Err(anyhow!("Unable to Determine Executable"))
                                    }
                                } else {
                                    // Existing file was deleted, that's all that's needed
                                    Ok(())
                                }
                            }
                            Err(e) => Err(anyhow!(e)),
                        };

                        let result = match attempt {
                            Ok(()) => Some(create),
                            Err(e) => {
                                debug!("Failed to Handle AutoStart: {e}");
                                None
                            }
                        };
                        self.context.memory_mut(|mem| {
                            mem.data.insert_temp(key, result);
                        })
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let response = renderer.winit_state.on_window_event(window, &event);

            // Request redraw if egui wants it AND we're not already a RedrawRequested event
            if response.repaint && !matches!(&event, WindowEvent::RedrawRequested) {
                window.request_redraw();
            }

            match event {
                WindowEvent::RedrawRequested => {
                    self.render_frame();
                }
                WindowEvent::CloseRequested => {
                    if self.app.should_close() {
                        // Clear variables
                        self.window = None;
                        self.renderer = None;

                        // Exit the event loop when window closes so run() can return
                        event_loop.exit();
                    }
                }
                WindowEvent::Resized(physical_size) => {
                    self.window_attributes.inner_size = Some(physical_size.into());
                    renderer.resize(physical_size)
                }
                WindowEvent::Moved(position) => {
                    self.window_attributes.position = Some(position.into());
                }
                _ => {}
            }
        }
    }
}

impl GlowRenderer {
    #[allow(deprecated)]
    fn new(window: Arc<Window>, egui_ctx: &egui::Context) -> Self {
        use glutin::config::ConfigTemplateBuilder;
        use glutin::context::{ContextApi, ContextAttributesBuilder};
        use glutin::prelude::*;
        use glutin::surface::SurfaceAttributesBuilder;

        let raw_window_handle = window.raw_window_handle().unwrap();
        let raw_display_handle = window.raw_display_handle().unwrap();

        // Create OpenGL display
        let gl_display = unsafe {
            glutin::display::Display::new(
                raw_display_handle,
                glutin::display::DisplayApiPreference::Egl,
            )
            .unwrap()
        };

        // Create OpenGL config
        let config_template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(false)
            .build();

        let config = unsafe {
            gl_display
                .find_configs(config_template)
                .unwrap()
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .unwrap()
        };

        // Create OpenGL context, we won't specify an API version, glow will pick the best.
        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(None))
            .build(Some(raw_window_handle));

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&config, &context_attributes)
                .unwrap()
        };

        // Create OpenGL surface
        let size = window.inner_size();
        let surface_attributes = SurfaceAttributesBuilder::<glutin::surface::WindowSurface>::new()
            .build(
                raw_window_handle,
                size.width.try_into().unwrap(),
                size.height.try_into().unwrap(),
            );

        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&config, &surface_attributes)
                .unwrap()
        };

        // Make context current
        let gl_context = not_current_gl_context.make_current(&gl_surface).unwrap();

        // Create glow context
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                let s = std::ffi::CString::new(s)
                    .expect("failed to construct C string from string for gl proc address");

                gl_display.get_proc_address(&s)
            })
        });

        // Set up egui winit state
        let viewport_id = egui_ctx.viewport_id();
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            viewport_id,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        // Create egui glow painter
        let painter = egui_glow::Painter::new(Arc::clone(&gl), "", None, false)
            .expect("Failed to create egui_glow painter");

        Self {
            gl_context,
            gl_surface,
            winit_state: egui_winit,
            painter,
            gl,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.gl_surface.resize(
                &self.gl_context,
                new_size.width.try_into().unwrap(),
                new_size.height.try_into().unwrap(),
            );

            unsafe {
                self.gl
                    .viewport(0, 0, new_size.width as i32, new_size.height as i32);
            }
        }
    }

    fn render_egui(&mut self, full_output: &egui::FullOutput, egui_ctx: &egui::Context) {
        let clipped_primitives =
            egui_ctx.tessellate(full_output.shapes.clone(), full_output.pixels_per_point);

        let dimensions = [
            self.gl_surface.width().unwrap(),
            self.gl_surface.height().unwrap(),
        ];

        unsafe {
            self.gl.clear_color(0.1, 0.2, 0.3, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        self.painter.paint_and_update_textures(
            dimensions,
            full_output.pixels_per_point,
            &clipped_primitives,
            &full_output.textures_delta,
        );
    }
}

impl Drop for GlowRenderer {
    fn drop(&mut self) {
        self.painter.destroy();
    }
}
