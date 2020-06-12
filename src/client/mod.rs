pub mod debug_ui;
pub mod graph;
pub mod window;

use crate::game_loop::Module;
use glium::Display;
use std::cell::RefCell;
use std::rc::Rc;

pub type Event = glutin::event::Event<'static, ()>;
pub type EventLoop = glutin::event_loop::EventLoop<()>;

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
    events_loop: Option<EventLoop>,
}

impl ClientModule {
    pub fn new(title: String) -> Self {
        let (display, event_loop) = {
            let event_loop = EventLoop::new();
            let wb = glutin::window::WindowBuilder::new().with_title(title.clone());
            let cb = glutin::ContextBuilder::new()
                //            .with_vsync(true)
                .with_srgb(true);
            (
                Rc::new(glium::Display::new(wb, cb, &event_loop).unwrap()),
                event_loop,
            )
        };

        global_display.with(|r| *r.borrow_mut() = Some(display.clone()));

        ClientModule {
            display,
            events_loop: Some(event_loop),
        }
    }
}

impl Module for ClientModule {
    fn get_submodules(&mut self) -> Vec<Box<dyn Module>> {
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
