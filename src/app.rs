use log::debug;
use nalgebra_glm as glm;
use serde::Deserialize;
use simplelog::*;
use snafu::{ResultExt, Snafu};
use std::collections::HashMap;
use std::{fs::File, time::Instant};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to set a logger: {}", source))]
    SetLogger { source: log::SetLoggerError },

    #[snafu(display("Failed to create a log file named '{}': {}", name, source))]
    CreateLogFile {
        name: String,
        source: std::io::Error,
    },

    #[snafu(display("Failed to create a window: {}", source))]
    CreateWindow { source: winit::error::OsError },

    #[snafu(display("Failed to load a settings file named '{}': {}", name, source))]
    LoadSettingsFile {
        name: String,
        source: config::ConfigError,
    },

    #[snafu(display("Failed to deserialize the settings file: {}", source))]
    DeserializeSettings { source: config::ConfigError },

    #[snafu(display("Failed to lookup the settings key '{}': {}", name, source))]
    LookupKey {
        name: String,
        source: config::ConfigError,
    },
}

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
}

impl App {
    pub const TITLE: &'static str = "Dragonglass - GLTF Model Viewer";
    pub const LOG_FILE: &'static str = "dragonglass.log";
    pub const SETTINGS_FILE: &'static str = "settings.toml";

    pub fn run() -> Result<()> {
        Self::setup_logger()?;

        let mut app = App::default();
        let settings = Self::load_settings()?;

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(Self::TITLE)
            .with_inner_size(PhysicalSize::new(
                settings.width as u32,
                settings.height as u32,
            ))
            .build(&event_loop)
            .context(CreateWindow)?;

        let mut last_frame = Instant::now();
        let mut cursor_moved = false;
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::NewEvents { .. } => {
                    app.delta_time = (Instant::now().duration_since(last_frame).as_micros() as f64)
                        / 1_000_000_f64;
                    last_frame = Instant::now();
                    // TODO: Update()
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(keycode),
                                state,
                                ..
                            },
                        ..
                    } => {
                        if keycode == VirtualKeyCode::Escape {
                            *control_flow = ControlFlow::Exit;
                        }
                        *app.input.keystates.entry(keycode).or_insert(state) = state;
                    }
                    WindowEvent::Resized(PhysicalSize { width, height }) => {
                        app.window_dimensions = glm::vec2(width as f32, height as f32);
                    }
                    WindowEvent::MouseInput { button, state, .. } => {
                        let clicked = state == ElementState::Pressed;
                        match button {
                            MouseButton::Left => app.input.mouse.is_left_clicked = clicked,
                            MouseButton::Right => app.input.mouse.is_right_clicked = clicked,
                            _ => {}
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let last_position = app.input.mouse.position;
                        let current_position = glm::vec2(position.x as _, position.y as _);
                        app.input.mouse.position = current_position;
                        app.input.mouse.position_delta = current_position - last_position;
                        let center = PhysicalPosition::new(
                            (app.window_dimensions.x / 2.0) as i32,
                            (app.window_dimensions.y / 2.0) as i32,
                        );
                        app.input.mouse.offset_from_center = glm::vec2(
                            (center.x - position.x as i32) as _,
                            (center.y - position.y as i32) as _,
                        );
                        cursor_moved = true;
                    }
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, v_lines),
                        ..
                    } => {
                        app.input.mouse.wheel_delta = v_lines;
                    }
                    _ => {}
                },
                Event::MainEventsCleared => {
                    if !cursor_moved {
                        app.input.mouse.position_delta = glm::vec2(0.0, 0.0);
                    }
                    cursor_moved = false;

                    window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    // TODO: Draw app
                }
                Event::RedrawEventsCleared => {
                    app.input.mouse.wheel_delta = 0.0;
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
                File::create(Self::LOG_FILE).context(CreateLogFile {
                    name: Self::LOG_FILE.to_string(),
                })?,
            ),
        ])
        .context(SetLogger {})
    }

    fn load_settings() -> Result<Settings> {
        debug!("Loading settings file");
        let mut config = config::Config::default();
        config
            .merge(config::File::with_name(Self::SETTINGS_FILE))
            .context(LoadSettingsFile {
                name: Self::SETTINGS_FILE.to_string(),
            })?;
        let settings: Settings = config.try_into().context(DeserializeSettings)?;
        Ok(settings)
    }
}

pub type KeyMap = HashMap<VirtualKeyCode, ElementState>;

#[derive(Default)]
pub struct Input {
    pub keystates: KeyMap,
    pub mouse: Mouse,
}

pub struct Mouse {
    pub is_left_clicked: bool,
    pub is_right_clicked: bool,
    pub position: glm::Vec2,
    pub position_delta: glm::Vec2,
    pub offset_from_center: glm::Vec2,
    pub wheel_delta: f32,
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            is_left_clicked: false,
            is_right_clicked: false,
            position: glm::vec2(0.0, 0.0),
            position_delta: glm::vec2(0.0, 0.0),
            offset_from_center: glm::vec2(0.0, 0.0),
            wheel_delta: 0.0,
        }
    }
}

impl Input {
    pub fn is_key_pressed(&self, keycode: VirtualKeyCode) -> bool {
        self.keystates.contains_key(&keycode) && self.keystates[&keycode] == ElementState::Pressed
    }
}
