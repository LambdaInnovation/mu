use specs::prelude::*;

use mu::*;
use mu::asset::*;
use mu::log::*;
use mu::client::graphics::GraphicsModule;
use mu::client::ui::*;
use mu::ecs::HasParent;
use mu::math::{vec2};
use mu::util::Color;
use mu::client::sprite::*;
use mu::resource::*;
use mu::client::text::*;
use std::collections::HashMap;

struct TestDialogComponent {
    dialog_root: Entity,
    btn_ok: Entity,
    btn_close: Entity,
    header: Entity
}

impl TestDialogComponent {

    pub fn create_dialog(world: &mut World, canvas: Entity) {
        let resources = world.read_resource::<DialogResources>();
        let close_sprite = resources.close_sprite.clone();
        let kasumi_sprite = resources.default_sprite.clone();
        drop(resources);

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
        let ent_header = world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_pivot(vec2(0.5, 1.))
                .with_layout_x(LayoutType::expand(0., 0.))
                .with_layout_y(LayoutType::normal(AlignType::Max, 0., 80.))
                .with_raycast()
            )
            .with(image1)
            .build();

        let mut text0 = UIText::new();
        text0.text = "Hello world!!!".to_string();
        text0.x_align = AlignType::Min;
        world.create_entity()
            .with(HasParent::new(ent_header))
            .with(Widget::new()
                .with_layout_x(LayoutType::expand(20., 0.))
                .with_layout_y(LayoutType::expand(0., 0.))
            )
            .with(text0)
            .build();

        let mut image1 = Image::new();
        image1.sprite = Some(close_sprite);
        let ent_button_close = world.create_entity()
            .with(HasParent::new(ent_header))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Max, -40., 60.))
                .with_layout_y(LayoutType::normal(AlignType::Middle, 0., 60.))
                .with_raycast())
            .with(image1)
            .with(UIClickTint::new())
            .build();

        let mut image1 = Image::new();
        image1.color = Color::rgb(0.8, 0.2, 0.2);

        let mut tint = UIClickTint::new();
        tint.click_color = Color::rgb(0.8, 0.2, 0.2);
        tint.normal_color = Color::rgb(1.0, 0.4, 0.4);

        let ent_button = world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 0., 300.))
                .with_layout_y(LayoutType::normal(AlignType::Min, 60., 60.))
                .with_raycast()
            )
            .with(tint)
            .with(image1)
            .build();

        let mut text1 = UIText::new();
        text1.text = "Duplicate".to_string();
        text1.x_align = AlignType::Middle;
        let _ent_button_text = world.create_entity()
            .with(HasParent::new(ent_button))
            .with(Widget::new()
                .with_layout_x(LayoutType::expand(0., 0.))
                .with_layout_y(LayoutType::expand(0., 0.))
            )
            .with(text1)
            .build();

        let mut image2 = Image::new();
        image2.sprite = Some(kasumi_sprite);
        world.create_entity()
            .with(HasParent::new(ent_window))
            .with(Widget::new()
                .with_layout_x(LayoutType::normal(AlignType::Middle, 0., 200.))
                .with_layout_y(LayoutType::normal(AlignType::Middle, 5., 200.))
            )
            .with(image2)
            .build();

        world.write_component::<TestDialogComponent>().insert(ent_window, TestDialogComponent {
            dialog_root: ent_window,
            btn_ok: ent_button,
            btn_close: ent_button_close,
            header: ent_header
        }).unwrap();
    }

}

impl Component for TestDialogComponent {
    type Storage = HashMapStorage<Self>;
}

struct TestDialogSystem;

impl<'a> System<'a> for TestDialogSystem {
    type SystemData = (Entities<'a>, ReadStorage<'a, TestDialogComponent>, WriteStorage<'a, Widget>, ReadExpect<'a, WidgetEvents>);

    fn run(&mut self, (entities, dialogs, mut widgets, events): Self::SystemData) {
        for dlg in (&dialogs).join() {
            for ev in &events.events {
                match ev {
                    WidgetEvent::Clicked { entity, btn: 0 } if *entity == dlg.btn_ok => {
                        info!("OK btn clicked!");
                    }
                    WidgetEvent::Clicked { entity, btn: 0 } if *entity == dlg.btn_close => {
                        entities.delete(dlg.dialog_root).unwrap();
                    },
                    WidgetEvent::Drag { entity, delta, btn: 0 } if *entity == dlg.header => {
                        let widget = widgets.get_mut(dlg.dialog_root).unwrap();
                        match widget.layout_x {
                            LayoutType::Normal { align, pos, len } => {
                                widget.layout_x = LayoutType::Normal { align, pos: pos + delta.x, len }
                            }
                            _ => panic!()
                        }
                        match widget.layout_y {
                            LayoutType::Normal { align, pos, len } => {
                                widget.layout_y = LayoutType::Normal { align, pos: pos + delta.y, len }
                            }
                            _ => panic!()
                        }
                        widget._mark_dirty();
                    },
                    _ => ()
                }
            }
        }
    }
}

struct MyModule;

struct DialogResources {
    default_sprite: SpriteRef,
    close_sprite: SpriteRef
}

impl Module for MyModule {
    fn init(&self, ctx: &mut InitContext) {
        let mut fonts = HashMap::new();
        fonts.insert("Default".to_string(), load_asset::<FontArc>("Inconsolata-Regular.ttf").unwrap());
        ctx.init_data.world.insert(FontInitData {
            fonts
        });
        ctx.dispatch(InsertInfo::new(""),
            |_, i| {
                i.insert(TestDialogSystem);
            });
    }

    fn start(&self, ctx: &mut StartContext) {
        let wgpu_state = ctx.world.read_resource::<WgpuState>();
        let sprite_sheet_ref = {
            let mut res_mgr = ctx.world.write_resource::<ResManager>();
            load_sprite_sheet(&mut *res_mgr, &wgpu_state, "texture/kasumi.sheet.json").unwrap()
        };
        let sprite_ref = SpriteRef::new(&sprite_sheet_ref, 0);

        let close_sheet_ref = {
            let mut res_mgr = ctx.world.write_resource::<ResManager>();
            load_sprite_sheet(&mut *res_mgr, &wgpu_state, "texture/close.sheet.json").unwrap()
        };
        let close_sprite_ref = SpriteRef::new(&close_sheet_ref, 0);

        drop(wgpu_state);
        ctx.world.insert(DialogResources {
            default_sprite: sprite_ref,
            close_sprite: close_sprite_ref
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
        .add_game_module(TextModule)
        .add_game_module(UIModule)
        .add_game_module(MyModule)
        .build();

    runtime.start();
}