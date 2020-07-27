use specs::{Component, VecStorage, System, ReadExpect, ReadStorage, WriteStorage, Entities, Join, Entity};
use crate::math::*;
use specs_hierarchy::Hierarchy;
use crate::ecs::HasParent;
use std::cmp::Ordering;

// UI axis: x+ right; y+ up

pub struct RefResolution {
    pub width: u32,
    pub height: u32,
    pub scale_dimension: f32,
}

pub struct Canvas {
    order: i32,
    ref_resolution: RefResolution
}

impl Component for Canvas {
    type Storage = VecStorage<Self>;
}

pub enum LayoutType {
    Expand{ off_n: f32, off_p: f32 },
    Normal{ pos: f32, len: f32 }
}

#[derive(Default)]
struct WidgetRuntimeInfo {
    /// Whether widget rect needs to be recalculated.
    dirty: bool,
    /// Rect in canvas space.
    canvas_rect: Rect,
}

pub struct Widget {
    scl: Vec2,
    pivot: Vec2,
    layout_x: LayoutType,
    layout_y: LayoutType,
    runtime_info: WidgetRuntimeInfo
}

impl Component for Widget {
    type Storage = VecStorage<Self>;
}

/// 触发Widget更新的时机:
///
/// 1. Widget第一帧显示前
/// 2. 自己身上有layout, 那么需要在child object update的时候更新自己 (future todo)
/// 3. 父层级或自己的layout属性被修改，那么需要自上而下重新计算
///
/// 总体的机制是 widget.runtime_info.dirty = true, 然后 UISystem 自身来计算


/// UI layout update -> UI control update
pub struct UILayoutSystem {
}

impl<'a> System<'a> for UILayoutSystem {
    type SystemData = (
        Entities<'a>,
        ReadExpect<'a, Hierarchy<HasParent>>,
        ReadStorage<'a, Canvas>,
        WriteStorage<'a, Widget>);

    fn run(&mut self, (entities, hierarchy, canvas_vec, mut widget_vec): Self::SystemData) {
        let mut all_canvas: Vec<(Entity, &Canvas)> = (&*entities, &canvas_vec).join().collect();
        all_canvas.sort_by_key(|x| x.1.order);

        for (ent, canvas) in all_canvas {
            // TODO
        }
    }
}
