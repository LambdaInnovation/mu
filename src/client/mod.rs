use winit::event;

pub mod graphics;
pub mod input;
pub mod sprite;
pub mod editor;
pub mod ui;
pub mod text;

/// A specs `Resource`. contains information about window.
pub struct WindowInfo {
    pub frame_event_list: Vec<event::Event<'static, ()>>,
    pub grab_cursor: bool,
    pub show_cursor: bool,
    pub pixel_size: (u32, u32),
    // dpi, cursor state, etc...
    // platform independent?

    pub(crate) last_grab_cursor: bool,
    pub(crate) last_show_cursor: bool,
}

impl WindowInfo {

    pub fn new() -> Self {
        Self {
            frame_event_list: vec![],
            pixel_size: (128, 128),
            grab_cursor: false,
            show_cursor: true,
            last_grab_cursor: false,
            last_show_cursor: true
        }
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        let (x, y) = self.pixel_size;
        (x as f32) / (y as f32)
    }

}