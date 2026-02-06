use glam::Vec2;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Tracks current input state (keys held, mouse position, etc.)
pub struct InputState {
    pub mouse_pos: Vec2,
    pub mouse_delta: Vec2,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub middle_pressed: bool,
    /// True for one frame when button first pressed
    pub left_just_clicked: bool,
    pub right_just_clicked: bool,
    pub scroll_delta: f32,
    pub keys_held: std::collections::HashSet<KeyCode>,
    pub keys_just_pressed: std::collections::HashSet<KeyCode>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            left_pressed: false,
            right_pressed: false,
            middle_pressed: false,
            left_just_clicked: false,
            right_just_clicked: false,
            scroll_delta: 0.0,
            keys_held: std::collections::HashSet::new(),
            keys_just_pressed: std::collections::HashSet::new(),
        }
    }

    /// Call at the start of each frame to clear per-frame state.
    pub fn begin_frame(&mut self) {
        self.mouse_delta = Vec2::ZERO;
        self.scroll_delta = 0.0;
        self.left_just_clicked = false;
        self.right_just_clicked = false;
        self.keys_just_pressed.clear();
    }

    pub fn handle_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = Vec2::new(position.x as f32, position.y as f32);
                self.mouse_delta = new_pos - self.mouse_pos;
                self.mouse_pos = new_pos;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = *state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        if pressed && !self.left_pressed {
                            self.left_just_clicked = true;
                        }
                        self.left_pressed = pressed;
                    }
                    MouseButton::Right => {
                        if pressed && !self.right_pressed {
                            self.right_just_clicked = true;
                        }
                        self.right_pressed = pressed;
                    }
                    MouseButton::Middle => self.middle_pressed = pressed,
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll_delta += match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 120.0,
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if !self.keys_held.contains(&key) {
                            self.keys_just_pressed.insert(key);
                        }
                        self.keys_held.insert(key);
                    } else {
                        self.keys_held.remove(&key);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn key_held(&self, key: KeyCode) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }

    pub fn space_held(&self) -> bool {
        self.key_held(KeyCode::Space)
    }
}
