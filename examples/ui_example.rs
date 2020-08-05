use mu::{Module, InitData, StartData, RuntimeBuilder};
use specs::{WorldExt, Builder};
use mu::client::ui::{Canvas, RefResolution, Widget, LayoutType, AlignType, UIModule, Image};
use mu::ecs::HasParent;
use mu::client::graphics::GraphicsModule;
use mu::util::Color;
use mu::math::{Vec2, vec2};

struct MyModule;

impl Module for MyModule {
    fn init(&self, _init_data: &mut InitData) {
    }

    fn start(&self, start_data: &mut StartData) {
        let ent_canvas = start_data.world.create_entity()
            .with(Canvas::new(0, RefResolution::new(1920, 1080, 0.5)))
            .build();

        let mut image0 = Image::new();
        image0.color = Color::rgb(0.8, 0.8, 0.6);
        let ent_window = start_data.world.create_entity()
            .with(HasParent::new(ent_canvas))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 0., 640.))
                .with_layout_y(LayoutType::normal(AlignType::Middle, 0., 480.))
            )
            .with(image0)
            .build();

        let mut image1 = Image::new();
        image1.color = Color::rgb(0.6, 0.6, 0.4);
        let _ent_header = start_data.world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_pivot(vec2(0.5, 1.))
                .with_layout_x(LayoutType::expand(0., 0.))
                .with_layout_y(LayoutType::normal(AlignType::Max, 0., 100.))
            )
            .with(image1)
            .build();

        let mut image1 = Image::new();
        image1.color = Color::rgb(0.8, 0.2, 0.2);
        let _ent_button = start_data.world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 0., 300.))
                .with_layout_y(LayoutType::normal(AlignType::Min, 100., 100.))
                .with_raycast()
            )
            .with(image1)
            .build();
    }
}

fn main() {
    mu::asset::set_base_asset_path("./examples/asset");
    let runtime = RuntimeBuilder::new("UI Test")
        .add_game_module(GraphicsModule)
        .add_game_module(UIModule)
        .add_game_module(MyModule)
        .build();

    runtime.start();
}