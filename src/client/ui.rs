use std::rc::Rc;

use cgmath::SquareMatrix;
use specs::prelude::*;
use specs_hierarchy::Hierarchy;

use crate::*;
use crate::client::graphics::{Material, Texture};
use crate::client::input::RawInputData;
use crate::client::sprite::SpriteRef;
use crate::client::WindowInfo;
use crate::ecs::HasParent;
use crate::math::*;
use crate::util::Color;
use crate::resource::ResourceRef;

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
    batcher: internal::UIBatcher
}

impl Component for Canvas {
    type Storage = VecStorage<Self>;
}

impl Canvas {

    pub fn new(order: i32, ref_resolution: RefResolution) -> Self {
        Canvas { order, ref_resolution, batcher: internal::UIBatcher::new() }
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

#[derive(Copy, Clone, Debug)]
enum WidgetCursorState {
    Idle,
    /// 鼠标在widget上按下 未松开 可能拖动到任意位置
    Dragging
}

pub struct Widget {
    pub scl: Vec2,
    pub pivot: Vec2,
    pub rot: f32,
    pub layout_x: LayoutType,
    pub layout_y: LayoutType,
    pub raycast: bool,
    runtime_info: internal::WidgetRuntimeInfo,
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
            runtime_info: internal::WidgetRuntimeInfo::new(),
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

    pub fn with_raycast(mut self) -> Self {
        self.raycast = true;
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

pub enum WidgetEvent {
    /// Fired in the frame widget is clicked
    Clicked { entity: Entity, btn: u8 },

    // Fired every frame widget is being dragged. delta: cursor position change in PARENT space.
    // Drag { entity: Entity, btn: u8, delta: Vec2 },

    /// Fired in the frame when widget's dragging is released.
    Release { entity: Entity, btn: u8 },
}

/// A specs `Resources`. Contains all UI events in **PREVIOUS** frame.
pub struct WidgetEvents {
    pub events: Vec<WidgetEvent>
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

pub struct UIModule;

impl Module for UIModule {
    fn init(&self, init_ctx: &mut InitContext) {
        use super::graphics;
        // 这个其实不用insert到thread local，但是执行依赖关系不好处理
        init_ctx.group_thread_local.dispatch(
            InsertInfo::new("ui_layout").before(&[graphics::DEP_CAM_DRAW_SETUP]),
            |_, i| i.insert_thread_local(internal::UIUpdateSystem {})
        );

        init_ctx.group_thread_local.dispatch(
            InsertInfo::new("ui_images").after(&["ui_layout"]).before(&["ui_render"]),
            |_, i| i.insert_thread_local(internal::UIImageBatchSystem {})
        );

        init_ctx.group_thread_local.dispatch(
            InsertInfo::new("ui_render")
                .after(&[graphics::DEP_CAM_DRAW_SETUP]).before(&[graphics::DEP_CAM_DRAW_TEARDOWN])
                .order(graphics::render_order::UI),
            |d, i| i.insert_thread_local(internal::UIRenderSystem::new(d))
        );
    }

    fn start(&self, start_data: &mut StartContext) {
        start_data.world.insert(WidgetEvents { events: vec![] });
    }
}

mod internal {
    use crate::client::input::ButtonState;
    use super::*;
    use crate::{WgpuState, WgpuStateCell};
    use crate::client::graphics::{UniformLayoutConfig, UniformBindingType, UniformVisibility, SamplerConfig, FilterMode, ShaderProgram, MatProperty};
    use crate::resource::ResManager;
    use std::collections::HashMap;

    // #[derive(Default)]
    pub struct WidgetRuntimeInfo {
        /// Whether widget rect needs to be recalculated.
        pub dirty: bool,
        /// Size in local space.
        pub size: Vec2,
        /// Matrix to transform vertex from local space to NDC.
        pub wvp: Mat3,
        /// Matrix to transform vertex from NDC to local space.
        pub wvp_inverse: Mat3,
        /// widget在canvas里的绘制顺序
        pub draw_idx: u32,
        cursor_states: [WidgetCursorState; 8]
    }

    impl WidgetRuntimeInfo {

        pub fn new() -> Self {
            Self {
                dirty: true,
                size: vec2(0., 0.),
                wvp: Mat3::one(),
                wvp_inverse: Mat3::one(),
                draw_idx: 0,
                cursor_states: [WidgetCursorState::Idle; 8]
            }
        }

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

    fn calc_widget_mat(offset: Vec2, scl: Vec2, rot: f32) -> Mat3 {
        let translation_mat = mat3::translate(offset);
        // TODO: 支持scl和rot
        translation_mat
    }

    pub struct UICursorData {
        pub cursor_ndc: Vec2,
        pub btn_states: [ButtonState; 8],
    }

    pub struct WidgetRecurseContextMut<'a, 'b> {
        widget_events: &'a mut WriteExpect<'b, WidgetEvents>,
        hierarchy: &'a Hierarchy<HasParent>,
        widget_vec: &'a mut WriteStorage<'b, Widget>,
        cur_widget_draw_idx: u32
    }

    pub struct WidgetFrame {
        wvp: Mat3,
        size: Vec2
    }

    pub fn _update_widget_layout(
        ctx: &mut WidgetRecurseContextMut,
        frame: &WidgetFrame, entity: Entity, dirty: bool) {

        let (self_frame, self_dirty) = {
            let widget = ctx.widget_vec.get_mut(entity).unwrap();

            widget.runtime_info.draw_idx = ctx.cur_widget_draw_idx;
            ctx.cur_widget_draw_idx += 1;

            let d = if dirty || widget.runtime_info.dirty {
                let (x, width) = _calc_layout(frame.size.x,
                                              widget.layout_x, widget.pivot.x);
                let (y, height) = _calc_layout(frame.size.y,
                                               widget.layout_y, widget.pivot.y);

                widget.runtime_info.size = vec2(width, height);
                widget.runtime_info.wvp = frame.wvp * calc_widget_mat(vec2(x, y),
                                                                      widget.scl, widget.rot);
                widget.runtime_info.wvp_inverse = widget.runtime_info.wvp.invert().unwrap();
                true
            } else {
                false
            };

            (WidgetFrame {
                wvp: widget.runtime_info.wvp,
                size: widget.runtime_info.size
            }, d)
        };

        for child in ctx.hierarchy.children(entity).iter().map(|x| x.clone()) {
            _update_widget_layout(ctx, &self_frame, child, dirty || self_dirty);
        }
    }

    pub fn _update_widget_input(ctx: &mut WidgetRecurseContextMut, entity: Entity, input: &UICursorData)
                                -> u8 { // 返回值是子节点或自己是否已经处理hover/click
        let mut button_flags: u8 = 0;
        for child in ctx.hierarchy.children(entity) {
            button_flags |= _update_widget_input(ctx, *child, &input);
        }

        let widget = ctx.widget_vec.get_mut(entity).unwrap();
        let pos_local: Vec3 = widget.runtime_info.wvp_inverse * vec3(input.cursor_ndc.x, input.cursor_ndc.y, 1.);
        let pos_local = vec2(pos_local.x, pos_local.y);
        if widget.raycast {
            // info!("update_widget_layout {:?} {:?} {:?}", entity, pos_local, widget.runtime_info.size);
        }
        let rect = Rect::new(0., 0., widget.runtime_info.size.x, widget.runtime_info.size.y);
        if widget.raycast {
            for btn_id in 0u8..8 {
                if (button_flags & (1 << btn_id)) != 0 { // 其他人处理过了
                    continue
                }

                let btn_state = input.btn_states[btn_id as usize];

                let cursor_state = &mut widget.runtime_info.cursor_states[btn_id as usize];
                match cursor_state {
                    WidgetCursorState::Dragging => {
                        if btn_state.is_up() {
                            // info!("Widget up! {:?}", entity.id());
                            ctx.widget_events.events.push(WidgetEvent::Release { entity, btn: btn_id });
                            *cursor_state = WidgetCursorState::Idle;
                        } else {
                            button_flags |= 1 << btn_id;
                        }
                    },
                    _ => {
                        if rect.contains(&pos_local) && btn_state == ButtonState::Down {
                            // info!("Widget down! {:?}", entity.id());
                            ctx.widget_events.events.push(WidgetEvent::Clicked { entity, btn: btn_id });
                            *cursor_state = WidgetCursorState::Dragging;
                            button_flags |= 1 << btn_id;
                        }
                    }
                }
            }
        }

        button_flags
    }

    /// UI layout update -> UI control update
    pub struct UIUpdateSystem {

    }

    impl<'a> System<'a> for UIUpdateSystem {

        type SystemData = (
            Entities<'a>,
            ReadExpect<'a, Hierarchy<HasParent>>,
            ReadStorage<'a, Canvas>,
            WriteStorage<'a, Widget>,
            ReadExpect<'a, WindowInfo>,
            ReadExpect<'a, RawInputData>,
            WriteExpect<'a, WidgetEvents>);

        fn run(&mut self, (entities, hierarchy, canvas_vec, mut widget_vec, window_info, input, mut widget_events): Self::SystemData) {
            widget_events.events.clear();

            let mut all_canvas: Vec<(Entity, &Canvas)> = (&*entities, &canvas_vec).join().collect();
            all_canvas.sort_by_key(|x| x.1.order);

            for (ent, canvas) in all_canvas {
                let (width, height) = canvas.actual_size(&*window_info);

                let frame = WidgetFrame {
                    wvp: mat3::ortho(0., width, 0., height), // Map (0,0)->(width,height) to NDC
                    size: vec2(width, height)
                };

                let mut rec_ctx = internal::WidgetRecurseContextMut {
                    widget_events: &mut widget_events,
                    hierarchy: &*hierarchy,
                    widget_vec: &mut widget_vec,
                    cur_widget_draw_idx: 0
                };

                let cursor_pos = input.cursor_position;
                fn cvt_cursor(x: f32, sz: u32) -> f32 {
                    2. * ((x / (sz as f32)) - 0.5)
                }

                // Y axis need to be inversed
                let cursor_pos = vec2(cvt_cursor(cursor_pos.x, window_info.pixel_size.0),
                                      cvt_cursor(window_info.pixel_size.1 as f32 - cursor_pos.y - 1., window_info.pixel_size.1));
                let cursor_data = internal::UICursorData {
                    cursor_ndc: cursor_pos,
                    btn_states: input.get_mouse_buttons()
                };

                for child in hierarchy.children(ent) {
                    // FIXME: canvas的dirty 取决于window是否resize
                    internal::_update_widget_layout(&mut rec_ctx, &frame, *child, false);
                    internal::_update_widget_input(&mut rec_ctx, *child, &cursor_data);
                }
            }

            // info!("cursor pos: {:?}", input.cursor_position);
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

    pub struct UIBatcher {
        ls: Vec<(u32, DrawInstance)>
    }

    impl UIBatcher {

        pub fn new() -> Self {
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


    struct ImageBatchContext<'a, 'b> {
        entities: &'a Entities<'b>,
        hierarchy: &'a Hierarchy<HasParent>,
        widget_vec: &'a ReadStorage<'b, Widget>,
        image_read: &'a ReadStorage<'b, Image>,
        batcher: &'a mut UIBatcher
    }

    impl<'a, 'b> ImageBatchContext<'a, 'b> {
    }

    pub struct UIImageBatchSystem {
    }

    impl UIImageBatchSystem {

        fn _walk<'a>(ctx: &mut ImageBatchContext, entity: Entity) {
            if let Some(image) = ctx.image_read.get(entity) {
                let widget = ctx.widget_vec.get(entity).unwrap();
                // 这里再乘一个 size 把 [0,1] 的顶点坐标缩放
                let wvp = widget.runtime_info.wvp * mat3::scale(widget.runtime_info.size);
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

    impl_vertex!(ImageVertex, v_pos => 0, v_uv => 1);

    #[derive(Copy, Clone, Default)]
    struct ImageInstanceData {
        i_wvp_c0: [f32; 4],
        i_wvp_c1: [f32; 4],
        i_wvp_c2: [f32; 4],
        i_wvp_c3: [f32; 4],
        i_uv_min: [f32; 2],
        i_uv_max: [f32; 2],
        i_color: [f32; 4]
    }

    impl_vertex!(ImageInstanceData, Instance,
        i_wvp_c0 => 2, i_wvp_c1 => 3, i_wvp_c2 => 4, i_wvp_c3 => 5,
        i_uv_min => 6, i_uv_max => 7, i_color => 8);

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

    struct UIImageRenderData {
        default_program: ResourceRef<ShaderProgram>,
        default_material: Material,
        default_pipeline: wgpu::RenderPipeline,
        vbo: wgpu::Buffer,
        ibo: wgpu::Buffer,
        white_texture: ResourceRef<Texture>
    }

    impl UIImageRenderData {

        fn new(res_mgr: &mut ResManager, wgpu_state: &WgpuState) -> Self {
            use crate::client::graphics;
            let program = graphics::load_shader_by_content(&wgpu_state.device,
               include_str!("../../assets/ui_image_default.vert"),
               include_str!("../../assets/ui_image_default.frag"),
                "ui_image_default.vert",
                "ui_image_default.frag",
            &[
                UniformLayoutConfig {
                    binding: 0,
                    name: "u_texture".to_string(),
                    ty: UniformBindingType::Texture,
                    visibility: UniformVisibility::Fragment
                },
                UniformLayoutConfig {
                    binding: 1,
                    name: "u_sampler".to_string(),
                    ty: UniformBindingType::Sampler,
                    visibility: UniformVisibility::Fragment
                }
            ]);
            let program = res_mgr.add(program);

            let vbo = wgpu_state.device.create_buffer_with_data(bytemuck::cast_slice(&[
                    ImageVertex::new(0., 0., 0., 0.),
                    ImageVertex::new(0., 1., 0., 1.),
                    ImageVertex::new(1., 1., 1., 1.),
                    ImageVertex::new(1., 0., 1., 0.)
                ]),
                wgpu::BufferUsage::VERTEX);

            let ibo = wgpu_state.device.create_buffer_with_data(
                bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
                wgpu::BufferUsage::INDEX
            );

            let white_texture = res_mgr.add(graphics::create_texture(
                wgpu_state, vec![255, 255, 255, 255], (1, 1), &SamplerConfig {
                    address: wgpu::AddressMode::ClampToEdge,
                    filter: FilterMode::Nearest
                }));

            let mut properties = HashMap::new();
            properties.insert("u_texture".to_string(), MatProperty::Texture(white_texture.clone()));
            properties.insert("u_sampler".to_string(), MatProperty::TextureSampler(white_texture.clone()));

            let default_mat = Material::create(&res_mgr, wgpu_state, program.clone(), properties);

            let program_inst = res_mgr.get(&program);

            let pipeline_layout = wgpu_state.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&program_inst.bind_group_layout]
            });
            
            let pipeline = wgpu_state.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                layout: &pipeline_layout,
                vertex_stage: program_inst.vertex_desc(),
                fragment_stage: Some(program_inst.fragment_desc()),
                rasterization_state: None,
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                color_states: &[wgpu::ColorStateDescriptor {
                    format: wgpu_state.sc_desc.format,
                    alpha_blend: wgpu::BlendDescriptor::REPLACE,
                    color_blend: wgpu::BlendDescriptor::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL
                }],
                depth_stencil_state: None,
                vertex_state: wgpu::VertexStateDescriptor {
                    index_format: wgpu::IndexFormat::Uint16,
                    vertex_buffers: &[get_vertex!(ImageVertex), get_vertex!(ImageInstanceData)]
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false
            });

            Self {
                default_program: program,
                vbo, ibo,
                white_texture,
                default_material: default_mat,
                default_pipeline: pipeline
            }
        }
    }

    pub struct UIRenderSystem {
        image_data: UIImageRenderData,
        wgpu_state: WgpuStateCell
    }

    impl UIRenderSystem {

        pub fn new(init_data: &mut InitData) -> Self {
            let wgpu_state = init_data.wgpu_state.borrow();
            let image_data = UIImageRenderData::new(&mut init_data.res_mgr, &*wgpu_state);
            Self {
                image_data,
                wgpu_state: init_data.wgpu_state.clone()
            }
        }

    }

    impl<'a> System<'a> for UIRenderSystem {
        type SystemData = (ReadExpect<'a, ResManager>, WriteStorage<'a, Canvas>);

        fn run(&mut self, (res_mgr, mut canvas_write): Self::SystemData) {
            use crate::client::graphics;

            let wgpu_state = self.wgpu_state.borrow();
            let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: None
            });
            for canvas in (&mut canvas_write).join() {
                let draw_calls = canvas.batcher.finish();
                for draw in draw_calls {
                    match draw {
                        DrawInstance::Image {
                            wvp, sprite, material, color
                        } => {
                            // TODO: Support custom material
                            let image_data = &mut self.image_data;
                            let (texture, uv0, uv1) = match &sprite {
                                Some(sr) => {
                                    let sheet = res_mgr.get(&sr.sheet);
                                    let spr_data = &sheet.sprites[sr.idx];
                                    (sheet.texture.clone(), spr_data.uv_min, spr_data.uv_max)
                                }
                                None => {
                                    (image_data.white_texture.clone(), vec2(0., 0.), vec2(1., 1.))
                                }
                            };

                            image_data.default_material.set("u_texture", MatProperty::Texture(texture.clone()));
                            image_data.default_material.set("u_sampler", MatProperty::TextureSampler(texture));

                            let instances = [ImageInstanceData {
                                i_wvp_c0: wvp.x.into(),
                                i_wvp_c1: wvp.y.into(),
                                i_wvp_c2: wvp.z.into(),
                                i_wvp_c3: wvp.w.into(),
                                i_uv_min: [uv0.x, uv0.y],
                                i_uv_max: [uv1.x, uv1.y],
                                i_color: color.into(),
                            }];
                            let instance_buf = wgpu_state.device.create_buffer_with_data(
                                bytemuck::cast_slice(&instances),
                                wgpu::BufferUsage::VERTEX
                            );
                            
                            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                                    attachment: &wgpu_state.frame_texture.as_ref().unwrap().view,
                                    resolve_target: None,
                                    load_op: wgpu::LoadOp::Load,
                                    store_op: wgpu::StoreOp::Store,
                                    clear_color: Default::default()
                                }],
                                depth_stencil_attachment: None
                            });

                            let bind_group = image_data.default_material.get_bind_group(&*res_mgr, &wgpu_state.device);
                            render_pass.set_pipeline(&image_data.default_pipeline);
                            render_pass.set_bind_group(0, &bind_group, &[]);
                            render_pass.set_vertex_buffer(0, &image_data.vbo, 0, 0);
                            render_pass.set_vertex_buffer(1, &instance_buf, 0, 0);
                            render_pass.set_index_buffer(&image_data.ibo, 0, 0);
                            render_pass.draw_indexed(0..6, 0, 0..1);
                        },
                        _ => unimplemented!()
                    }
                }
            }

            wgpu_state.queue.submit(&[encoder.finish()]);
        }
    }
}

#[cfg(test)]
mod test {
    use specs::{Builder, DispatcherBuilder, World, WorldExt};
    use specs_hierarchy::HierarchySystem;

    use crate::client::ui::{AlignType, Canvas, internal, LayoutType, RefResolution, Widget};
    use crate::client::WindowInfo;
    use crate::ecs::HasParent;
    use crate::math::{vec2};
    use crate::math;

    #[test]
    fn layout_simple() {
        let mut world = World::new();

        let mut dispatcher = DispatcherBuilder::new()
            .with(HierarchySystem::<HasParent>::new(&mut world), "", &[])
            .with(internal::UIUpdateSystem {}, "", &[])
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
                batcher: internal::UIBatcher::new()
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
                assert!(math::vec2_approx_eq(w.runtime_info.size, vec2(1920., 1080.)));
            } else {
                panic!();
            }

            if let Some(w) = widget_storage.get(w1) {
                assert!(math::vec2_approx_eq(w.runtime_info.size, vec2(100., 100.)))
            } else {
                panic!();
            }
        }

        println!("UI layout OK!");
    }

}