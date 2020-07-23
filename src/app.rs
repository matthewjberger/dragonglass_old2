use crate::{
    camera::{FreeCamera, OrbitalCamera},
    renderer::{Backend, Renderer},
};
use anyhow::{Context, Result};
use imgui::Context as ImguiContext;
use imgui::*;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use log::debug;
use nalgebra_glm as glm;
use serde::Deserialize;
use simplelog::*;
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

        renderer.initialize(&app, &mut imgui);

        let mut last_frame = Instant::now();
        let mut cursor_moved = false;
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            platform.handle_event(imgui.io_mut(), &window, &event);

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

                        if keycode == VirtualKeyCode::Tab && state == ElementState::Pressed {
                            app.using_free_camera = !app.using_free_camera;
                            let _ = app.setup_camera(&window);
                        }
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
                        let center = app.window_center();
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

                    renderer.render(&app, &draw_data);

                    if !cursor_moved {
                        app.input.mouse.position_delta = glm::vec2(0.0, 0.0);
                    }
                    cursor_moved = false;
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

    fn window_center(&self) -> PhysicalPosition<i32> {
        PhysicalPosition::new(
            (self.window_dimensions.x / 2.0) as i32,
            (self.window_dimensions.y / 2.0) as i32,
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
            self.free_camera.update(&self.input, self.delta_time as f32);
        } else {
            self.orbital_camera
                .update(&self.input, self.delta_time as f32);
        }
    }

    fn reset_controls(&mut self, window: &mut winit::window::Window) {
        if self.using_free_camera {
            // Center cursor for free camera
            let _ = window.set_cursor_position(self.window_center());
        }
        self.input.mouse.wheel_delta = 0.0;
    }
}
