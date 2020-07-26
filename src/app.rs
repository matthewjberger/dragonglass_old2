use crate::{
    camera::{FreeCamera, OrbitalCamera},
    input::Input,
    renderer::{Backend, Renderer},
};
use anyhow::{Context, Result};
use imgui::Context as ImguiContext;
use imgui::*;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use legion::prelude::*;
use log::debug;
use nalgebra_glm as glm;
use serde::Deserialize;
use simplelog::*;
use std::{fs::File, time::Instant};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[derive(Debug, Deserialize)]
pub struct Settings {
    width: i64,
    height: i64,
}

#[derive(Default)]
pub struct App {
    pub window_dimensions: glm::Vec2,
    pub input: Input,
    pub delta_time: f64,
    pub free_camera: FreeCamera,
    pub orbital_camera: OrbitalCamera,
    pub using_free_camera: bool,
}

impl App {
    pub const TITLE: &'static str = "Dragonglass - GLTF Model Viewer";
    pub const LOG_FILE: &'static str = "dragonglass.log";
    pub const SETTINGS_FILE: &'static str = "settings.toml";

    pub fn run() -> Result<()> {
        Self::setup_logger()?;

        let settings = Self::load_settings()?;

        let event_loop = EventLoop::new();
        let mut window = WindowBuilder::new()
            .with_title(Self::TITLE)
            .with_inner_size(PhysicalSize::new(
                settings.width as u32,
                settings.height as u32,
            ))
            .build(&event_loop)?;

        let mut app = App::default();
        app.window_dimensions = glm::vec2(
            window.inner_size().width as _,
            window.inner_size().height as _,
        );

        app.setup_camera(&window)?;

        // IMGUI
        let mut imgui = ImguiContext::create();
        imgui.set_ini_filename(None);

        let mut platform = WinitPlatform::init(&mut imgui);

        let hidpi_factor = platform.hidpi_factor();
        let font_size = (13.0 * hidpi_factor) as f32;
        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                size_pixels: font_size,
                ..FontConfig::default()
            }),
        }]);
        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Rounded);

        let mut renderer = Renderer::create_backend(&Backend::Vulkan, &mut window)?;

        renderer.initialize(&mut imgui);

        let universe = Universe::new();
        let mut world = universe.create_world();

        //let mut schedule = Schedule::builder().add_system().flush().build();
        //schedule.execute();

        let mut last_frame = Instant::now();
        let mut cursor_moved = false;
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            platform.handle_event(imgui.io_mut(), &window, &event);
            app.input.handle_event(&event, app.window_center());

            if app.input.is_key_pressed(VirtualKeyCode::Escape) {
                *control_flow = ControlFlow::Exit;
            }

            if app.input.is_key_pressed(VirtualKeyCode::Tab) {
                app.using_free_camera = !app.using_free_camera;
                let _ = app.setup_camera(&window);
            }

            match event {
                Event::NewEvents { .. } => {
                    app.delta_time = (Instant::now().duration_since(last_frame).as_micros() as f64)
                        / 1_000_000_f64;
                    last_frame = Instant::now();

                    app.update_camera();

                    renderer.update(&app);

                    app.reset_controls(&mut window);
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(PhysicalSize { width, height }) => {
                        app.window_dimensions = glm::vec2(width as f32, height as f32);
                    }
                    _ => {}
                },
                Event::MainEventsCleared => {
                    platform
                        .prepare_frame(imgui.io_mut(), &window)
                        .expect("Failed to prepare frame");

                    let ui = imgui.frame();

                    Window::new(im_str!("Hello world"))
                        .size([300.0, 100.0], Condition::FirstUseEver)
                        .build(&ui, || {
                            ui.text(im_str!("Hello world!"));
                            ui.text(im_str!("This...is...imgui-rs!"));
                            ui.separator();
                            let mouse_pos = ui.io().mouse_pos;
                            ui.text(format!(
                                "Mouse Position: ({:.1},{:.1})",
                                mouse_pos[0], mouse_pos[1]
                            ));
                        });

                    platform.prepare_render(&ui, &window);
                    let draw_data = ui.render();

                    renderer.render(&app.window_dimensions, &draw_data);
                }
                _ => {}
            }
        });
    }

    fn setup_logger() -> Result<()> {
        CombinedLogger::init(vec![
            TermLogger::new(LevelFilter::max(), Config::default(), TerminalMode::Mixed),
            WriteLogger::new(
                LevelFilter::Info,
                Config::default(),
                File::create(Self::LOG_FILE)
                    .with_context(|| format!("log file path: {}", Self::LOG_FILE.to_string()))?,
            ),
        ])?;
        Ok(())
    }

    fn load_settings() -> Result<Settings> {
        debug!("Loading settings file");
        let mut config = config::Config::default();
        config
            .merge(config::File::with_name(Self::SETTINGS_FILE))
            .with_context(|| format!("settings file path: {}", Self::SETTINGS_FILE.to_string()))?;
        let settings: Settings = config.try_into()?;
        Ok(settings)
    }

    fn window_center(&self) -> glm::Vec2 {
        glm::vec2(
            (self.window_dimensions.x / 2.0) as _,
            (self.window_dimensions.y / 2.0) as _,
        )
    }

    fn setup_camera(&mut self, window: &winit::window::Window) -> Result<()> {
        if self.using_free_camera {
            self.free_camera.position_at(&glm::vec3(0.0, -4.0, -4.0));
            self.free_camera.look_at(&glm::vec3(0.0, 0.0, 0.0));

            // Free camera setup. Hide, grab, and center cursor
            window.set_cursor_visible(false);
            let _ = window.set_cursor_grab(true);

            let center = self.window_center();
            window.set_cursor_position(PhysicalPosition::new(center.x as i32, center.y as i32))?;
            self.input.mouse.position = glm::vec2(center.x as _, center.y as _);
        } else {
            // orbital
            window.set_cursor_visible(true);
            let _ = window.set_cursor_grab(false);
        }

        Ok(())
    }

    fn update_camera(&mut self) {
        if self.using_free_camera {
            self.free_camera.update(&self.input, self.delta_time as f32);
        } else {
            self.orbital_camera
                .update(&self.input, self.delta_time as f32);
        }
    }

    fn reset_controls(&mut self, window: &mut winit::window::Window) {
        if self.using_free_camera {
            let center = self.window_center();
            // Center cursor for free camera
            let _ =
                window.set_cursor_position(PhysicalPosition::new(center.x as i32, center.y as i32));
        }
        self.input.mouse.wheel_delta = 0.0;
    }
}
