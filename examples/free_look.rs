use mu::*;
use mu::client::graphics;
use wgpu::util::DeviceExt;

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

struct DrawBoxSystem {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    ubo: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline
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
            let offset = face * 6;
            for i in &per_face {
                indices.push(offset + i);
            }
        }

        let ibo = ws.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsage::INDEX
        });

        let shader = graphics::load_shader(&ws.device, "shader/light_forward.vert");
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
        let float_count = float_count as wgpu::BufferAddress;

        let ubo_range = 0..(4 * float_count);

        unimplemented!()
        // Self {
        //
        // }
    }

}


fn main() {

}