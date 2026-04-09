use crate::device_manager::DeviceMessage;
use crate::{
    APP_NAME, AUTO_START_KEY, BACKGROUND_PARAM, ToMainMessages, get_autostart_file,
    prepare_context, run_async_blocking,
};
use anyhow::{Result, anyhow};
use ashpd::WindowIdentifier;
use ashpd::desktop::background::Background;
use beacn_lib::crossbeam::channel::Sender;
use egui::{Context, Id};
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
use glutin::display::DisplayApiPreference;
use glutin::prelude::GlSurface;
use ini::Ini;
use log::{debug, error, warn};
use std::sync::Arc;
use std::time::Instant;
use std::{env, fs};

const EVENT_PROXY: &str = "event_proxy";

#[derive(Debug, Clone)]
pub enum UserEvent {
    RequestRedraw,
    FocusWindow,
    #[allow(dead_code)]
    DeviceMessage(DeviceMessage),
    SetAutoStart(bool),
    Quit,
}

#[derive(Clone)]
struct EventProxy(Arc<EventLoopProxy<UserEvent>>);

pub trait App {
    fn with_context(&mut self, ctx: &Context);
    fn update(&mut self, ctx: &Context);
    fn should_close(&mut self) -> bool;
    fn on_close(&mut self);
    fn handle_device_message(&mut self, msg: DeviceMessage);
}

pub struct WindowRunner {
    app: Box<dyn App>,
    initial_hide: bool,
    window: Option<Arc<Window>>,
    renderer: Option<GlowRenderer>,
    app_start_time: Instant,
    context: Context,
    event_loop_proxy: Option<EventLoopProxy<UserEvent>>,
    window_attributes: WindowAttributes,
    sender: Sender<ToMainMessages>,
}

struct GlowRenderer {
    gl_context: glutin::context::PossiblyCurrentContext,
    gl_surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    winit_state: egui_winit::State,
    painter: egui_glow::Painter,
    gl: Arc<glow::Context>,
}

impl WindowRunner {
    pub fn new(
        app: Box<dyn App>,
        sender: Sender<ToMainMessages>,
        attributes: WindowAttributes,
    ) -> Self {
        Self {
            app,
            initial_hide: true,
            window: None,
            renderer: None,
            app_start_time: Instant::now(),
            context: Default::default(),
            event_loop_proxy: None,
            window_attributes: attributes,
            sender,
        }
    }

    pub fn run(mut self, event_loop: &mut EventLoop<UserEvent>, initial_hide: bool) -> Result<()> {
        self.initial_hide = initial_hide;
        event_loop.set_control_flow(ControlFlow::Wait);
        self.event_loop_proxy = Some(event_loop.create_proxy());
        self.create_new_context();
        event_loop.run_app_on_demand(&mut self)?;
        Ok(())
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

            if let Err(e) = renderer.gl_surface.swap_buffers(&renderer.gl_context) {
                error!("Failed to swap buffers: {e}");
                self.destroy_window();
            }
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.window.is_none() {
            debug!("Creating Window");
            self.create_new_context();
            let attributes = self.window_attributes.clone();
            let window = Arc::new(event_loop.create_window(attributes)?);
            let renderer = GlowRenderer::new(Arc::clone(&window), &self.context)?;
            self.window = Some(window);
            self.renderer = Some(renderer);
        }

        Ok(())
    }

    fn create_new_context(&mut self) {
        self.context = Context::default();
        prepare_context(&mut self.context);
        self.app.with_context(&self.context);

        if let Some(proxy) = &self.event_loop_proxy {
            self.context.data_mut(|data| {
                data.insert_persisted(Id::new(EVENT_PROXY), EventProxy(Arc::new(proxy.clone())));
            });

            let proxy = proxy.clone();
            self.context.set_request_repaint_callback(move |_info| {
                let _ = proxy.send_event(UserEvent::RequestRedraw);
            });
        } else {
            warn!("Event loop proxy unavailable while creating egui context");
        }

        let _ = self
            .sender
            .send(ToMainMessages::UpdateContext(self.context.clone()));
    }

    fn destroy_window(&mut self) {
        let had_window = self.window.is_some() || self.renderer.is_some();
        self.window = None;
        self.renderer = None;
        if had_window {
            self.app.on_close();
        }
    }
}

fn desktop_entry_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn send_user_event(ctx: &egui::Context, event: UserEvent) {
    let proxy = ctx.data_mut(|data| data.get_persisted::<EventProxy>(Id::new(EVENT_PROXY)));
    if let Some(proxy) = proxy {
        let _ = proxy.0.send_event(event);
    }
}

impl ApplicationHandler<UserEvent> for WindowRunner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.initial_hide {
            if let Err(e) = self.create_window(event_loop) {
                error!("Failed to create window on resume: {e}");
                let _ = self.sender.send(ToMainMessages::Quit);
                event_loop.exit();
            }
        } else {
            self.initial_hide = false;
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::RequestRedraw => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            UserEvent::FocusWindow => {
                if let Err(e) = self.create_window(event_loop) {
                    error!("Failed to create window while focusing: {e}");
                    let _ = self.sender.send(ToMainMessages::Quit);
                    event_loop.exit();
                    return;
                }

                if let Some(window) = &self.window {
                    if let Some(true) = window.is_minimized() {
                        window.set_minimized(false);
                    }

                    window.focus_window();
                    window.request_user_attention(Some(UserAttentionType::Informational));
                }
            }
            UserEvent::DeviceMessage(msg) => {
                self.app.handle_device_message(msg);
            }
            UserEvent::SetAutoStart(create) => {
                let key = Id::new(AUTO_START_KEY);
                if let Some(window) = &self.window {
                    if env::var("FLATPAK_SANDBOX_DIR").is_ok() {
                        println!("Running inside Flatpak, using Background Portal");

                        #[allow(deprecated)]
                        let window_handle = window.raw_window_handle().unwrap();

                        #[allow(deprecated)]
                        let display_handle = window.raw_display_handle().ok();

                        let reason = "Manage Beacn Devices on Startup";

                        run_async_blocking(async {
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
                                    String::from(BACKGROUND_PARAM),
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

                        let attempt = match get_autostart_file() {
                            Ok(path) => {
                                if path.exists() && fs::remove_file(path.clone()).is_err() {
                                    Err(anyhow!("Unable to remove existing AutoStart"))
                                } else if create {
                                    if let Ok(exe) = env::current_exe() {
                                        let mut conf = Ini::new();
                                        let exe = desktop_entry_escape(&exe.to_string_lossy());

                                        conf.with_section(Some("Desktop Entry"))
                                            .set("Type", "Application")
                                            .set("Name", "Beacn Utility")
                                            .set("Comment", "A Tool for Configuring Beacn Devices")
                                            .set("Exec", format!("\"{exe}\" {BACKGROUND_PARAM}"))
                                            .set("Terminal", "false");

                                        match conf.write_to_file(path) {
                                            Ok(()) => Ok(()),
                                            Err(e) => Err(anyhow!("Failed to Write File, {}", e)),
                                        }
                                    } else {
                                        Err(anyhow!("Unable to Determine Executable"))
                                    }
                                } else {
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
            UserEvent::Quit => {
                debug!("Quit Event Received, closing window");
                self.destroy_window();
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
            let response = renderer.winit_state.on_window_event(window, &event);

            if response.repaint && !matches!(&event, WindowEvent::RedrawRequested) {
                window.request_redraw();
            }

            match event {
                WindowEvent::RedrawRequested => {
                    self.render_frame();
                }
                WindowEvent::CloseRequested => {
                    if self.app.should_close() {
                        debug!("Window Closed, cleaning handlers");
                        self.destroy_window();
                    }
                }
                WindowEvent::Destroyed => {
                    debug!("Window Destroyed, cleaning handlers");
                    self.destroy_window();
                }
                WindowEvent::Resized(physical_size) => {
                    self.window_attributes.inner_size = Some(physical_size.into());
                    renderer.resize(physical_size)
                }
                WindowEvent::Moved(position) => {
                    self.window_attributes.position = Some(position.into());
                }
                _ => {
                    if !matches!(
                        event,
                        WindowEvent::CursorMoved { .. }
                            | WindowEvent::MouseInput { .. }
                            | WindowEvent::KeyboardInput { .. }
                            | WindowEvent::CursorEntered { .. }
                            | WindowEvent::AxisMotion { .. }
                            | WindowEvent::CursorLeft { .. }
                            | WindowEvent::MouseWheel { .. }
                    ) {
                        debug!("Unhandled Window Event: {event:?}")
                    }
                }
            }
        }
    }
}

impl GlowRenderer {
    #[allow(deprecated)]
    fn new(window: Arc<Window>, egui_ctx: &egui::Context) -> Result<Self> {
        use glutin::config::ConfigTemplateBuilder;
        use glutin::context::{ContextApi, ContextAttributesBuilder};
        use glutin::prelude::*;
        use glutin::surface::SurfaceAttributesBuilder;

        let raw_window_handle = window
            .raw_window_handle()
            .map_err(|e| anyhow!("Failed to get raw window handle: {e}"))?;
        let raw_display_handle = window
            .raw_display_handle()
            .map_err(|e| anyhow!("Failed to get raw display handle: {e}"))?;

        let config_template = ConfigTemplateBuilder::new()
            .with_transparency(false)
            .build();

        debug!("Creating glutin Display with Config: {:?}", config_template);

        let gl_display = unsafe {
            glutin::display::Display::new(raw_display_handle, DisplayApiPreference::Egl)
                .map_err(|e| anyhow!("Failed to create GL display: {e}"))?
        };

        let config = unsafe {
            gl_display
                .find_configs(config_template)
                .map_err(|e| anyhow!("Failed to enumerate GL configs: {e}"))?
                .max_by_key(|config| config.num_samples())
                .ok_or_else(|| anyhow!("No compatible OpenGL config found"))?
        };

        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(None))
            .build(Some(raw_window_handle));

        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw_window_handle));

        let not_current_gl_context = unsafe {
            match gl_display.create_context(&config, &context_attributes) {
                Ok(ctx) => ctx,
                Err(e) => {
                    warn!("Failed to Create OpenGL Context, trying OpenGL ES: {}", e);
                    gl_display
                        .create_context(&config, &fallback_context_attributes)
                        .map_err(|e| anyhow!("Failed to create OpenGL ES context: {e}"))?
                }
            }
        };

        let size = window.inner_size();
        let surface_attributes = SurfaceAttributesBuilder::<glutin::surface::WindowSurface>::new()
            .build(
                raw_window_handle,
                size.width
                    .try_into()
                    .map_err(|_| anyhow!("Invalid window width"))?,
                size.height
                    .try_into()
                    .map_err(|_| anyhow!("Invalid window height"))?,
            );

        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&config, &surface_attributes)
                .map_err(|e| anyhow!("Failed to create GL window surface: {e}"))?
        };

        let gl_context = not_current_gl_context
            .make_current(&gl_surface)
            .map_err(|e| anyhow!("Failed to make GL context current: {e}"))?;

        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                let s = std::ffi::CString::new(s)
                    .expect("failed to construct C string from string for gl proc address");
                gl_display.get_proc_address(&s)
            })
        });

        let viewport_id = egui_ctx.viewport_id();
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            viewport_id,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let painter = egui_glow::Painter::new(Arc::clone(&gl), "", None, false)
            .map_err(|e| anyhow!("Failed to create egui glow painter: {e}"))?;

        Ok(Self {
            gl_context,
            gl_surface,
            winit_state: egui_winit,
            painter,
            gl,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            let width = match new_size.width.try_into() {
                Ok(width) => width,
                Err(_) => {
                    warn!("Invalid resize width: {}", new_size.width);
                    return;
                }
            };
            let height = match new_size.height.try_into() {
                Ok(height) => height,
                Err(_) => {
                    warn!("Invalid resize height: {}", new_size.height);
                    return;
                }
            };

            self.gl_surface.resize(&self.gl_context, width, height);

            unsafe {
                self.gl
                    .viewport(0, 0, new_size.width as i32, new_size.height as i32);
            }
        }
    }

    fn render_egui(&mut self, full_output: &egui::FullOutput, egui_ctx: &egui::Context) {
        let clipped_primitives =
            egui_ctx.tessellate(full_output.shapes.clone(), full_output.pixels_per_point);

        let Some(width) = self.gl_surface.width() else {
            warn!("GL surface width unavailable, skipping frame");
            return;
        };
        let Some(height) = self.gl_surface.height() else {
            warn!("GL surface height unavailable, skipping frame");
            return;
        };
        let dimensions = [width, height];

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
