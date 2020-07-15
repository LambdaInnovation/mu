use crate::math::Vec2;
use cgmath::vec2;
use glutin::event;

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum ButtonState {
    Released, Down, Pressing, Up
}

pub struct RawInputData {
    // Keyboard
    frame_character_list: Vec<char>,
    key_state: [ButtonState;256],
    // Mouse
    mouse_button_state: [ButtonState; 8],
    mouse_wheel_delta: f32,
    mouse_frame_movement: Vec2,
    cursor_position: Vec2
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
                    event::MouseScrollDelta::LineDelta(dx, dy) => {
                        info!("LineDelta");
                    },
                    event::MouseScrollDelta::PixelDelta(pos) => {
                        info!("PixelDelta");
                    }
                }
            },
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
        RawInputData::_iter_button_state(&mut self.key_state);
        RawInputData::_iter_button_state(&mut self.mouse_button_state);
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

    fn _mouse_btn_to_id(btn: event::MouseButton) -> u8 {
        match btn {
            event::MouseButton::Left => 0,
            event::MouseButton::Right => 1,
            event::MouseButton::Middle => 2,
            event::MouseButton::Other(id) => 3 + id
        }
    }

    fn _id_to_mouse_btn(id: u8) -> event::MouseButton {
        match id {
            0 => event::MouseButton::Left,
            1 => event::MouseButton::Right,
            2 => event::MouseButton::Middle,
            _ => event::MouseButton::Other(id - 3)
        }
    }

}