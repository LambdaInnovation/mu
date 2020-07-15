use mu;
use mu::{RuntimeBuilder, Module, InitData, InsertInfo};
use mu::client::graphics::{GraphicsModule, DEP_RENDER_SETUP, DEP_RENDER_TEARDOWN};
use mu::client::graphics;
use glium::{Program, VertexBuffer, implement_vertex, Display, DrawParameters, uniform, IndexBuffer, Surface};
use glium::backend::Facade;
use specs::System;
use glium::index::{NoIndices, PrimitiveType};
use glium::uniforms::{MagnifySamplerFilter, MinifySamplerFilter};

#[derive(Copy, Clone)]
struct QuadVertex {
    position: [f32; 3],
    uv: [f32; 2]
}

impl QuadVertex {
    fn new(x: f32, y: f32, u: f32, v: f32) -> Self {
        Self {
            position: [x, y, 0.0],
            uv : [u, v]
        }
    }
}

implement_vertex!(QuadVertex, position, uv);

struct DrawQuadSystem {
    program: Program,
    vbo: VertexBuffer<QuadVertex>,
    ibo: IndexBuffer<u16>,
    tex: glium::texture::CompressedSrgbTexture2d
}

impl DrawQuadSystem {

    fn new(display: &Display) -> Self {
        let program = graphics::load_shader(&display, "shader/quad.shader.json");
        let quad = vec![
            QuadVertex::new(-0.5, 0.5, 0., 0.),
            QuadVertex::new(-0.5, -0.5, 0., 1.),
            QuadVertex::new(0.5, -0.5, 1., 1.),
            QuadVertex::new(0.5, 0.5, 1., 0.)
        ];
        let texture = graphics::load_texture(&display, "texture/landscape.tex.json");
        let ibo = IndexBuffer::new(display, PrimitiveType::TrianglesList,
            &[0u16, 1, 2, 0, 2, 3]).unwrap();
        Self {
            program,
            vbo: VertexBuffer::new(display, &quad).unwrap(),
            tex: texture,
            ibo
        }
    }

}

impl<'a> System<'a> for DrawQuadSystem {
    type SystemData = ();

    fn run(&mut self, data: Self::SystemData) {
        graphics::with_render_data(|r| {
            let draw_params: DrawParameters = Default::default();
            let uniforms = uniform! {
                tex: self.tex.sampled().magnify_filter(MagnifySamplerFilter::Linear)
                    .minify_filter(MinifySamplerFilter::LinearMipmapLinear)
            };
            r.frame.clear_color_and_depth((0.1, 0.1, 0.3, 0.0), 0.0);
            r.frame.draw(
                &self.vbo,
                &self.ibo,
                &self.program,
                &uniforms,
                &draw_params).unwrap();
        });
    }
}

struct QuadModule;

impl Module for QuadModule {
    fn init(&self, init_data: &mut InitData) {
        let display_clone = init_data.display.clone();
        let insert_info =
            InsertInfo::new("quad")
                .after(&[DEP_RENDER_SETUP])
                .before(&[DEP_RENDER_TEARDOWN]);
        init_data.dispatch_thread_local(
            insert_info,
            move |i| i.insert_thread_local(DrawQuadSystem::new(&*display_clone)));
    }
}

fn main() {
    mu::common_init();
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Textured Quad")
        .add_game_module(GraphicsModule)
        .add_game_module(QuadModule)
        .build();

    runtime.start();
}