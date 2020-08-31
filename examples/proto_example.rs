use mu::log::info;
use mu::*;
use mu::client::graphics::*;
use mu::client::sprite::*;
use mu::client::editor::*;
use specs::prelude::*;
use mu::ecs::Transform;
use mu::util::Color;
use mu::proto::*;
use mu::proto_default::DefaultSerializeModule;
use std::task::Poll;
use std::sync::{Mutex, Arc};
use imgui::*;

struct MyModule;

// 演示创建一个proto，并监听proto实际创建的实体，再回填到json中

struct MyGuiSystem {
    target_path: ImString,
}

impl<'a> System<'a> for MyGuiSystem {
    type SystemData = (ReadExpect<'a, ListenEntityCreateData>, WriteExpect<'a, ProtoStoreRequests>);

    fn run(&mut self, (create_info, mut store_requests): Self::SystemData) {
        with_frame(|frame| {
            Window::new(im_str!("Save proto as"))
                .build(frame, || {
                frame.input_text(im_str!("Target Path"), &mut self.target_path).build();
                if frame.small_button(im_str!("Save")) {
                    if let Poll::Ready(loaded_entities) = &*create_info.result_poll.lock().unwrap() {
                        store_requests.push(ProtoStoreRequest::new(loaded_entities, &self.target_path.to_string()));
                    } else {
                        info!("Proto not yet created");
                    }
                }
            });
        });
    }
}

struct ListenEntityCreateData {
    result_poll: Arc<Mutex<Poll< Vec<Entity> >>>
}

impl Module for MyModule {
    fn init(&self, ctx: &mut InitContext) {
        ctx.dispatch_thread_local(
            InsertInfo::default().before(&[DEP_IMGUI_TEARDOWN]).after(&[DEP_IMGUI_SETUP]),
            |_, i| i.insert_thread_local(MyGuiSystem { target_path: ImString::new("proto_stored.json") })
        );
    }

    fn start(&self, start_data: &mut StartContext) {
        let req = ProtoLoadRequest::new("proto/test_proto.json");
        let result = req.result.clone();

        start_data.world.write_resource::<ProtoLoadRequests>()
            .push(req);

        start_data.world.insert(ListenEntityCreateData {
            result_poll: result
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
        .add_game_module(EditorModule { asset_path: None })
        .build();

    runtime.start();
}