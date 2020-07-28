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

#[derive(Copy, Clone, Debug)]
pub enum AlignType {
    Left, Middle, Right
}

impl AlignType {

    fn ratio(&self) -> f32 {
        match &self {
            AlignType::Left => 0.0,
            AlignType::Middle => 0.5,
            AlignType::Right => 1.0
        }
    }

}

#[derive(Copy, Clone, Debug)]
pub enum LayoutType {
    Expand{ off_n: f32, off_p: f32 },
    Normal{ align: AlignType, pos: f32, len: f32 }
}

// #[derive(Default)]
struct WidgetRuntimeInfo {
    /// Whether widget rect needs to be recalculated.
    dirty: bool,
    /// Rect in local space.
    local_rect: Rect,
    /// Matrix to transform vertex from local space to screen space.
    wvp: Mat3,
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

impl UILayoutSystem {

    fn _calc_layout(parent_length: f32, layout: LayoutType) -> (f32, f32) {
        match layout {
            LayoutType::Normal { align, pos, len } => {
                let pivot_pos = align.ratio() * parent_length;
                (pivot_pos + pos, len)
            },
            LayoutType::Expand { off_n, off_p } => {
                let parent_end = parent_length;
                let self_start = off_n;
                let self_end = parent_end - off_p;
                (self_start, self_end - self_start)
            },
        }
    }
}

struct WidgetFrame {
    wvp: Mat3,
    rect: Rect
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

        let update_widget_layout = |frame: WidgetFrame, entity: Entity, dirty: bool| {
            let frame = {
                let widget = widget_vec.get_mut(entity).unwrap();
                if dirty || widget.runtime_info.dirty {
                    let (x, width) = Self::_calc_layout(frame.rect.width, widget.layout_x);
                    let (y, height) = Self::_calc_layout(frame.rect.height, widget.layout_y);

                    widget.runtime_info.local_rect = Rect::new(x, y, width, height);
                    // widget.runtime_info.wvp = frame.wvp * Mat3::tran
                }

                WidgetFrame {
                    wvp: widget.runtime_info.wvp,
                    rect: widget.runtime_info.local_rect
                }
            };
        };

        for (ent, canvas) in all_canvas {
            for child in hierarchy.children(ent) {
                if let Some(widget) = widget_vec.get_mut(*child) {
                }
            }
        }
    }
}
