use mu::*;
use mu::client::graphics::*;
use mu::client::sprite::*;
use mu::resource::*;
use specs::{WorldExt, Builder};
use mu::ecs::Transform;
use mu::util::Color;

struct MyModule;

impl Module for MyModule {
    fn start(&self, start_data: &mut StartContext) {
        let (spr_main, spr_lu, spr_ld, spr_ru, spr_rd) = {
            let mut res_mgr = start_data.world.write_resource::<ResManager>();

            let wgpu_state = (*start_data.wgpu_state).borrow();
            let sheet_ref = load_sprite_sheet(&mut res_mgr, &wgpu_state, "texture/test_grid.sheet.json")
                .unwrap();

            let sprite_ref = SpriteRef::new(&sheet_ref, 0);
            let sprite_ref_lu = SpriteRef::from_name(&res_mgr, &sheet_ref, "LU").unwrap();
            let sprite_ref_ld = SpriteRef::from_name(&res_mgr, &sheet_ref, "LD").unwrap();
            let sprite_ref_ru = SpriteRef::from_name(&res_mgr, &sheet_ref, "RU").unwrap();
            let sprite_ref_rd = SpriteRef::from_name(&res_mgr, &sheet_ref, "RD").unwrap();

            (sprite_ref, sprite_ref_lu, sprite_ref_ld, sprite_ref_ru, sprite_ref_rd)
        };

        const OFFSET: f32 = 0.5;
        start_data.world.create_entity()
            .with(Transform::new().pos(math::vec3(-OFFSET, OFFSET, 0.)))
            .with(SpriteRenderer::new(spr_lu.clone()))
            .build();

        start_data.world.create_entity()
            .with(Transform::new().pos(math::vec3(-OFFSET, -OFFSET, 0.)))
            .with(SpriteRenderer::new(spr_ld.clone()))
            .build();

        start_data.world.create_entity()
            .with(Transform::new().pos(math::vec3(OFFSET, -OFFSET, 0.)))
            .with(SpriteRenderer::new(spr_rd.clone()))
            .build();

        start_data.world.create_entity()
            .with(Transform::new().pos(math::vec3(OFFSET, OFFSET, 0.)))
            .with(SpriteRenderer::new(spr_ru.clone()))
            .build();

        start_data.world.create_entity()
            .with(Transform::new().pos(math::vec3(1.5, 0., 0.)))
            .with(SpriteRenderer::new(spr_main.clone()))
            .build();

        {
            let mut sr = SpriteRenderer::new(spr_main.clone());
            sr.color.g = 0.1;
            sr.color.b = 0.1;

            start_data.world.create_entity()
                .with(Transform::new().pos(math::vec3(-1.5, 0., 0.)))
                .with(sr)
                .build();
        }

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

    let runtime = RuntimeBuilder::new("Sprite example")
        .add_game_module(GraphicsModule)
        .add_game_module(SpriteModule)
        .add_game_module(MyModule)
        .build();

    runtime.start();
}