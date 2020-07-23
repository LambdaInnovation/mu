use mu::client::sprite::{SpriteModule, SpriteRenderer, SpriteRef, SpriteSheetManager};
use mu::client::graphics::{GraphicsModule, Camera, CameraProjection};
use mu::{RuntimeBuilder, Module, StartData, math};
use mu::client::sprite;
use specs::{WorldExt, Builder};
use mu::ecs::Transform;
use mu::util::Color;

struct MyModule;

impl Module for MyModule {
    fn start(&self, start_data: &mut StartData) {
        let sheet_ref = start_data.world.write_resource::<SpriteSheetManager>()
            .load(&start_data.display, "texture/test_grid.sheet.json").unwrap();
        let sprite_ref = SpriteRef::new(sheet_ref, 0);

        start_data.world.create_entity()
            .with(Transform::new().pos(math::vec3(0., 1., 0.)))
            .with(SpriteRenderer { sprite: sprite_ref.clone(), material: None })
            .build();

        start_data.world.create_entity()
            .with(Transform::new().pos(math::vec3(0.5, -0.3, 0.)))
            .with(SpriteRenderer { sprite: sprite_ref.clone(), material: None })
            .build();

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
    mu::common_init();
    mu::asset::set_base_asset_path("./examples/asset");

    let runtime = RuntimeBuilder::new("Sprite example")
        .add_game_module(GraphicsModule)
        .add_game_module(SpriteModule)
        .add_game_module(MyModule)
        .build();

    runtime.start();
}