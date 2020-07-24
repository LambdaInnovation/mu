use mu::{RuntimeBuilder, InitData, InsertInfo};
use mu::client::graphics::{GraphicsModule, DEP_RENDER_TEARDOWN, DEP_RENDER_SETUP, load_shader};
use mu::client;
use glium::{Display, VertexBuffer, DepthTest, Surface};
use specs::System;
use specs::{ReadExpect};
use mu::ecs::Time;
use mu::asset::ResourceRef;
use mu::asset;

#[derive(Copy, Clone)]
struct TriangleVertex {
    position: [f32; 3]
}

glium::implement_vertex!(TriangleVertex, position);

struct DrawTriangleSystem {
    program: ResourceRef<glium::Program>,
    vbo: glium::VertexBuffer<TriangleVertex>,
    elapsed: f32
}

impl DrawTriangleSystem {

    fn new(display: &Display) -> Self {
        let program = load_shader(&display, "shader/triangle.shader.json");

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
            elapsed: 0.0
        }
    }

}

impl<'a> System<'a> for DrawTriangleSystem {
    type SystemData = ReadExpect<'a, Time>;

    fn run(&mut self, time: Self::SystemData) {
        client::graphics::with_render_data(|r| {
            let draw_params = glium::DrawParameters {
                depth: glium::Depth {
                    test: DepthTest::Overwrite,
                    write: false,
                    ..Default::default()
                },
                ..Default::default()
            };

            let dt = (*time).get_delta_time();
            self.elapsed += dt;

            let uniform = glium::uniform! {
                offset: (0.0, 0.5 * f32::sin(2. * self.elapsed), 0.0)
            };

            r.frame.clear_color_and_depth((0.3, 0.1, 0.1, 0.0), 0.0);
            asset::with_local_resource_mgr(|mgr| {
                let program = mgr.get(&self.program);
                r.frame.draw(
                    &self.vbo,
                    glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                    program,
                    &uniform,
                    &draw_params).unwrap();
            });
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