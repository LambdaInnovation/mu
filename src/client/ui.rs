use specs::{Component, VecStorage, System, ReadExpect, ReadStorage, WriteStorage, Entities, Join, Entity};
use crate::math::*;
use specs_hierarchy::Hierarchy;
use crate::ecs::HasParent;
use std::cmp::Ordering;
use core::fmt::Alignment::Center;
use crate::client::WindowInfo;
use crate::client::sprite::SpriteRef;
use crate::util::Color;
use crate::{Module, InitData, InsertInfo};

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

impl LayoutType {

    pub fn expand(n: f32, p: f32) -> Self {
        LayoutType::Expand { off_n: n, off_p: p }
    }

    pub fn normal(align: AlignType, pos: f32, len: f32) -> Self {
        LayoutType::Normal { align, pos, len }
    }

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

impl WidgetRuntimeInfo {

    pub fn new() -> Self {
        Self {
            dirty: true,
            local_rect: Rect::new_origin(0., 0.),
            wvp: Mat3::one()
        }
    }

}

pub struct Widget {
    scl: Vec2,
    pivot: Vec2,
    rot: f32,
    layout_x: LayoutType,
    layout_y: LayoutType,
    runtime_info: WidgetRuntimeInfo
}

impl Component for Widget {
    type Storage = VecStorage<Self>;
}

impl Widget {

    pub fn new() -> Self {
        Widget {
            scl: vec2(1., 1.),
            pivot: vec2(0.5, 0.5),
            rot: 0.,
            layout_x: LayoutType::Normal { align: AlignType::Middle, pos: 0.0, len: 100. },
            layout_y: LayoutType::Normal { align: AlignType::Middle, pos: 0.0, len: 100. },
            runtime_info: WidgetRuntimeInfo::new()
        }
    }

    fn _mark_dirty(&mut self) {
        self.runtime_info.dirty = true;
    }

    pub fn with_layout_x(mut self, layout: LayoutType) -> Self {
        self.layout_x = layout;
        self._mark_dirty();
        self
    }

    pub fn with_layout_y(mut self, layout: LayoutType) -> Self {
        self.layout_y = layout;
        self._mark_dirty();
        self
    }

    // pub fn runtime_info(&self) -> &WidgetRuntimeInfo {
    //     &self.runtime_info
    // }

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

fn _calc_layout(parent_length: f32, layout: LayoutType, pivot: f32) -> (f32, f32) {
    match layout {
        LayoutType::Normal { align, pos, len } => {
            let pivot_pos = align.ratio() * parent_length;
            (pivot_pos + pos - len * pivot, len)
        },
        LayoutType::Expand { off_n, off_p } => {
            let parent_end = parent_length;
            let self_start = off_n;
            let self_end = parent_end - off_p;
            (self_start, self_end - self_start)
        },
    }
}

fn calc_widget_mat(rect: &Rect, scl: Vec2, rot: f32) -> Mat3 {
    let translation_mat = mat3::translate(-vec2(rect.x, rect.y));
    // TODO: 支持scl和rot

    translation_mat
}

fn _update_widget_layout(
    ctx: &mut WidgetRecurseContext,
    frame: &WidgetFrame, entity: Entity, dirty: bool) {

    let (self_frame, self_dirty) = {
        let widget = ctx.widget_vec.get_mut(entity).unwrap();

        let d = if dirty || widget.runtime_info.dirty {
            let (x, width) = _calc_layout(frame.rect.width,
                                                widget.layout_x, widget.pivot.x);
            let (y, height) = _calc_layout(frame.rect.height,
                                                 widget.layout_y, widget.pivot.y);

            widget.runtime_info.local_rect = Rect::new(x, y, width, height);
            widget.runtime_info.wvp = frame.wvp * calc_widget_mat(&widget.runtime_info.local_rect,
                widget.scl, widget.rot);
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

        for (ent, canvas) in all_canvas {
            let (width, height) = canvas.actual_size(&*window_info);

            let frame = WidgetFrame {
                wvp: mat3::ortho(0., width, 0., height), // Map (0,0)->(width,height) to NDC
                rect: Rect::new(0., 0., width, height)
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

/// An UI image. Size goes with Widget size.
pub struct Image {
    pub sprite: Option<SpriteRef>,
    pub color: Color
}

// UI drawing: 需要每个canvas顺序绘制 所以实际的绘制顺序应该是
// Canvas1-S1S2S3... Canvas2-S1S2S3...
// 和系统的执行顺序存在交错
// 这个先不处理了 等到wgpu-rs切换的时候更好处理

struct UIImageRenderSystem {

}

impl UIImageRenderSystem {

    pub fn new() -> Self {
        Self {}
    }
}

impl<'a> System<'a> for UIImageRenderSystem {
    type SystemData = (ReadStorage<'a, Canvas>, ReadExpect<'a, Hierarchy<HasParent>>, ReadStorage<'a, Widget>);

    fn run(&mut self, data: Self::SystemData) {
        use super::graphics;
        graphics::with_render_data(|render_data| {
            let mut frame = &mut render_data.frame;
        });
    }
}

pub struct UIModule;

impl Module for UIModule {
    fn init(&self, init_data: &mut InitData) {
        use super::graphics;
        // 这个其实不用insert到thread local，但是执行依赖关系不好处理
        init_data.group_thread_local.dispatch(
            InsertInfo::new("ui_layout").before(&[&graphics::DEP_RENDER_SETUP]),
            |i| i.insert_thread_local(UILayoutSystem {})
        );

        init_data.group_thread_local.dispatch(
            InsertInfo::new("ui_images").after(&[&graphics::DEP_RENDER_SETUP]).before(&[&graphics::DEP_RENDER_TEARDOWN]),
            |i| i.insert_thread_local(UIImageRenderSystem::new())
        );
    }
}

#[cfg(test)]
mod test {
    use specs::{World, WorldExt, DispatcherBuilder, Builder};
    use crate::client::ui::{UILayoutSystem, Canvas, RefResolution, Widget, LayoutType, AlignType};
    use crate::client::WindowInfo;
    use specs_hierarchy::HierarchySystem;
    use crate::ecs::HasParent;
    use crate::math::Rect;

    #[test]
    fn layout_simple() {
        let mut world = World::new();

        let mut dispatcher = DispatcherBuilder::new()
            .with(HierarchySystem::<HasParent>::new(&mut world), "", &[])
            .with(UILayoutSystem {}, "", &[])
            .build();

        dispatcher.setup(&mut world);

        let canvas_ent = world.create_entity()
            .with(Canvas {
                order: 0,
                ref_resolution: RefResolution {
                    width: 1920,
                    height: 1080,
                    scale_dimension: 0.5
                }
            })
            .build();

        let w0 = world.create_entity()
            .with(HasParent::new(canvas_ent))
            .with(Widget::new()
                .with_layout_x(LayoutType::expand(0., 0.))
                .with_layout_y(LayoutType::expand(0., 0.)))
            .build();

        let w1 = world.create_entity()
            .with(HasParent::new(w0))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 50., 100.))
                .with_layout_y(LayoutType::normal(AlignType::Middle, -50., 100.)))
            .build();

        let mut window_info = WindowInfo::new();
        window_info.pixel_size = (1280, 720);
        world.insert(window_info);

        dispatcher.dispatch(&world);
        world.maintain();

        {
            let widget_storage = world.read_storage::<Widget>();
            if let Some(w) = widget_storage.get(w0) {
                assert!(Rect::approx_eq(&w.runtime_info.local_rect,
                                        &Rect::new_origin(1920., 1080.)));
            } else {
                panic!();
            }

            if let Some(w) = widget_storage.get(w1) {
                assert!(Rect::approx_eq(&w.runtime_info.local_rect,
                &Rect::new(960., 440., 100., 100.)))
            } else {
                panic!();
            }
        }

        println!("UI layout OK!");
    }

}