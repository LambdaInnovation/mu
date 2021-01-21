use mu::*;
use mu::asset::*;
use mu::client::graphics::{GraphicsModule, Camera, CameraProjection};
use mu::client::text::{TextModule, WorldText, FontInitData, FontArc};
use specs::prelude::*;
use mu::ecs::Transform;
use mu::util::Color;
use std::collections::HashMap;
use mu::math::*;
use wgpu_glyph::{Layout, HorizontalAlign, VerticalAlign};

struct MyModule;

impl Module for MyModule {
    fn init(&self, ctx: &mut InitContext) {
        let mut fonts = HashMap::new();
        fonts.insert("Default".to_string(), load_asset::<FontArc>("Inconsolata-Regular.ttf").unwrap());
        fonts.insert("Chn".to_string(), load_asset::<FontArc>("SourceHanSansSC-Normal.otf").unwrap());
        ctx.init_data.world.insert(FontInitData {
            fonts
        });
    }

    fn start(&self, start_data: &mut StartContext) {
        start_data.world.create_entity()
            .with(Transform::new())
            .with(Camera { projection: CameraProjection::Orthographic { size: 2., z_near: -1., z_far: 1. },
                clear_color: Some(Color::rgb(0.5, 0.5, 0.5)),
                clear_depth: false,
                ..Default::default()
            })
            .build();

        start_data.world.create_entity()
            .with(Transform::new().pos(vec3(0., 0., 0.)))
            .with(WorldText {
                text: "Now I am become death, the destroyer of worlds".to_string(),
                sz: 0.1,
                color: Color::white(),
                layout: Layout::default().h_align(HorizontalAlign::Center).v_align(VerticalAlign::Center),
                font: "Default".to_string()
            })
            .build();

        start_data.world.create_entity()
            .with(Transform::new().pos(vec3(0., -0.5, 0.)))
            .with(WorldText {
                text: "道可道非常道名可名非常名".to_string(),
                sz: 0.15,
                color: Color::white(),
                layout: Layout::default().h_align(HorizontalAlign::Center).v_align(VerticalAlign::Center),
                font: "Chn".to_string()
            })
            .build();
    }
}

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");
    let runtime = RuntimeBuilder::new("UI Test")
        .add_game_module(GraphicsModule)
        .add_game_module(TextModule)
        .add_game_module(MyModule)
        .build();

    runtime.start();
}
