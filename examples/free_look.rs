use mu::*;

#[derive(Copy, Clone)]
struct BoxVertex {
    pos: [f32; 3],
    uv: [f32; 2]
}

impl BoxVertex {
    fn new(x: f32, y: f32, z: f32, u: f32, v: f32) -> Self {
        Self {
            pos: [x, y, z],
            uv: [u, v]
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

    // fn new(ws: &WgpuState) -> Self {
    //
    // }

}


fn main() {

}