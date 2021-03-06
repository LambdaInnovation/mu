use crate::math::*;
use winit::event;

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum ButtonState {
    Released, Down, Pressing, Up
}

impl ButtonState {
    pub fn is_down(&self) -> bool {
        *self == ButtonState::Down || *self == ButtonState::Pressing
    }

    pub fn is_up(&self) -> bool {
        *self == ButtonState::Up || *self == ButtonState::Released
    }
}

/// Processed input data for raw input device (keyboard, mouse, controller, etc.)
pub struct RawInputData {
    // Keyboard
    pub frame_character_list: Vec<char>,
    key_state: [ButtonState;256],
    // Mouse
    mouse_button_state: [ButtonState; 8],
    pub mouse_wheel_delta: f32,
    pub mouse_frame_movement: Vec2,
    pub cursor_position: Vec2
}

impl RawInputData {

    pub fn new() -> Self {
        Self {
            frame_character_list: vec![],
            key_state: [ButtonState::Released;256],
            mouse_button_state: [ButtonState::Released;8],
            mouse_wheel_delta: 0.,
            mouse_frame_movement: vec2(0., 0.),
            cursor_position: vec2(0., 0.)
        }
    }

    pub fn on_window_event(&mut self, ev: &event::WindowEvent) {
        match ev {
            event::WindowEvent::ReceivedCharacter(ch) => {
                self.frame_character_list.push(*ch)
            }
            event::WindowEvent::KeyboardInput { input, .. } => {
                match input.virtual_keycode {
                    Some(k) => {
                        let s = match input.state {
                            event::ElementState::Pressed => ButtonState::Down,
                            event::ElementState::Released => ButtonState::Up
                        };
                        self.key_state[k as usize] = s;
                    }
                    _ => ()
                }
            },
            event::WindowEvent::MouseInput { button, state, .. } => {
                let id = RawInputData::_mouse_btn_to_id(*button);
                let s = match state {
                    event::ElementState::Pressed => ButtonState::Down,
                    event::ElementState::Released => ButtonState::Up
                };
                self.mouse_button_state[id as usize] = s;
            },
            event::WindowEvent::MouseWheel { delta, ..  } => {
                match delta {
                    event::MouseScrollDelta::LineDelta(_dx, dy) => {
                        self.mouse_wheel_delta += *dy;
                    },
                    _ => ()
                    // event::MouseScrollDelta::PixelDelta(pos) => {
                    //     // info!("PixelDelta");
                    // }
                }
            },
            event::WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = vec2(position.x as f32, position.y as f32);
            }
            _ => ()
        }
    }

    pub fn on_device_event(&mut self, ev: &event::DeviceEvent) {
        match ev {
            event::DeviceEvent::MouseMotion { delta: (dx, dy) } => {
                self.mouse_frame_movement = vec2(*dx as f32, *dy as f32)
            },
            _ => ()
        }
    }

    pub fn on_frame_end(&mut self) {
        self.frame_character_list.clear();
        self.mouse_frame_movement = vec2(0., 0.);
        self.mouse_wheel_delta = 0.;
        RawInputData::_iter_button_state(&mut self.key_state);
        RawInputData::_iter_button_state(&mut self.mouse_button_state);
    }

    pub fn get_key(&self, key: event::VirtualKeyCode) -> ButtonState {
        self.key_state[key as usize]
    }

    pub fn get_mouse_button(&self, btn: event::MouseButton) -> ButtonState {
        self.mouse_button_state[Self::_mouse_btn_to_id(btn) as usize]
    }

    pub fn get_mouse_buttons(&self) -> [ButtonState; 8] {
        self.mouse_button_state.clone()
    }

    fn _iter_button_state(v: &mut [ButtonState]) {
        for i in 0..v.len() {
            if v[i] == ButtonState::Down {
                v[i] = ButtonState::Pressing;
            } else if v[i] == ButtonState::Up {
                v[i] = ButtonState::Released;
            }
        }
    }

    fn _mouse_btn_to_id(btn: event::MouseButton) -> u16 {
        match btn {
            event::MouseButton::Left => 0,
            event::MouseButton::Right => 1,
            event::MouseButton::Middle => 2,
            event::MouseButton::Other(id) => 3 + id
        }
    }

    fn _id_to_mouse_btn(id: u16) -> event::MouseButton {
        match id {
            0 => event::MouseButton::Left,
            1 => event::MouseButton::Right,
            2 => event::MouseButton::Middle,
            _ => event::MouseButton::Other(id - 3)
        }
    }

}