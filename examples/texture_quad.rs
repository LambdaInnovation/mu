use mu;
use mu::log;
use mu::{RuntimeBuilder, Module, InitData, InsertInfo, StartData};
use mu::client::graphics::{GraphicsModule, DEP_RENDER_SETUP, DEP_RENDER_TEARDOWN, Camera};
use mu::client::graphics;
use glium::{Program, VertexBuffer, implement_vertex, Display, DrawParameters, uniform, IndexBuffer, Surface};
use specs::{System, WorldExt, Builder, ReadExpect, WriteStorage, Join};
use glium::index::{PrimitiveType};
use glium::uniforms::{MagnifySamplerFilter, MinifySamplerFilter};
use mu::ecs::{Transform, Time};
use mu::client::graphics::CameraProjection::Orthographic;
use mu::util::Color;
use mu::client::input::RawInputData;
use glutin::event::VirtualKeyCode;

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

    fn run(&mut self, _: Self::SystemData) {
        graphics::with_render_data(|r| {
            for cam_info in &r.camera_infos {
                let draw_params: DrawParameters = Default::default();
                // log::info!("{:?}", cam_info.wvp_matrix);
                let wvp_mat_arr: [[f32; 4]; 4] = cam_info.wvp_matrix.into();
                let uniforms = uniform! {
                    tex: self.tex.sampled().magnify_filter(MagnifySamplerFilter::Linear)
                        .minify_filter(MinifySamplerFilter::LinearMipmapLinear),
                    wvp_matrix: wvp_mat_arr
                };
                r.frame.clear_color_and_depth((0.1, 0.1, 0.3, 0.0), 0.0);
                r.frame.draw(
                    &self.vbo,
                    &self.ibo,
                    &self.program,
                    &uniforms,
                    &draw_params).unwrap();
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
    fn init(&self, init_data: &mut InitData) {
        init_data.dispatch(
            InsertInfo::new("update_trans"),
            move |i| i.insert(UpdateCameraSystem));

        let display_clone = init_data.display.clone();
        let insert_info =
            InsertInfo::new("quad")
                .after(&[DEP_RENDER_SETUP])
                .before(&[DEP_RENDER_TEARDOWN]);
        init_data.dispatch_thread_local(
            insert_info,
            move |i| i.insert_thread_local(DrawQuadSystem::new(&*display_clone)));
    }

    fn start(&self, start_data: &mut StartData) {
        start_data.world.create_entity()
            .with(Transform::new())
            .with(Camera { projection: Orthographic { size: 2., z_near: -1., z_far: 1. },
                clear_color: Some(Color::black()),
                clear_depth: true
            })
            .build();
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