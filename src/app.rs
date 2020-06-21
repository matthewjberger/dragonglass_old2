use log::{debug, info};
use simplelog::*;
use snafu::{ResultExt, Snafu};
use std::fs::File;
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use serde::Deserialize;

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

pub struct App;

impl App {
    pub const TITLE: &'static str = "Dragonglass - GLTF Model Viewer";
    pub const LOG_FILE: &'static str = "dragonglass.log";
    pub const SETTINGS_FILE: &'static str = "settings.toml";

    pub fn run() -> Result<()> {
        Self::setup_logger()?;
        info!("Setting up app.");

        let settings = Self::load_settings()?;

        let event_loop = EventLoop::new();
        let _window = WindowBuilder::new()
            .with_title(Self::TITLE)
            .with_inner_size(PhysicalSize::new(
                settings.width as u32,
                settings.height as u32,
            ))
            .build(&event_loop)
            .context(CreateWindow)?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(keycode),
                                ..
                            },
                        ..
                    } => {
                        if keycode == VirtualKeyCode::Escape {
                            *control_flow = ControlFlow::Exit;
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn setup_logger() -> Result<()> {
        let logfile_name = "dragonglass.log";
        CombinedLogger::init(vec![
            TermLogger::new(LevelFilter::max(), Config::default(), TerminalMode::Mixed),
            WriteLogger::new(
                LevelFilter::Info,
                Config::default(),
                File::create(&logfile_name).context(CreateLogFile {
                    name: logfile_name.to_string(),
                })?,
            ),
        ])
        .context(SetLogger {})
    }

    fn load_settings() -> Result<Settings> {
        debug!("Loading settings file");
        let mut config = config::Config::default();
        config
            .merge(config::File::with_name("settings"))
            .context(LoadSettingsFile {
                name: Self::SETTINGS_FILE.to_string(),
            })?;
        let settings: Settings = config.try_into().context(DeserializeSettings)?;
        Ok(settings)
    }
}
