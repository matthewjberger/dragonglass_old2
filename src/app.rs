use crate::{
    camera::{
        fps_camera_controls_system, orbital_camera_controls_system, FreeCamera, OrbitalCamera,
    },
    gui::Gui,
    input::Input,
    renderer::{AssetName, Backend, Renderer, Transform},
    system::System,
};
use anyhow::{Context, Result};
use legion::prelude::*;
use log::{debug, warn};
use nalgebra_glm as glm;
use serde::Deserialize;
use simplelog::*;
use std::fs::File;
use winit::{
    dpi::PhysicalSize,
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
pub struct App;

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

        let window_dimensions = glm::vec2(
            window.inner_size().width as _,
            window.inner_size().height as _,
        );

        let mut resources = Resources::default();
        resources.insert(Input::default());
        resources.insert(System::new(window_dimensions));

        let universe = Universe::new();
        let mut world = universe.create_world();

        // FIXME: Add tag to mark this as the main camera
        world.insert((), vec![(OrbitalCamera::default(),)]);

        let mut update_schedule = Schedule::builder()
            .add_system(fps_camera_controls_system())
            .add_system(orbital_camera_controls_system())
            .flush()
            .build();

        let mut gui = Gui::new(&window);
        let mut renderer = Renderer::create_backend(&Backend::Vulkan, &mut window)?;
        renderer.initialize(&world, &mut gui.context_mut());

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            if let Some(mut system) = resources.get_mut::<System>() {
                system.handle_event(&event);

                if system.exit_requested {
                    *control_flow = ControlFlow::Exit;
                }
            }

            gui.handle_event(&event, &window);

            if let Some(mut input) = resources.get_mut::<Input>() {
                let system = resources
                    .get::<System>()
                    .expect("Failed to get system resource!");
                input.handle_event(&event, system.window_center());
                input.allowed = !gui.capturing_input();

                if input.is_key_pressed(VirtualKeyCode::Escape) {
                    *control_flow = ControlFlow::Exit;
                }
            }

            match event {
                Event::NewEvents { .. } => {
                    update_schedule.execute(&mut world, &mut resources);
                }
                Event::MainEventsCleared => {
                    let draw_data = gui
                        .render_frame(&window)
                        .expect("Failed to render gui frame!");

                    renderer.render(&world, &resources, &draw_data);
                }
                Event::WindowEvent {
                    event: WindowEvent::DroppedFile(path),
                    ..
                } => {
                    if let Some(raw_path) = path.to_str() {
                        if let Some(extension) = path.extension() {
                            match extension.to_str() {
                                Some("glb") | Some("gltf") => {
                                    world.insert(
                                        (),
                                        vec![(
                                            Transform::default(),
                                            AssetName(raw_path.to_string()),
                                        )],
                                    );
                                }
                                _ => warn!(
                                "File extension {:#?} is not a valid '.glb' or '.gltf' extension",
                                extension
                            ),
                            }
                        }
                    }
                }
                _ => {}
            }
        });
    }

    fn setup_logger() -> Result<()> {
        CombinedLogger::init(vec![
            TermLogger::new(LevelFilter::max(), Config::default(), TerminalMode::Mixed),
            WriteLogger::new(
                LevelFilter::max(),
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
}
