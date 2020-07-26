use crate::{
    camera::{FreeCamera, OrbitalCamera},
    gui::Gui,
    input::Input,
    renderer::{Backend, Renderer},
    system::System,
};
use anyhow::{Context, Result};
use legion::prelude::*;
use log::debug;
use nalgebra_glm as glm;
use serde::Deserialize;
use simplelog::*;
use std::fs::File;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[derive(Debug, Deserialize)]
pub struct Settings {
    width: i64,
    height: i64,
}

pub struct App {
    pub input: Input,
    pub system: System,
    pub free_camera: FreeCamera,
    pub orbital_camera: OrbitalCamera,
    pub using_free_camera: bool,
}

impl App {
    pub const TITLE: &'static str = "Dragonglass - GLTF Model Viewer";
    pub const LOG_FILE: &'static str = "dragonglass.log";
    pub const SETTINGS_FILE: &'static str = "settings.toml";

    pub fn new(window_dimensions: glm::Vec2) -> Self {
        Self {
            input: Input::default(),
            system: System::new(window_dimensions),
            free_camera: FreeCamera::default(),
            orbital_camera: OrbitalCamera::default(),
            using_free_camera: false,
        }
    }

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

        let window_dimensions = glm::vec2(
            window.inner_size().width as _,
            window.inner_size().height as _,
        );

        let mut app = App::new(window_dimensions);
        app.setup_camera(&window)?;

        let mut gui = Gui::new(&window);
        let mut renderer = Renderer::create_backend(&Backend::Vulkan, &mut window)?;

        renderer.initialize(&mut gui.context_mut());

        let universe = Universe::new();
        let mut world = universe.create_world();

        //let mut schedule = Schedule::builder().add_system().flush().build();
        //schedule.execute();

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            app.system.handle_event(&event);

            app.input.handle_event(&event, app.system.window_center());

            gui.handle_event(&event, &window);

            if app.input.is_key_pressed(VirtualKeyCode::Escape) {
                *control_flow = ControlFlow::Exit;
            }

            if app.input.is_key_pressed(VirtualKeyCode::Tab) {
                app.using_free_camera = !app.using_free_camera;
                let _ = app.setup_camera(&window);
            }

            match event {
                Event::NewEvents { .. } => {
                    app.update_camera();

                    renderer.update(&app);

                    app.reset_controls(&mut window);
                }
                Event::MainEventsCleared => {
                    let draw_data = gui
                        .render_frame(&window)
                        .expect("Failed to render gui frame!");

                    renderer.render(&app.system.window_dimensions, &draw_data);
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

    fn setup_camera(&mut self, window: &winit::window::Window) -> Result<()> {
        if self.using_free_camera {
            self.free_camera.position_at(&glm::vec3(0.0, -4.0, -4.0));
            self.free_camera.look_at(&glm::vec3(0.0, 0.0, 0.0));

            // Free camera setup. Hide, grab, and center cursor
            window.set_cursor_visible(false);
            let _ = window.set_cursor_grab(true);

            let center = self.system.window_center_physical();
            window.set_cursor_position(center)?;
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
            self.free_camera
                .update(&self.input, self.system.delta_time as f32);
        } else {
            self.orbital_camera
                .update(&self.input, self.system.delta_time as f32);
        }
    }

    fn reset_controls(&mut self, window: &mut winit::window::Window) {
        if self.using_free_camera {
            let center = self.system.window_center_physical();
            // Center cursor for free camera
            let _ = window.set_cursor_position(center);
        }
        self.input.mouse.wheel_delta = 0.0;
    }
}
