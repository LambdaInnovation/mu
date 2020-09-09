use mu::*;
use mu::client::graphics::*;
use specs::{System, WorldExt};
use specs::{ReadExpect};
use mu::ecs::Time;
use mu::client::graphics;
use mu::util::Color;
use wgpu::util::DeviceExt;

#[derive(Copy, Clone)]
struct TriangleVertex {
    pub position: [f32; 3]
}

impl_vertex!(TriangleVertex, position => 0);

#[derive(Copy, Clone)]
struct TriangleUniform {
    pub offset: [f32; 3]
}
unsafe impl bytemuck::Pod for TriangleUniform {}
unsafe impl bytemuck::Zeroable for TriangleUniform {}

struct DrawTriangleSystem {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    ubo: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    bindgroup: wgpu::BindGroup,
    elapsed: f32,
}

impl DrawTriangleSystem {

    fn new(wgpu_state: &WgpuState) -> Self {
        let program = {
            load_shader(&wgpu_state.device, "shader/triangle.shader.json")
        };

        let triangle = {
            let v1 = TriangleVertex {
                position: [-0.5, -0.5, 0.0],
            };
            let v2 = TriangleVertex {
                position: [0.0, 0.5, 0.0],
            };
            let v3 = TriangleVertex {
                position: [0.5, -0.5, 0.0],
            };
            [v1, v2, v3]
        };

        let indices = [0u16, 1, 2];

        let (vbo, ibo, ubo, pipeline, bindgroup) = {
            let vbo = wgpu_state.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice( & triangle),
                    usage: wgpu::BufferUsage::VERTEX
                }
            );

            let ibo = wgpu_state.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsage::INDEX
                }
            );

            let ubo = wgpu_state.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[TriangleUniform { offset: [0., 0., 0.] }]),
                    usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST
                }
            );

            let range = 0..std::mem::size_of::<TriangleUniform>() as wgpu::BufferAddress;
            let uniform_bindgroup = wgpu_state.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &program.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(ubo.slice(range)),
                    },
                ],
                label: None
            });

            let render_pipeline_layout = wgpu_state.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&program.bind_group_layout],
                push_constant_ranges: &[]
            });

            let pipeline = wgpu_state.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&render_pipeline_layout),
                vertex_stage: wgpu::ProgrammableStageDescriptor { module: &program.vertex, entry_point: "main" },
                fragment_stage: Some(wgpu::ProgrammableStageDescriptor { module: &program.fragment, entry_point: "main" }),
                rasterization_state: None,
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                color_states: &[
                    wgpu::ColorStateDescriptor {
                        format: wgpu_state.sc_desc.format,
                        color_blend: wgpu::BlendDescriptor::REPLACE,
                        alpha_blend: wgpu::BlendDescriptor::REPLACE,
                        write_mask: wgpu::ColorWrite::ALL
                    }
                ],
                depth_stencil_state: None,
                vertex_state: wgpu::VertexStateDescriptor {
                    index_format: wgpu::IndexFormat::Uint16,
                    vertex_buffers: &[get_vertex!(TriangleVertex)]
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false
            });

            (vbo, ibo, ubo, pipeline, uniform_bindgroup)
        };

        Self {
            vbo,
            ibo,
            ubo,
            pipeline,
            bindgroup,
            elapsed: 0.0
        }
    }

}

impl<'a> System<'a> for DrawTriangleSystem {
    type SystemData = (ReadExpect<'a, WgpuState>, ReadExpect<'a, Time>);

    fn run(&mut self, (wgpu_state, time): Self::SystemData) {
        let dt = (*time).get_delta_time();
        self.elapsed += dt;

        let offset_y = 0.5 * f32::sin(2. * self.elapsed);
        let tmp_buffer = wgpu_state.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[TriangleUniform { offset: [0., offset_y, 0. ] }]),
            usage: wgpu::BufferUsage::COPY_SRC
        });

        let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: None
        });
        encoder.copy_buffer_to_buffer(&tmp_buffer, 0, &self.ubo, 0,
                                      std::mem::size_of::<TriangleUniform>() as wgpu::BufferAddress);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[
                wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &wgpu_state.frame_texture.as_ref().unwrap().output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color::rgb(0.3, 0.1, 0.1).into()),
                        store: true
                    }
                }
            ],
            depth_stencil_attachment: None
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vbo.slice(..));
        render_pass.set_index_buffer(self.ibo.slice(..));
        render_pass.set_bind_group(0, &self.bindgroup, &[]);
        render_pass.draw_indexed(0..3, 0, 0..1);
        drop(render_pass);

        wgpu_state.queue.submit(Some(encoder.finish()));
        //
        // let uniform = glium::uniform! {
        //         offset: (0.0, , 0.0)
        //     };
    }
}

struct TriangleModule;

impl mu::Module for TriangleModule {
    fn init(&self, init_data: &mut InitContext) {
        init_data.dispatch_thread_local(InsertInfo::new("triangle")
                                            .after(&[graphics::DEP_CAM_DRAW_SETUP])
                                            .before(&[graphics::DEP_CAM_DRAW_TEARDOWN]),
            move |init_data, insert| {
                let wgpu_state = init_data.world.read_resource::<WgpuState>();
                let sys = DrawTriangleSystem::new(&*wgpu_state);
                insert.insert_thread_local(sys);
            })
    }
}

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");
    let runtime = RuntimeBuilder::new("Hello Triangle")
        .add_game_module(GraphicsModule)
        .add_game_module(TriangleModule)
        .build();

    runtime.start();
}