use mu::*;
use mu::math;
use mu::client::graphics::*;
use specs::prelude::*;
use mu::ecs::{Transform, Time};
use mu::util::Color;
use mu::client::input::RawInputData;
use winit::event::VirtualKeyCode;
use wgpu::BufferAddress;
use wgpu::util::DeviceExt;

#[derive(Copy, Clone)]
struct QuadVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2]
}

impl QuadVertex {
    fn new(x: f32, y: f32, u: f32, v: f32) -> Self {
        Self {
            position: [x, y, 0.0],
            uv : [u, v]
        }
    }
}

impl_vertex!(QuadVertex, position => 0, uv => 1);

struct DrawQuadSystem {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    ubo: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline
}

impl DrawQuadSystem {

    fn new(wgpu_state: &WgpuState) -> Self {
        let program = load_shader(&wgpu_state.device, "shader/quad.shader.json");
        let texture = load_texture(&*wgpu_state, "texture/landscape.tex.json");

        let quad = vec![
            QuadVertex::new(-0.5, 0.5, 0., 0.),
            QuadVertex::new(-0.5, -0.5, 0., 1.),
            QuadVertex::new(0.5, -0.5, 1., 1.),
            QuadVertex::new(0.5, 0.5, 1., 0.)
        ];

        let texture_view = texture.default_view;

        let vbo = wgpu_state.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsage::VERTEX
        });
        let ibo = wgpu_state.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
           usage: wgpu::BufferUsage::INDEX
        });

        let ubo = wgpu_state.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[0.0f32; 16]),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST
            }
        );

        // let ubo_range = 0..std::mem::size_of::<[f32;16]>() as wgpu::BufferAddress;
        let bind_group = wgpu_state.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &program.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(ubo.slice(..))
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view)
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler)
                },
            ],
            label: None
        });

        let pipeline_layout = wgpu_state.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&program.bind_group_layout],
            push_constant_ranges: &[]
        });

        let pipeline = wgpu_state.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor { module: &program.vertex, entry_point: "main" },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor { module: &program.fragment, entry_point: "main" }),
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
                vertex_buffers: &[get_vertex!(QuadVertex)]
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false
        });

        Self {
            vbo,
            ibo,
            ubo,
            bind_group,
            pipeline,
        }
    }

}

impl<'a> System<'a> for DrawQuadSystem {
    type SystemData = ReadExpect<'a, WgpuState>;

    fn run(&mut self, wgpu_state: Self::SystemData) {
        use std::mem;
        with_render_data(|r| {
            let cam_infos = &mut r.camera_infos;
            for cam_info in cam_infos {
                let wvp_mat_arr: [f32; 16] = math::mat::to_array(cam_info.wvp_matrix);
                let tmp_mat_buf = wgpu_state.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&wvp_mat_arr),
                        usage: wgpu::BufferUsage::COPY_SRC
                    }
                );
                cam_info.encoder.copy_buffer_to_buffer(&tmp_mat_buf, 0,
                    &self.ubo, 0, mem::size_of::<[f32; 16]>() as BufferAddress);

                {
                    let mut render_pass = cam_info.render_pass(&*wgpu_state);
                    render_pass.set_pipeline(&self.pipeline);
                    render_pass.set_bind_group(0, &self.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, self.vbo.slice(..));
                    render_pass.set_index_buffer(self.ibo.slice(..));
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
        let x_axis = Self::_map_axis(&input, VirtualKeyCode::D, VirtualKeyCode::A);
        let y_axis = Self::_map_axis(&input, VirtualKeyCode::W, VirtualKeyCode::S);

        for item in (&mut trans_vec).join() {
            item.pos.x += x_axis * dt;
            item.pos.y += y_axis * dt;
        }
    }
}


struct QuadModule;

impl Module for QuadModule {
    fn init(&self, ctx: &mut InitContext) {
        ctx.dispatch(
            InsertInfo::new("update_trans"),
            move |_, i| i.insert(UpdateCameraSystem));

        let insert_info =
            InsertInfo::new("quad")
                .after(&[DEP_CAM_DRAW_SETUP])
                .before(&[DEP_CAM_DRAW_TEARDOWN]);
        ctx.dispatch_thread_local(
            insert_info,
            move |init, i|
                i.insert_thread_local(DrawQuadSystem::new(&*init.world.read_resource())));
    }

    fn start(&self, start_data: &mut StartContext) {
        start_data.world.create_entity()
            .with(Transform::new())
            .with(Camera { projection: CameraProjection::Orthographic { size: 2., z_near: -1., z_far: 1. },
                clear_color: Some(Color::rgb(0.1, 0.1, 0.3)),
                clear_depth: true
            })
            .build();
    }
}

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Textured Quad (WASD to move camera)")
        .add_game_module(GraphicsModule)
        .add_game_module(QuadModule)
        .build();

    runtime.start();
}