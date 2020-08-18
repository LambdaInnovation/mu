use specs::prelude::*;

use mu::*;
use mu::log::*;
use mu::client::graphics::GraphicsModule;
use mu::client::ui::*;
use mu::ecs::HasParent;
use mu::math::{vec2};
use mu::util::Color;
use mu::client::sprite::*;
use mu::resource::{ResourceRef, ResManager};

struct TestDialogComponent {
    btn_ok: Entity
}

impl TestDialogComponent {

    pub fn create_dialog(world: &mut World, canvas: Entity) {
        let mut image0 = Image::new();
        image0.color = Color::rgb(0.8, 0.8, 0.6);
        let ent_window = world.create_entity()
            .with(HasParent::new(canvas))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 0., 640.))
                .with_layout_y(LayoutType::normal(AlignType::Middle, 0., 480.))
            )
            .with(image0)
            .build();

        let mut image1 = Image::new();
        image1.color = Color::rgb(0.6, 0.6, 0.4);
        let _ent_header = world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_pivot(vec2(0.5, 1.))
                .with_layout_x(LayoutType::expand(0., 0.))
                .with_layout_y(LayoutType::normal(AlignType::Max, 0., 80.))
            )
            .with(image1)
            .build();

        let mut image1 = Image::new();
        image1.color = Color::rgb(0.8, 0.2, 0.2);
        let ent_button = world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 0., 300.))
                .with_layout_y(LayoutType::normal(AlignType::Min, 60., 60.))
                .with_raycast()
            )
            .with(image1)
            .build();

        let mut image2 = Image::new();
        let sprite_ref = world.read_resource::<DialogResources>().default_sprite.clone();
        image2.sprite = Some(sprite_ref);
        world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 0., 200.))
                .with_layout_y(LayoutType::normal(AlignType::Middle, 5., 200.))
            )
            .with(image2)
            .build();

        world.write_component::<TestDialogComponent>().insert(ent_window, TestDialogComponent {
            btn_ok: ent_button
        }).unwrap();
    }

}

impl Component for TestDialogComponent {
    type Storage = HashMapStorage<Self>;
}

struct TestDialogSystem;

impl<'a> System<'a> for TestDialogSystem {
    type SystemData = (ReadStorage<'a, TestDialogComponent>, ReadExpect<'a, WidgetEvents>);

    fn run(&mut self, (dialogs, events): Self::SystemData) {
        for dlg in (&dialogs).join() {
            for ev in &events.events {
                match ev {
                    WidgetEvent::Clicked { entity, .. } if *entity == dlg.btn_ok => {
                        info!("OK btn clicked!");
                    }
                    _ => ()
                }
            }
        }
    }
}

struct MyModule;

struct DialogResources {
    default_sprite: SpriteRef
}

impl Module for MyModule {
    fn init(&self, init_ctx: &mut InitContext) {
        init_ctx.dispatch(InsertInfo::new(""),
            |_, i| {
                i.insert(TestDialogSystem);
            });
    }

    fn start(&self, ctx: &mut StartContext) {
        let wgpu_state = ctx.wgpu_state.borrow();
        let sprite_sheet_ref = {
            let mut res_mgr = ctx.world.write_resource::<ResManager>();
            load_sprite_sheet(&mut *res_mgr, &wgpu_state, "texture/kasumi.sheet.json").unwrap()
        };
        let sprite_ref = SpriteRef::new(&sprite_sheet_ref, 0);

        ctx.world.insert(DialogResources {
            default_sprite: sprite_ref
        });

        let ent_canvas = ctx.world.create_entity()
            .with(Canvas::new(0, RefResolution::new(1920, 1080, 0.5)))
            .build();

        TestDialogComponent::create_dialog(&mut ctx.world, ent_canvas);
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