pub mod debug_ui;
pub mod graph;
pub mod window;

use crate::game_loop::Module;
use glium::Display;
use glutin::EventsLoop;
use std::cell::RefCell;
use std::rc::Rc;

thread_local!(
static global_display: RefCell<Option<Rc<Display>>> = RefCell::new(None)
);

pub fn get_display() -> Rc<Display> {
    global_display.with(|r| r.borrow().clone().expect("Global display is not created!"))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ClientInfo {
    pub width: u32,
    pub height: u32,
    pub is_focused: bool,
}

pub struct ClientModule {
    display: Rc<Display>,
    events_loop: Option<EventsLoop>,
}

impl ClientModule {
    pub fn new(title: String) -> Self {
        let (display, events_loop) = {
            let events_loop = glutin::EventsLoop::new();
            let wb = glutin::WindowBuilder::new().with_title(title.clone());
            let cb = glutin::ContextBuilder::new()
                //            .with_vsync(true)
                .with_srgb(true);
            (
                Rc::new(glium::Display::new(wb, cb, &events_loop).unwrap()),
                events_loop,
            )
        };

        global_display.with(|r| *r.borrow_mut() = Some(display.clone()));

        ClientModule {
            display,
            events_loop: Some(events_loop),
        }
    }
}

impl Module for ClientModule {
    fn get_submodules(&mut self) -> Vec<Box<Module>> {
        vec![
            Box::new(graph::GraphModule::new(self.display.clone())),
            Box::new(window::WindowModule::new(
                self.display.clone(),
                self.events_loop.take().unwrap(),
            )),
            Box::new(debug_ui::DebugUIModule::new(self.display.clone())),
        ]
    }
}
