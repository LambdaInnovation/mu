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
use glium;
use glium::{Frame, Program, Display, VertexBuffer, IndexBuffer, Surface};
use crate::asset::ResourceRef;
use std::rc::Rc;
use glium::index::PrimitiveType;
use crate::client::graphics::{Material, Texture, UniformMat4};
use crate::client::input::RawInputData;

// UI axis: x+ right; y+ up

pub struct RefResolution {
    pub width: u32,
    pub height: u32,
    pub scale_dimension: f32,
}

impl RefResolution {
    pub fn new(width: u32, height: u32, scale_dimension: f32) -> Self {
        RefResolution { width, height, scale_dimension }
    }
}

pub struct Canvas {
    order: i32,
    ref_resolution: RefResolution,
    batcher: UIBatcher
}

impl Component for Canvas {
    type Storage = VecStorage<Self>;
}

impl Canvas {

    pub fn new(order: i32, ref_resolution: RefResolution) -> Self {
        Canvas { order, ref_resolution, batcher: UIBatcher::new() }
    }

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
    Min, Middle, Max
}

impl AlignType {

    fn ratio(&self) -> f32 {
        match &self {
            AlignType::Min => 0.0,
            AlignType::Middle => 0.5,
            AlignType::Max => 1.0
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
    /// widget在canvas里的绘制顺序
    draw_idx: u32,
}

impl WidgetRuntimeInfo {

    pub fn new() -> Self {
        Self {
            dirty: true,
            local_rect: Rect::new_origin(0., 0.),
            wvp: Mat3::one(),
            draw_idx: 0
        }
    }

}

pub struct Widget {
    pub scl: Vec2,
    pub pivot: Vec2,
    pub rot: f32,
    pub layout_x: LayoutType,
    pub layout_y: LayoutType,
    pub raycast: bool,
    runtime_info: WidgetRuntimeInfo,
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
            runtime_info: WidgetRuntimeInfo::new(),
            raycast: false
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

    pub fn with_pivot(mut self, p: Vec2) -> Self {
        self.pivot = p;

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
pub struct UIUpdateSystem {

}

struct WidgetFrame {
    wvp: Mat3,
    rect: Rect
}

struct WidgetRecurseContextMut<'a, 'b> {
    entities: &'a Entities<'b>,
    hierarchy: &'a Hierarchy<HasParent>,
    widget_vec: &'a mut WriteStorage<'b, Widget>,
    cur_widget_draw_idx: u32
}

struct ImageBatchContext<'a, 'b> {
    entities: &'a Entities<'b>,
    hierarchy: &'a Hierarchy<HasParent>,
    widget_vec: &'a ReadStorage<'b, Widget>,
    image_read: &'a ReadStorage<'b, Image>,
    batcher: &'a mut UIBatcher
}

impl<'a, 'b> ImageBatchContext<'a, 'b> {


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
    let translation_mat = mat3::translate(vec2(rect.x, rect.y));
    // TODO: 支持scl和rot
    translation_mat
}

fn _update_widget_layout(
    ctx: &mut WidgetRecurseContextMut,
    frame: &WidgetFrame, entity: Entity, dirty: bool) {

    let (self_frame, self_dirty) = {
        let widget = ctx.widget_vec.get_mut(entity).unwrap();

        widget.runtime_info.draw_idx = ctx.cur_widget_draw_idx;
        ctx.cur_widget_draw_idx += 1;

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

impl<'a> System<'a> for UIUpdateSystem {

    type SystemData = (
        Entities<'a>,
        ReadExpect<'a, Hierarchy<HasParent>>,
        ReadStorage<'a, Canvas>,
        WriteStorage<'a, Widget>,
        ReadExpect<'a, WindowInfo>,
        ReadExpect<'a, RawInputData>);

    fn run(&mut self, (entities, hierarchy, canvas_vec, mut widget_vec, window_info, input): Self::SystemData) {
        let mut all_canvas: Vec<(Entity, &Canvas)> = (&*entities, &canvas_vec).join().collect();
        all_canvas.sort_by_key(|x| x.1.order);

        for (ent, canvas) in all_canvas {
            let (width, height) = canvas.actual_size(&*window_info);

            let frame = WidgetFrame {
                wvp: mat3::ortho(0., width, 0., height), // Map (0,0)->(width,height) to NDC
                rect: Rect::new(0., 0., width, height)
            };

            let mut rec_ctx = WidgetRecurseContextMut {
                entities: &entities,
                hierarchy: &*hierarchy,
                widget_vec: &mut widget_vec,
                cur_widget_draw_idx: 0
            };

            for child in hierarchy.children(ent) {
                // FIXME: canvas的dirty 取决于window是否resize
                _update_widget_layout(&mut rec_ctx, &frame, *child, false);
            }
        }

        // info!("cursor pos: {:?}", input.cursor_position);
    }
}

/// An UI image. Size goes with Widget size.
pub struct Image {
    pub sprite: Option<SpriteRef>,
    pub material: Option<ResourceRef<Material>>,
    pub color: Color
}

impl Image {
    pub fn new() -> Self {
        Image { sprite: None, material: None, color: Color::white() }
    }
}

impl Component for Image {
    type Storage = VecStorage<Self>;
}

// UI drawing: 需要每个canvas顺序绘制 所以实际的绘制顺序应该是
// Canvas1-S1S2S3... Canvas2-S1S2S3...
// 和系统的执行顺序存在交错
// 这个先不处理了 等到wgpu-rs切换的时候更好处理

#[derive(Copy, Clone)]
struct ImageVertex {
    v_pos: [f32; 2],
    v_uv: [f32; 2],
}

impl ImageVertex {
    fn new(x: f32, y: f32, u: f32, v: f32) -> Self {
        Self {
            v_pos: [x, y],
            v_uv: [u, v]
        }
    }
}

glium::implement_vertex!(ImageVertex, v_pos, v_uv);

#[derive(Copy, Clone, Default)]
struct ImageInstanceData {
    i_wvp: [[f32; 4]; 4],
    i_uv_min: [f32; 2],
    i_uv_max: [f32; 2],
    i_color: [f32; 4]
}

glium::implement_vertex!(ImageInstanceData, i_wvp, i_uv_min, i_uv_max, i_color);

struct UIImageBatchSystem {
}

impl UIImageBatchSystem {

    fn _walk<'a>(ctx: &mut ImageBatchContext, entity: Entity) {
        if let Some(image) = ctx.image_read.get(entity) {
            let widget = ctx.widget_vec.get(entity).unwrap();
            // 这里再乘一个 size 把 [0,1] 的顶点坐标缩放
            let wvp = widget.runtime_info.wvp * mat3::scale(widget.runtime_info.local_rect.size());
            let final_wvp = mat3::extend_to_mat4(&wvp);

            ctx.batcher.batch(widget.runtime_info.draw_idx, DrawInstance::Image {
                sprite: image.sprite.clone(),
                material: image.material.clone(),
                color: image.color,
                wvp: final_wvp
            });
        }

        for child in ctx.hierarchy.children(entity) {
            Self::_walk(ctx, child.clone());
        }
    }
}

impl<'a> System<'a> for UIImageBatchSystem {
    type SystemData = (
        WriteStorage<'a, Canvas>,
        Entities<'a>, ReadExpect<'a, Hierarchy<HasParent>>,
        ReadStorage<'a, Widget>,
        ReadStorage<'a, Image>);

    fn run(&mut self, (mut canvas, entities, hierarchy, widget_storage, image_storage): Self::SystemData) {
        for (ent, canvas) in (&entities, &mut canvas).join() {
            let mut ctx = ImageBatchContext {
                entities: &entities,
                hierarchy: &hierarchy,
                widget_vec: &widget_storage,
                image_read: &image_storage,
                batcher: &mut canvas.batcher
            };

            for child in ctx.hierarchy.children(ent) {
                Self::_walk(&mut ctx, child.clone());
            }
        }
    }

}

enum DrawInstance {
    Image {
        wvp: Mat4,
        sprite: Option<SpriteRef>,
        material: Option<ResourceRef<Material>>,
        color: Color
    },
    Text {  }
}

struct UIBatcher {
    ls: Vec<(u32, DrawInstance)>
}

impl UIBatcher {

    fn new() -> Self {
        Self { ls: vec![] }
    }

    fn batch(&mut self, id: u32, instance: DrawInstance) {
        self.ls.push((id, instance));
    }

    fn finish(&mut self) -> Vec<DrawInstance> {
        let mut result = Vec::<(u32, DrawInstance)>::new();
        std::mem::swap(&mut result, &mut self.ls);

        result.sort_by_key(|(id, _)| *id);
        result.into_iter().map(|x| x.1).collect()
    }

}

pub struct UIModule;

struct UIImageRenderData {
    default_program: ResourceRef<Program>,
    vbo: VertexBuffer<ImageVertex>,
    ibo: IndexBuffer<u16>,
    white_texture: ResourceRef<Texture>
}

impl UIImageRenderData {
    
    fn new(display: &Display) -> Self {
        use crate::client::graphics;
        let program = graphics::load_shader_by_content(display,
                                                       include_str!("../../assets/ui_image_default.vert"),
                                                       include_str!("../../assets/ui_image_default.frag"));

        let vbo = VertexBuffer::new(display, &[
            ImageVertex::new(0., 0., 0., 0.),
            ImageVertex::new(0., 1., 0., 1.),
            ImageVertex::new(1., 1., 1., 1.),
            ImageVertex::new(1., 0., 1., 0.)
        ]).unwrap();

        let ibo = IndexBuffer::new(
            display,
            PrimitiveType::TrianglesList,
            &[0u16, 1, 2, 0, 2, 3]).unwrap();
        
        let white_texture = graphics::create_texture(display, vec![255, 255, 255, 255], (1, 1));
        
        Self {
            default_program: program,
            vbo, ibo,
            white_texture
        }
    }
}

struct UIRenderSystem {
    image_data: UIImageRenderData,
    display: Rc<Display>
}

impl UIRenderSystem {

    fn new(display: Rc<Display>) -> Self {
        Self {
            image_data: UIImageRenderData::new(&display),
            display
        }
    }

}

impl<'a> System<'a> for UIRenderSystem {
    type SystemData = WriteStorage<'a, Canvas>;

    fn run(&mut self, mut data: Self::SystemData) {
        use crate::client::graphics;
        use crate::asset;

        graphics::with_render_data(|f| {
            asset::with_local_resource_mgr(|res_mgr| {
                let image_data = &self.image_data;
                let img_default_program = res_mgr.get(&image_data.default_program);
                let img_white_texture = res_mgr.get(&self.image_data.white_texture);
                for canvas in (&mut data).join() {
                    let draw_calls = canvas.batcher.finish();
                    for draw in draw_calls {
                        match draw {
                            DrawInstance::Image {
                                wvp, sprite, material, color
                            } => {
                                let program = img_default_program;
                                let (texture, uv0, uv1) = match &sprite {
                                    Some(sr) => {
                                        let sheet = res_mgr.get(&sr.sheet);
                                        let texture = res_mgr.get(&sheet.texture);
                                        let spr_data = &sheet.sprites[sr.idx];
                                        (texture, spr_data.uv_min, spr_data.uv_max)
                                    }
                                    None => {
                                        (img_white_texture, vec2(0., 0.), vec2(1., 1.))
                                    }
                                };
                                let uniform_block = glium::uniform! {
                                    u_texture: &texture.raw_texture
                                };

                                let instance_buf = VertexBuffer::new(&*self.display, &[ImageInstanceData {
                                    i_wvp: wvp.into(),
                                    i_uv_min: [uv0.x, uv0.y],
                                    i_uv_max: [uv1.x, uv1.y],
                                    i_color: color.into()
                                }]).unwrap();

                                f.frame.draw((&image_data.vbo, instance_buf.per_instance().unwrap()),
                                             &image_data.ibo,
                                             program, &uniform_block,
                                             &Default::default()).unwrap();
                            },
                            _ => unimplemented!()
                        }
                    }
                }
            });
        });
    }
}

impl Module for UIModule {
    fn init(&self, init_data: &mut InitData) {
        use super::graphics;
        // 这个其实不用insert到thread local，但是执行依赖关系不好处理
        init_data.group_thread_local.dispatch(
            InsertInfo::new("ui_layout").before(&[&graphics::DEP_RENDER_SETUP]),
            |i| i.insert_thread_local(UIUpdateSystem {})
        );

        init_data.group_thread_local.dispatch(
            InsertInfo::new("ui_images").after(&["ui_layout"]).before(&["ui_render"]),
            |i| i.insert_thread_local(UIImageBatchSystem {})
        );

        let display_rc = init_data.display.clone();
        init_data.group_thread_local.dispatch(
            InsertInfo::new("ui_render")
                .after(&[graphics::DEP_RENDER_SETUP]).before(&[graphics::DEP_RENDER_TEARDOWN])
                .order(graphics::render_order::UI),
            |i| i.insert_thread_local(UIRenderSystem::new(display_rc))
        );
    }
}

#[cfg(test)]
mod test {
    use specs::{World, WorldExt, DispatcherBuilder, Builder};
    use crate::client::ui::{UIUpdateSystem, Canvas, RefResolution, Widget, LayoutType, AlignType, UIBatcher};
    use crate::client::WindowInfo;
    use specs_hierarchy::HierarchySystem;
    use crate::ecs::HasParent;
    use crate::math::Rect;

    #[test]
    fn layout_simple() {
        let mut world = World::new();

        let mut dispatcher = DispatcherBuilder::new()
            .with(HierarchySystem::<HasParent>::new(&mut world), "", &[])
            .with(UIUpdateSystem {}, "", &[])
            .build();

        dispatcher.setup(&mut world);

        let canvas_ent = world.create_entity()
            .with(Canvas {
                order: 0,
                ref_resolution: RefResolution {
                    width: 1920,
                    height: 1080,
                    scale_dimension: 0.5
                },
                batcher: UIBatcher::new()
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