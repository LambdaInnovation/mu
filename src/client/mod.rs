
use glutin;
use glutin::event;

pub mod graphics;
pub mod input;
pub mod editor;
pub mod sprite;

pub struct WindowInfo {
    pub frame_event_list: Vec<event::Event<'static, ()>>,
    pub grab_cursor_count: u32,
    pub pixel_size: (u32, u32),
    // dpi, cursor state, etc...
    // platform independent?
}

impl WindowInfo {

    pub fn new() -> Self {
        Self {
            frame_event_list: vec![],
            grab_cursor_count: 0,
            pixel_size: (128, 128)
        }
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        let (x, y) = self.pixel_size;
        (x as f32) / (y as f32)
    }

}