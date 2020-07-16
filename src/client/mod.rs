use glutin;
use glutin::event;

pub mod graphics;
pub mod input;
pub mod editor;

pub struct WindowInfo {
    pub frame_event_list: Vec<event::Event<'static, ()>>,
    pub grab_cursor_count: u32,
    // dpi, cursor state, etc...
    // platform independent?
}

impl WindowInfo {

    pub fn new() -> Self {
        Self {
            frame_event_list: vec![],
            grab_cursor_count: 0
        }
    }

}