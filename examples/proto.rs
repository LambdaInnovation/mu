use mu::*;
use mu::client::graphics::*;
use mu::client::sprite::*;
use specs::{WorldExt, Builder};
use mu::ecs::Transform;
use mu::util::Color;
use mu::proto::{DefaultSerializeModule, EntityLoadRequest, EntityLoadRequests};

struct MyModule;

impl Module for MyModule {
    fn start(&self, start_data: &mut StartContext) {
        start_data.world.write_resource::<EntityLoadRequests>()
            .push(EntityLoadRequest {
                path: "proto/test_proto.json".to_string()
            });

        start_data.world.create_entity()
            .with(Transform::new())
            .with(Camera {
                projection: CameraProjection::Orthographic { z_near: -1., z_far: 1., size: 4. } ,
                clear_depth: true,
                clear_color: Some(Color::black())
            })
            .build();
    }
}

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Sprite proto example")
        .add_game_module(GraphicsModule)
        .add_game_module(SpriteModule)
        .add_game_module(MyModule)
        .add_game_module(DefaultSerializeModule)
        .build();

    runtime.start();
}