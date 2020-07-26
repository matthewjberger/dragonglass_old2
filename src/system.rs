use nalgebra_glm as glm;
use std::{collections::HashMap, time::Instant};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
};

pub struct System {
    pub window_dimensions: glm::Vec2,
    pub delta_time: f64,
    pub last_frame: Instant,
    pub exit_requested: bool,
}

impl System {
    pub fn new() -> Self {
        Self {
            last_frame: Instant::now(),
            window_dimensions: glm::Vec2::default(),
            delta_time: 0.01,
            exit_requested: false,
        }
    }

    pub fn window_center(&self) -> glm::Vec2 {
        glm::vec2(
            (self.window_dimensions.x / 2.0) as _,
            (self.window_dimensions.y / 2.0) as _,
        )
    }

    pub fn window_center_physical(&self) -> PhysicalPosition<i32> {
        PhysicalPosition::new(
            (self.window_dimensions.x / 2.0) as i32,
            (self.window_dimensions.y / 2.0) as i32,
        )
    }

    pub fn handle_event<T>(&mut self, event: &Event<T>) {
        match event {
            Event::NewEvents { .. } => {
                self.delta_time = (Instant::now().duration_since(self.last_frame).as_micros()
                    as f64)
                    / 1_000_000_f64;
                self.last_frame = Instant::now();
            }
            Event::WindowEvent { event, .. } => match *event {
                WindowEvent::CloseRequested => self.exit_requested = true,
                WindowEvent::Resized(PhysicalSize { width, height }) => {
                    self.window_dimensions = glm::vec2(width as f32, height as f32);
                }
                _ => {}
            },
            _ => {}
        }
    }
}
