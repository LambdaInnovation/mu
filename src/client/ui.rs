use specs::{Component, VecStorage, System, ReadExpect, ReadStorage, WriteStorage, Entities, Join, Entity};
use crate::math::*;
use specs_hierarchy::Hierarchy;
use crate::ecs::HasParent;
use std::cmp::Ordering;
use crate::client::WindowInfo;

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

impl Canvas {

    /// Actual size depending on the screen.
    fn actual_size(&self, info: &WindowInfo) -> (f32, f32) {
        let (scr_w, scr_h) = info.pixel_size;
        let (scr_w, scr_h) = (scr_w as f32, scr_h as f32);
        let scl_w = (self.ref_resolution.width as f32) / scr_w;
        let scl_h = (self.ref_resolution.height as f32) / scr_h;

        let scl = lerp(scl_w, scl_h, self.ref_resolution.scale_dimension);
        return (scr_w * scl, scr_h * scl)
    }

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

struct WidgetFrame {
    wvp: Mat3,
    rect: Rect
}

struct WidgetRecurseContext<'a, 'b> {
    entities: &'a Entities<'b>,
    hierarchy: &'a Hierarchy<HasParent>,
    widget_vec: &'a mut WriteStorage<'b, Widget>
}

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

fn _update_widget_layout(
    ctx: &mut WidgetRecurseContext,
    frame: &WidgetFrame, entity: Entity, dirty: bool) {

    let (self_frame, self_dirty) = {
        let widget = ctx.widget_vec.get_mut(entity).unwrap();

        let d = if dirty || widget.runtime_info.dirty {
            let (x, width) = _calc_layout(frame.rect.width,
                                                widget.layout_x);
            let (y, height) = _calc_layout(frame.rect.height,
                                                 widget.layout_y);

            widget.runtime_info.local_rect = Rect::new(x, y, width, height);
            // widget.runtime_info.wvp = frame.wvp * Mat3::tran
            true
        } else {
            false
        };

        (WidgetFrame {
            wvp: widget.runtime_info.wvp,
            rect: widget.runtime_info.local_rect
        }, d)
    };

    for child in ctx.hierarchy.children(entity).iter().map(|x| x.clone()) {
        _update_widget_layout(ctx, &self_frame, child, dirty || self_dirty);
    }
}

impl<'a> System<'a> for UILayoutSystem {

    type SystemData = (
        Entities<'a>,
        ReadExpect<'a, Hierarchy<HasParent>>,
        ReadStorage<'a, Canvas>,
        WriteStorage<'a, Widget>,
        ReadExpect<'a, WindowInfo>);

    fn run(&mut self, (entities, hierarchy, canvas_vec, mut widget_vec, window_info): Self::SystemData) {
        let mut all_canvas: Vec<(Entity, &Canvas)> = (&*entities, &canvas_vec).join().collect();
        all_canvas.sort_by_key(|x| x.1.order);

        let update_widget_layout = |frame: WidgetFrame, entity: Entity, dirty: bool| {
            let frame = {
                let widget = widget_vec.get_mut(entity).unwrap();
                if dirty || widget.runtime_info.dirty {
                    let (x, width) = _calc_layout(frame.rect.width,
                                                        widget.layout_x);
                    let (y, height) = _calc_layout(frame.rect.height,
                                                         widget.layout_y);

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
            let size = canvas.actual_size(&*window_info);

            let frame = WidgetFrame {
                wvp: mat3::translate(Vec2::new(0., 0.)), // TODO
                rect: Rect::new(0., 0., size.0, size.1)
            };

            let mut rec_ctx = WidgetRecurseContext {
                entities: &entities,
                hierarchy: &*hierarchy,
                widget_vec: &mut widget_vec
            };

            for child in hierarchy.children(ent) {
                // FIXME: canvas的dirty 取决于window是否resize
                _update_widget_layout(&mut rec_ctx, &frame, *child, false);
            }
        }
    }
}
