use nalgebra_glm as glm;
use std::collections::HashMap;
use winit::event::{
    ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent,
};

pub type KeyMap = HashMap<VirtualKeyCode, ElementState>;

#[derive(Default)]
pub struct Input {
    pub keystates: KeyMap,
    pub mouse: Mouse,
}

impl Input {
    pub fn is_key_pressed(&self, keycode: VirtualKeyCode) -> bool {
        self.keystates.contains_key(&keycode) && self.keystates[&keycode] == ElementState::Pressed
    }

    pub fn handle_event<T>(&mut self, event: &Event<T>, window_center: glm::Vec2) {
        match event {
            Event::WindowEvent { event, .. } => match *event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(keycode),
                            state,
                            ..
                        },
                    ..
                } => {
                    *self.keystates.entry(keycode).or_insert(state) = state;
                }
                _ => {}
            },
            _ => {}
        }

        self.mouse.handle_event(&event, window_center);
    }
}

pub struct Mouse {
    pub is_left_clicked: bool,
    pub is_right_clicked: bool,
    pub position: glm::Vec2,
    pub position_delta: glm::Vec2,
    pub offset_from_center: glm::Vec2,
    pub wheel_delta: f32,
    pub moved: bool,
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
            moved: false,
        }
    }
}

impl Mouse {
    pub fn handle_event<T>(&mut self, event: &Event<T>, window_center: glm::Vec2) {
        match event {
            Event::NewEvents { .. } => {
                if !self.moved {
                    self.position_delta = glm::vec2(0.0, 0.0);
                }
                self.moved = false;
            }
            Event::WindowEvent { event, .. } => match *event {
                WindowEvent::MouseInput { button, state, .. } => {
                    let clicked = state == ElementState::Pressed;
                    match button {
                        MouseButton::Left => self.is_left_clicked = clicked,
                        MouseButton::Right => self.is_right_clicked = clicked,
                        _ => {}
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let last_position = self.position;
                    let current_position = glm::vec2(position.x as _, position.y as _);
                    self.position = current_position;
                    self.position_delta = current_position - last_position;
                    self.offset_from_center =
                        window_center - glm::vec2(position.x as _, position.y as _);
                    self.moved = true;
                }
                WindowEvent::MouseWheel {
                    delta: MouseScrollDelta::LineDelta(_, v_lines),
                    ..
                } => {
                    self.wheel_delta = v_lines;
                }
                _ => {}
            },
            _ => {}
        }
    }
}