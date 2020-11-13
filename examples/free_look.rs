use mu::*;
use mu::client::graphics;
use wgpu::util::DeviceExt;
use specs::prelude::*;
use mu::math::*;
use mu::util::Color;
use mu::ecs::{Transform, Time};
use mu::client::input::{RawInputData, ButtonState};
use mu::math::cgmath::Rotation3;
use cgmath::InnerSpace;
use winit::event::VirtualKeyCode;

#[derive(Copy, Clone)]
struct BoxVertex {
    pos: [f32; 3],
    uv: [f32; 2]
}

impl BoxVertex {
    fn new(p: [f32; 3], uv: [f32; 2]) -> Self {
        Self {
            pos: p,
            uv
        }
    }
}

impl_vertex!(BoxVertex, pos => 0, uv => 1);

struct BoxInstance {
    pos: Vec3,
    crl: Color
}

struct DrawBoxSystem {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    ubo: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    box_poses: Vec<BoxInstance>
}

impl DrawBoxSystem {

    fn new(ws: &WgpuState) -> Self {
        let v0 = [0., 0., 0.];
        let v1 = [1., 0., 0.];
        let v2 = [1., 0., 1.];
        let v3 = [0., 0., 1.];
        let v4 = [0., 1., 0.];
        let v5 = [1., 1., 0.];
        let v6 = [1., 1., 1.];
        let v7 = [0., 1., 1.];

        let uv0 = [0., 0.];
        let uv1 = [0., 1.];
        let uv2 = [1., 1.];
        let uv3 = [1., 0.];

        let vertices = vec![
            BoxVertex::new(v0, uv0),
            BoxVertex::new(v1, uv1),
            BoxVertex::new(v2, uv2),
            BoxVertex::new(v3, uv3),

            BoxVertex::new(v5, uv0),
            BoxVertex::new(v2, uv1),
            BoxVertex::new(v3, uv2),
            BoxVertex::new(v6, uv3),

            BoxVertex::new(v6, uv0),
            BoxVertex::new(v3, uv1),
            BoxVertex::new(v0, uv2),
            BoxVertex::new(v7, uv3),

            BoxVertex::new(v7, uv0),
            BoxVertex::new(v0, uv1),
            BoxVertex::new(v1, uv2),
            BoxVertex::new(v4, uv3),

            BoxVertex::new(v4, uv0),
            BoxVertex::new(v1, uv1),
            BoxVertex::new(v2, uv2),
            BoxVertex::new(v5, uv3),

            BoxVertex::new(v7, uv0),
            BoxVertex::new(v4, uv1),
            BoxVertex::new(v5, uv2),
            BoxVertex::new(v6, uv3),
        ];

        let vbo = ws.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsage::VERTEX
        });

        let mut indices = vec![];
        let per_face= [0, 1, 2, 0, 2, 3u16];
        for face in 0..6u16 {
            let offset = face * 4;
            for i in &per_face {
                indices.push(offset + i);
            }
        }

        let ibo = ws.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsage::INDEX
        });

        let shader = graphics::load_shader(&ws.device, "shader/light_forward.shader.json");
        let vert_uniform_layout = shader.layout_config.iter()
            .find(|x| match x.ty {
                graphics::UniformBindingType::DataBlock { .. } => true,
                _ => false
            })
            .unwrap();
        let float_count: usize = match &vert_uniform_layout.ty  {
            graphics::UniformBindingType::DataBlock { members } => {
                members.iter()
                    .map(|x| x.1.element_count())
                    .sum()
            },
            _ => unreachable!()
        };

        let texture = graphics::load_texture(&*ws, "texture/metal_box.tex.json");

        let float_count = float_count as usize;
        let ubo = ws.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vec![0.0f32; float_count]),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST
            }
        );
        let bind_group = ws.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &shader.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(ubo.slice(..))
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture.default_view)
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler)
                }
            ],
            label: None
        });

        let pipeline_layout = ws.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&shader.bind_group_layout],
            push_constant_ranges: &[]
        });

        let pipeline = ws.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor { module: &shader.vertex, entry_point: "main" },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor { module: &shader.fragment, entry_point: "main" }),
            rasterization_state: None,
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: ws.sc_desc.format,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[get_vertex!(BoxVertex)]
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false
        });

        const OFFSET: f32 = 2.0;
        Self {
            vbo,
            ibo,
            ubo,
            bind_group,
            pipeline,
            box_poses: vec![
                BoxInstance {
                    pos: vec3(0., 0., 0.),
                    crl: Color::white()
                },
                BoxInstance {
                    pos: vec3(OFFSET, 0., 0.),
                    crl: Color::rgb(1., 0., 0.),
                },
                BoxInstance {
                    pos: vec3(0., OFFSET, 0.),
                    crl: Color::rgb(0., 1., 0.),
                },
                BoxInstance {
                    pos: vec3(0., 0., OFFSET),
                    crl: Color::rgb(0., 0., 1.),
                }
            ]
        }
    }

}

impl<'a> System<'a> for DrawBoxSystem {
    type SystemData = ReadExpect<'a, WgpuState>;

    fn run(&mut self, ws: Self::SystemData) {
        use std::mem;
        graphics::with_render_data(|r| {
            let cam_infos = &mut r.camera_infos;
            for cam_info in cam_infos {
                for box_inst in &self.box_poses {
                    let local_to_world = Mat4::from_translation(box_inst.pos);
                    let wvp_mat_arr: [f32; 16] = math::mat::to_array(cam_info.wvp_matrix * local_to_world);
                    let crl_arr: [f32; 4] = box_inst.crl.into();
                    let mut ubo_arr = [0.0f32; 19];
                    ubo_arr[..16].clone_from_slice(&wvp_mat_arr);
                    ubo_arr[16..].clone_from_slice(&crl_arr[..3]);

                    let tmp_ubo_buf = ws.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&ubo_arr),
                            usage: wgpu::BufferUsage::COPY_SRC
                        }
                    );
                    cam_info.encoder.copy_buffer_to_buffer(&tmp_ubo_buf, 0,
                                                           &self.ubo, 0, mem::size_of::<[f32; 19]>() as wgpu::BufferAddress);

                    {
                        let mut render_pass = cam_info.render_pass(&*ws);
                        render_pass.set_pipeline(&self.pipeline);
                        render_pass.set_bind_group(0, &self.bind_group, &[]);
                        render_pass.set_vertex_buffer(0, self.vbo.slice(..));
                        render_pass.set_index_buffer(self.ibo.slice(..));
                        render_pass.draw_indexed(0..36, 0, 0..1);
                    }
                }
            }
        });
    }
}


struct CameraControlSystem {
    pub yaw: Rad,
    pub pitch: Rad,
}

impl CameraControlSystem {
    fn new() -> Self {
        Self {
            yaw: Zero::zero(),
            pitch: Zero::zero()
        }
    }
}

impl<'a> System<'a> for CameraControlSystem {
    type SystemData = (ReadExpect<'a, Time>, ReadExpect<'a, RawInputData>, WriteStorage<'a, Transform>, ReadStorage<'a, graphics::Camera>);

    fn run(&mut self, (time, input, mut trans_write, cam_read): Self::SystemData) {
        const ROTATE_SENSITIVITY: f32 = 0.01;
        const MOVE_SENSITIVITY: f32 = 2.0;

        let mouse_movement = input.mouse_frame_movement;

        for (trans, _) in (&mut trans_write, &cam_read).join() {
            self.yaw += cgmath::Rad(ROTATE_SENSITIVITY * mouse_movement.x); // Yaw
            self.pitch += cgmath::Rad(ROTATE_SENSITIVITY * mouse_movement.y); // Pitch

            let rot_basis = cgmath::Basis3::from_angle_x(self.pitch) * cgmath::Basis3::from_angle_y(self.yaw);
            trans.rot = rot_basis.into();

            fn map_axis(bs: ButtonState, negate: bool) -> f32 {
                if bs.is_down() {
                    if negate { -1. } else { 1. }
                } else {
                    0.
                }
            }
            let fwd_axis = map_axis(input.get_key(VirtualKeyCode::W), false) +
                                map_axis(input.get_key(VirtualKeyCode::S), true);
            let side_axis = map_axis(input.get_key(VirtualKeyCode::D), false) +
                map_axis(input.get_key(VirtualKeyCode::A), true);
            let axis = vec2(fwd_axis, side_axis);
            if axis.magnitude2() > 0.1 {
                let axis = axis.normalize();
                let dt = time.get_delta_time();
                let fwd = quat::get_forward_dir(trans.rot);
                log::info!("fwd {:?}", fwd);
                let right = quat::get_right_dir(trans.rot);
                trans.pos += (axis.x * dt * MOVE_SENSITIVITY) * fwd;
                trans.pos += (axis.y * dt * MOVE_SENSITIVITY) * right;
                // trans.pos.z += 0.1 * dt;
            }
        }
    }
}

struct MyModule;

impl Module for MyModule {

    fn init(&self, ctx: &mut InitContext) {
        ctx.dispatch(Default::default(), |_, i| i.insert(CameraControlSystem::new()));

        let insert_info =
            InsertInfo::new("box")
                .after(&[graphics::DEP_CAM_DRAW_SETUP])
                .before(&[graphics::DEP_CAM_DRAW_TEARDOWN]);
        ctx.dispatch_thread_local(
            insert_info,
            move |init, i|
                i.insert_thread_local(DrawBoxSystem::new(&*init.world.read_resource())));
    }

    fn start(&self, ctx: &mut StartContext) {
        // Camera
        ctx.world.create_entity()
            .with(graphics::Camera {
                projection: graphics::CameraProjection::Perspective {
                    fov: 60.0,
                    z_far: 1000.,
                    z_near: 0.01,
                },
                clear_color: Some(Color::black()),
                clear_depth: true
            })
            .with(Transform {
                rot: Quaternion::one(),
                pos: vec3(0., 0., 5.)
            })
            .build();
    }
}

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Free look & light")
        .add_game_module(graphics::GraphicsModule)
        .add_game_module(MyModule)
        .build();

    runtime.start();
}