/*!
Handles raw input (direct from devices) and virtual input (specified by game).
*/
use crate::client::*;
use crate::math::*;
use glutin as glt;
use glt::event as event;;
use glutin::{ElementState, Event};
use specs::prelude::*;
use std::collections::HashMap;
use std::hash::Hash;

#[repr(u8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum ButtonState {
    Up,
    Down,
    Idle,
    Hold,
}

impl ButtonState {
    pub fn is_down(&self) -> bool {
        *self == ButtonState::Down || *self == ButtonState::Hold
    }
    #[allow(dead_code)]
    pub fn is_up(&self) -> bool {
        *self == ButtonState::Up || *self == ButtonState::Idle
    }
}

struct ButtonGroup<T: Hash + Eq + Clone> {
    states: HashMap<T, ButtonState>,
}

impl<T: Hash + Eq + Clone> ButtonGroup<T> {
    pub fn new() -> Self {
        ButtonGroup {
            states: HashMap::new(),
        }
    }

    pub fn on_element_state(&mut self, ix: T, state: ElementState) {
        let prev_state = if self.states.contains_key(&ix) {
            self.states[&ix]
        } else {
            ButtonState::Idle
        };
        if prev_state == ButtonState::Hold && state == ElementState::Pressed {
            return;
        }
        self.states.insert(
            ix,
            if state == ElementState::Pressed {
                ButtonState::Down
            } else {
                ButtonState::Up
            },
        );
    }

    pub fn on_flush(&mut self) {
        self.states = self
            .states
            .iter()
            .map(|(k, v)| {
                let replace = match v {
                    ButtonState::Down => ButtonState::Hold,
                    ButtonState::Up => ButtonState::Idle,
                    _ => *v,
                };
                (k.clone(), replace)
            })
            .collect();
    }

    pub fn get_state(&self, ix: T) -> ButtonState {
        if self.states.contains_key(&ix) {
            self.states[&ix].clone()
        } else {
            ButtonState::Idle
        }
    }
}

pub struct RawInputData {
    pub character_queue: Vec<char>,
    mouse_btns: ButtonGroup<MouseButton>,
    keyboard_btns: ButtonGroup<VirtualKeyCode>,
    mouse_delta: Vec2,
    window_size: Vec2,
}

pub use glutin::event::MouseButton;
pub use glutin::event::VirtualKeyCode;
use specs::shrev::{EventChannel, ReaderId};
use specs::{Read, ReadExpect, System, WriteExpect};

impl RawInputData {
    pub fn new() -> RawInputData {
        RawInputData {
            mouse_btns: ButtonGroup::new(),
            keyboard_btns: ButtonGroup::new(),
            character_queue: vec![],
            mouse_delta: Vec2::zero(),
            window_size: Vec2::zero(),
        }
    }

    pub fn on_window_event(&mut self, event: &glt::event::WindowEvent) {
        match event {
            glt::WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_btns.on_element_state(*button, *state);
            }
            glt::WindowEvent::KeyboardInput {
                input:
                    glt::KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                self.keyboard_btns.on_element_state(*keycode, *state);
            }
            glt::WindowEvent::ReceivedCharacter(ch) => self.character_queue.push(*ch),
            glt::WindowEvent::Resized(_size) => {}
            _ => (),
        }
    }

    pub fn on_device_event(&mut self, event: &glt::DeviceEvent) {
        // TODO: 这里是因为 window_size 延迟初始化了
        if self.window_size[0] <= 1.0 || self.window_size[1] <= 1.0 {
            return;
        }

        match event {
            glutin::DeviceEvent::Motion { axis, value } => {
                let dx = if *axis == 0 { *value as f32 } else { 0.0 };
                let dy = if *axis == 1 { *value as f32 } else { 0.0 };

                let dx = dx / self.window_size[0];
                let dy = dy / self.window_size[0];

                self.mouse_delta = Vec2::new(self.mouse_delta.x + dx, self.mouse_delta.y + dy);
            }
            _ => (),
        }
    }

    pub fn flush(&mut self, width: f32, height: f32) {
        self.keyboard_btns.on_flush();
        self.mouse_btns.on_flush();
        self.character_queue.clear();
        self.mouse_delta = Vec2::zero();

        self.window_size = Vec2::new(width, height);
    }

    pub fn get_key_state(&self, key_code: VirtualKeyCode) -> ButtonState {
        return self.keyboard_btns.get_state(key_code);
    }

    #[allow(dead_code)]
    pub fn get_mouse_state(&self, button: MouseButton) -> ButtonState {
        return self.mouse_btns.get_state(button);
    }

    pub fn get_mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }
}

struct InputSystem {
    event_reader: Option<ReaderId<Event>>,
}

impl InputSystem {
    pub fn new() -> Self {
        InputSystem { event_reader: None }
    }
}

impl<'a> System<'a> for InputSystem {
    type SystemData = (
        Read<'a, EventChannel<Event>>,
        ReadExpect<'a, ClientInfo>,
        WriteExpect<'a, RawInputData>,
    );

    fn run(&mut self, (events, client_info, mut raw_input_data): Self::SystemData) {
        raw_input_data.flush(client_info.width as f32, client_info.height as f32);
        for event in events.read(&mut self.event_reader.as_mut().expect("")) {
            match &event {
                &event::Event::WindowEvent { ref event, .. } => {
                    if client_info.is_focused {
                        raw_input_data.on_window_event(&event);
                    }
                }
                &event::Event::DeviceEvent {
                    device_id: _,
                    ref event,
                } => {
                    if client_info.is_focused {
                        raw_input_data.on_device_event(&event);
                    }
                }
                _ => (),
            }
        }
    }

    fn setup(&mut self, res: &mut World) {
        res.insert(RawInputData::new());
        Self::SystemData::setup(res);
        self.event_reader = Some(res.fetch_mut::<EventChannel<Event>>().register_reader());
    }
}

use crate::game_loop::Module;
pub struct InputModule;

impl InputModule {
    pub fn new() -> Self {
        InputModule
    }
}

impl Module for InputModule {
    fn build(&mut self, init_data: &mut crate::InitData) {
        use crate::InsertInfo;
        init_data.dispatch(InsertInfo::new("input"), |f| f.insert(InputSystem::new()));
    }

    fn on_start(&self, _start_data: &mut crate::StartData) {}
}
