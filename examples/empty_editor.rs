use mu::{RuntimeBuilder, Module, InitData, InsertInfo};
use mu::client::editor;
use mu::client::graphics;
use mu::client::graphics::GraphicsModule;
use mu::client::editor::EditorModule;
use specs::System;
use glium::Surface;

struct MyRenderSystem;

impl<'a> System<'a> for MyRenderSystem {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        graphics::with_render_data(|f| {
            f.frame.clear_color_and_depth((0., 0., 0., 0.), 0.)
        });
    }
}

struct MyModule;

impl Module for MyModule {
    fn init(&self, init_data: &mut InitData) {
        init_data.dispatch_thread_local(
            InsertInfo::new("app").before(&[editor::DEP_SETUP]).after(&[graphics::DEP_RENDER_SETUP]),
            |f| f.insert_thread_local(MyRenderSystem));
    }
}

fn main() {
    mu::common_init();
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Textured Quad")
        .add_game_module(GraphicsModule)
        .add_game_module(EditorModule)
        .add_game_module(MyModule)
        .build();

    runtime.start();
}