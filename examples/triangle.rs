use mu::{RuntimeBuilder, InitData, InsertInfo};
use mu::client::graphics::{GraphicsModule, load_shader_simple, DEP_RENDER_TEARDOWN, DEP_RENDER_SETUP};
use mu::client;
use glium::{Display, VertexBuffer, DepthTest, Surface};
use glium::program::ProgramCreationInput;
use mu::glium::Program;
use mu::specs::System;
use specs::ReadStorage;

#[derive(Copy, Clone)]
struct TriangleVertex {
    position: [f32; 3]
}

glium::implement_vertex!(TriangleVertex, position);

struct DrawTriangleSystem {
    program: glium::Program,
    vbo: glium::VertexBuffer<TriangleVertex>,
}

impl DrawTriangleSystem {

    fn new(display: &Display) -> Self {
        let program = load_shader_simple(&display,
            "shader/triangle.vert", "shader/triangle.frag");

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
            vec![v1, v2, v3]
        };

        Self {
            program,
            vbo: VertexBuffer::new(display, &triangle).unwrap(),
        }
    }

}

impl<'a> System<'a> for DrawTriangleSystem {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        client::graphics::with_render_data(|r| {
            let draw_params = glium::DrawParameters {
                depth: glium::Depth {
                    test: DepthTest::Overwrite,
                    write: false,
                    ..Default::default()
                },
                ..Default::default()
            };

            r.frame.draw(
                &self.vbo,
                glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                &self.program,
                &glium::uniform! {},
                &draw_params).unwrap();
        });
    }
}

struct TriangleModule;

impl mu::Module for TriangleModule {
    fn init(&self, init_data: &mut InitData) {
        let display_clone = init_data.display.clone();
        init_data.dispatch_thread_local(InsertInfo::new("triangle")
                                            .after(&[DEP_RENDER_SETUP])
                                            .before(&[DEP_RENDER_TEARDOWN]),
            move |insert| {
                insert.insert_thread_local(DrawTriangleSystem::new(&*display_clone));
            })
    }
}

fn main() {
    mu::common_init();
    mu::asset::set_base_asset_path("./examples/asset");
    let runtime = RuntimeBuilder::new("Hello Triangle")
        .add_game_module(GraphicsModule)
        .add_game_module(TriangleModule)
        .build();

    runtime.start();
}