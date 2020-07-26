use specs::{Component, VecStorage};
use crate::math::Vec2;

pub enum LayoutType {
    Expand{ off_p: f32, off_n: f32 },
    Normal{ pos: f32, len: f32 }
}

pub struct Widget {
    scl: Vec2,
    pivot: Vec2,
    layout_x: LayoutType,
    layout_y: LayoutType
}

impl Component for Widget {
    type Storage = VecStorage<Self>;
}