use mu;
use mu::math;
use mu::{RuntimeBuilder, Module, InitData, InsertInfo, WgpuStateCell, InitContext, StartContext};
use mu::client::graphics::*;
use specs::prelude::*;
use mu::ecs::{Transform, Time};
use mu::client::graphics::CameraProjection::Orthographic;
use mu::util::Color;
use mu::client::input::RawInputData;
use winit::event::VirtualKeyCode;
use mu::asset;
use mu::asset::{ResourceRef, ResManager};
use wgpu::BufferAddress;

#[derive(Copy, Clone)]
struct QuadVertex {
    position: [f32; 3],
    uv: [f32; 2]
}

unsafe impl bytemuck::Pod for QuadVertex {}
unsafe impl bytemuck::Zeroable for QuadVertex {}

impl QuadVertex {
    fn new(x: f32, y: f32, u: f32, v: f32) -> Self {
        Self {
            position: [x, y, 0.0],
            uv : [u, v]
        }
    }

    fn desc<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        use std::mem;
        wgpu::VertexBufferDescriptor {
            stride: mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3
                },
                wgpu::VertexAttributeDescriptor {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float2
                }
            ]
        }
    }
}

struct DrawQuadSystem {
    wgpu_state: WgpuStateCell,
    program: ResourceRef<ShaderProgram>,
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    ubo: wgpu::Buffer,
    tex: ResourceRef<Texture>,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline
}

impl DrawQuadSystem {

    fn new(res_mgr: &mut ResManager, wgpu_states_cell: WgpuStateCell) -> Self {
        let wgpu_state = wgpu_states_cell.borrow();
        let program_ref = {
            let mut program_pool = res_mgr.get_pool_mut::<ShaderProgram>();
            program_pool.load(&wgpu_state.device, "shader/quad.shader.json")
        };

        let texture_ref = {
            let mut texture_pool = res_mgr.get_pool_mut::<Texture>();
            texture_pool.load_texture(&*wgpu_state, "texture/landscape.tex.json")
        };

        let program = res_mgr.get(&program_ref);
        let quad = vec![
            QuadVertex::new(-0.5, 0.5, 0., 0.),
            QuadVertex::new(-0.5, -0.5, 0., 1.),
            QuadVertex::new(0.5, -0.5, 1., 1.),
            QuadVertex::new(0.5, 0.5, 1., 0.)
        ];
        let texture = res_mgr.get(&texture_ref);

        let texture_view = texture.raw_texture.create_default_view();
        let sampler = wgpu_state.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.,
            lod_max_clamp: 100.,
            compare: wgpu::CompareFunction::Always
        });

        let vbo = wgpu_state.device.create_buffer_with_data(bytemuck::cast_slice(&quad),
                                                            wgpu::BufferUsage::VERTEX);
        let ibo = wgpu_state.device.create_buffer_with_data(
            bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
            wgpu::BufferUsage::INDEX
        );

        let ubo = wgpu_state.device.create_buffer_with_data(
            bytemuck::cast_slice(&[0.0f32; 16]),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST
        );

        let bind_group = wgpu_state.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &program.bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &ubo,
                        range: 0..std::mem::size_of::<[f32;16]>() as wgpu::BufferAddress
                    }
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view)
                },
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler)
                },
            ],
            label: None
        });

        let pipeline_layout = wgpu_state.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&program.bind_group_layout],
        });

        let pipeline = wgpu_state.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor { module: &program.vertex, entry_point: "main" },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor { module: &program.fragment, entry_point: "main" }),
            rasterization_state: None,
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[QuadVertex::desc()]
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false
        });

        drop(wgpu_state);
        Self {
            wgpu_state: wgpu_states_cell,
            program: program_ref,
            tex: texture_ref,
            vbo,
            ibo,
            ubo,
            bind_group,
            pipeline,
        }
    }

}

impl<'a> System<'a> for DrawQuadSystem {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        use std::mem;
        with_render_data(|r| {
            let cam_infos = &mut r.camera_infos;
            let wgpu_state = self.wgpu_state.borrow();
            for cam_info in cam_infos {
                let wvp_mat_arr: [f32; 16] = math::mat::to_array(cam_info.wvp_matrix);
                let tmp_mat_buf = wgpu_state.device.create_buffer_with_data(
                    bytemuck::cast_slice(&wvp_mat_arr),
                    wgpu::BufferUsage::COPY_SRC
                );
                cam_info.encoder.copy_buffer_to_buffer(&tmp_mat_buf, 0,
                    &self.ubo, 0, mem::size_of::<[f32; 16]>() as BufferAddress);

                {
                    let mut render_pass = cam_info.render_pass(&*wgpu_state);
                    render_pass.set_pipeline(&self.pipeline);
                    render_pass.set_bind_group(0, &self.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, &self.vbo, 0, 0);
                    render_pass.set_index_buffer(&self.ibo, 0, 0);
                    render_pass.draw_indexed(0..6, 0, 0..1);
                }
            }
        });
    }
}

struct UpdateCameraSystem;

impl UpdateCameraSystem {

    fn _map_axis(input: &RawInputData, positive: VirtualKeyCode, negative: VirtualKeyCode) -> f32 {
        let pos = input.get_key(positive).is_down();
        let neg = input.get_key(negative).is_down();
        (if pos { 1. } else { 0. }) + (if neg { -1. } else { 0. })
    }
}

impl<'a> System<'a> for UpdateCameraSystem {
    type SystemData = (ReadExpect<'a, RawInputData>, ReadExpect<'a, Time>, WriteStorage<'a, Transform>);

    fn run(&mut self, (input, time, mut trans_vec): Self::SystemData) {
        let dt = (&time).get_delta_time();
        let x_axis = Self::_map_axis(&input, VirtualKeyCode::Right, VirtualKeyCode::Left);
        let y_axis = Self::_map_axis(&input, VirtualKeyCode::Up, VirtualKeyCode::Down);

        for item in (&mut trans_vec).join() {
            item.pos.x += x_axis * dt;
            item.pos.y += y_axis * dt;
        }
    }
}


struct QuadModule;

impl Module for QuadModule {
    fn init(&self, init_data: &mut InitContext) {
        init_data.dispatch(
            InsertInfo::new("update_trans"),
            move |_, i| i.insert(UpdateCameraSystem));

        let insert_info =
            InsertInfo::new("quad")
                .after(&[DEP_CAM_DRAW_SETUP])
                .before(&[DEP_CAM_DRAW_TEARDOWN]);
        init_data.dispatch_thread_local(
            insert_info,
            move |init, i|
                i.insert_thread_local(DrawQuadSystem::new(&mut init.res_mgr, init.wgpu_state.clone())));
    }

    fn start(&self, start_data: &mut StartContext) {
        start_data.world.create_entity()
            .with(Transform::new())
            .with(Camera { projection: Orthographic { size: 2., z_near: -1., z_far: 1. },
                clear_color: Some(Color::rgb(0.1, 0.1, 0.3)),
                clear_depth: true
            })
            .build();
    }
}

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Textured Quad")
        .add_game_module(GraphicsModule)
        .add_game_module(QuadModule)
        .build();

    runtime.start();
}