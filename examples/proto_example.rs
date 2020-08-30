use mu::log::info;
use mu::*;
use mu::client::graphics::*;
use mu::client::sprite::*;
use specs::prelude::*;
use mu::ecs::Transform;
use mu::util::Color;
use mu::proto::{ProtoLoadRequest, ProtoLoadRequests};
use mu::proto_default::DefaultSerializeModule;
use std::task::Poll;
use std::sync::{Mutex, Arc};

struct MyModule;

// 演示创建一个proto，并监听proto实际创建的实体

struct ListenEntityCreateData {
    result_poll: Option<Arc<Mutex<Poll< Vec<Entity> >>>>
}

struct ListenEntityCreatedSystem {}

impl<'a> System<'a> for ListenEntityCreatedSystem {
    type SystemData = WriteExpect<'a, ListenEntityCreateData>;

    fn run(&mut self, mut data: Self::SystemData) {
        let mut should_remove = false;
        if let Some(result_poll) = &data.result_poll {
            let cur_result = result_poll.lock().unwrap();
            match &*cur_result {
                Poll::Ready(entities) => {
                    info!("Entity loaded! {:?}", entities[0]);
                    should_remove = true;
                }
                _ => ()
            }
        }

        if should_remove {
            data.result_poll = None;
        }
    }
}

impl Module for MyModule {
    fn init(&self, ctx: &mut InitContext) {
        ctx.dispatch(InsertInfo::default(), |_, i| i.insert(ListenEntityCreatedSystem {}));
    }

    fn start(&self, start_data: &mut StartContext) {
        let req = ProtoLoadRequest::new("proto/test_proto.json");
        let result = req.result.clone();

        start_data.world.write_resource::<ProtoLoadRequests>()
            .push(req);

        start_data.world.insert(ListenEntityCreateData {
            result_poll: Some(result)
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