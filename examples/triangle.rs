use mu::*;
use mu::client::graphics::*;
use specs::System;
use specs::{ReadExpect};
use mu::ecs::Time;
use mu::asset::*;
use std::cell::RefCell;
use std::rc::Rc;
use mu::client::graphics;
use mu::util::Color;

#[derive(Copy, Clone)]
struct TriangleVertex {
    pub position: [f32; 3]
}

impl TriangleVertex {

    fn desc<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        use std::mem;
        wgpu::VertexBufferDescriptor {
            stride: mem::size_of::<TriangleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3
                }
            ]
        }
    }

}

unsafe impl bytemuck::Pod for TriangleVertex {}
unsafe impl bytemuck::Zeroable for TriangleVertex {}

#[derive(Copy, Clone)]
struct TriangleUniform {
    pub offset: [f32; 3]
}
unsafe impl bytemuck::Pod for TriangleUniform {}
unsafe impl bytemuck::Zeroable for TriangleUniform {}

struct DrawTriangleSystem {
    wgpu_states: WgpuStateCell,
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    ubo: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    bindgroup: wgpu::BindGroup,
    elapsed: f32,
}

impl DrawTriangleSystem {

    fn new(res_mgr: &mut ResManager, wgpu_states_ref: Rc<RefCell<WgpuState>>) -> Self {
        let program_pool = res_mgr.get_pool_mut::<ShaderProgram>();
        let program_ref = {
            let wgpu_states = wgpu_states_ref.borrow();
            program_pool.load(&wgpu_states.device, "shader/triangle.shader.json")
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
            let wgpu_states = wgpu_states_ref.borrow();
            let vbo = wgpu_states.device.create_buffer_with_data(
                bytemuck::cast_slice(&triangle),
                wgpu::BufferUsage::VERTEX
            );

            let ibo = wgpu_states.device.create_buffer_with_data(
                bytemuck::cast_slice(&indices),
                wgpu::BufferUsage::INDEX
            );

            let program = program_pool.get(&program_ref);

            let ubo = wgpu_states.device.create_buffer_with_data(
                bytemuck::cast_slice(&[TriangleUniform { offset: [0., 0., 0.] }]),
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST
            );

            let uniform_bindgroup = wgpu_states.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &program.bind_group_layout,
                bindings: &[
                    wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &ubo,
                            range: 0..std::mem::size_of::<TriangleUniform>() as wgpu::BufferAddress
                        },
                    },
                ],
                label: None
            });

            let render_pipeline_layout = wgpu_states.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&program.bind_group_layout],
            });

            let pipeline = wgpu_states.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                layout: &render_pipeline_layout,
                vertex_stage: wgpu::ProgrammableStageDescriptor { module: &program.vertex, entry_point: "main" },
                fragment_stage: Some(wgpu::ProgrammableStageDescriptor { module: &program.fragment, entry_point: "main" }),
                rasterization_state: None,
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                color_states: &[
                    wgpu::ColorStateDescriptor {
                        format: wgpu_states.sc_desc.format,
                        color_blend: wgpu::BlendDescriptor::REPLACE,
                        alpha_blend: wgpu::BlendDescriptor::REPLACE,
                        write_mask: wgpu::ColorWrite::ALL
                    }
                ],
                depth_stencil_state: None,
                vertex_state: wgpu::VertexStateDescriptor {
                    index_format: wgpu::IndexFormat::Uint16,
                    vertex_buffers: &[
                        TriangleVertex::desc()
                    ]
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false
            });

            (vbo, ibo, ubo, pipeline, uniform_bindgroup)
        };

        Self {
            wgpu_states: wgpu_states_ref,
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
    type SystemData = ReadExpect<'a, Time>;

    fn run(&mut self, time: Self::SystemData) {
        let wgpu_states = self.wgpu_states.borrow();

        let dt = (*time).get_delta_time();
        self.elapsed += dt;

        let offset_y = 0.5 * f32::sin(2. * self.elapsed);
        let tmp_buffer = wgpu_states.device.create_buffer_with_data(
            bytemuck::cast_slice(&[TriangleUniform { offset: [0., offset_y, 0. ] }]),
            wgpu::BufferUsage::COPY_SRC
        );

        let mut encoder = wgpu_states.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: None
        });
        encoder.copy_buffer_to_buffer(&tmp_buffer, 0, &self.ubo, 0,
                                      std::mem::size_of::<TriangleUniform>() as wgpu::BufferAddress);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[
                wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &wgpu_states.frame_texture.as_ref().unwrap().view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: Color::rgb(0.3, 0.1, 0.1).into()
                }
            ],
            depth_stencil_attachment: None
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, &self.vbo, 0, 0);
        render_pass.set_index_buffer(&self.ibo, 0, 0);
        render_pass.set_bind_group(0, &self.bindgroup, &[]);
        render_pass.draw_indexed(0..3, 0, 0..1);
        drop(render_pass);

        wgpu_states.queue.submit(&[encoder.finish()]);
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
                let sys = DrawTriangleSystem::new(&mut init_data.res_mgr, init_data.wgpu_state.clone());
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