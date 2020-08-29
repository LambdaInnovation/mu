// ! Auto-generated code, DO NOT MODIFY!
use crate::*;
use crate::proto::*;
use specs::prelude::*;
use serde_json::*;
use crate::ecs::Transform;
use crate::client::sprite::SpriteRenderer;
use crate::asset::load_asset;
use futures::task::Poll;

#[derive(SystemData)]
struct DefaultComponentsWrite<'a> {
    pub has_parent_write: WriteStorage<'a, HasParent>,
    pub transform_write:  WriteStorage<'a, Transform>,
    pub sprite_renderer_write: WriteStorage<'a, SpriteRenderer>,
}

pub trait DefaultExtras {
    fn wgpu_state(&self) -> &WgpuState;
}

#[derive(SystemData)]
struct DefaultExtrasData<'a> {
    pub wgpu_state_read: ReadExpect<'a, WgpuState>
}

impl<'a> DefaultExtras for DefaultExtrasData<'a> {
    fn wgpu_state(&self) -> &WgpuState {
        &*self.wgpu_state_read
    }
}

struct DefaultProtoSystem {
}

impl DefaultProtoSystem {

    fn write_components<'a>(v: Value, entity: Entity, ctx: &mut ProtoLoadContext<DefaultExtrasData>, cmpt_write: &mut DefaultComponentsWrite<'a>) {
        match v {
            Value::Object(mut m) => {
                match m.remove("Transform") {
                    Some(cmpt_val) => {
                        let t: Transform = ComponentS11n::load(cmpt_val, ctx);
                        cmpt_write.transform_write.insert(entity, t).expect("Write Transform failed");
                    },
                    _ => ()
                }
                match m.remove("HasParent") {
                    Some(cmpt_val) => {
                        let t: HasParent = ComponentS11n::load(cmpt_val, ctx);
                        cmpt_write.has_parent_write.insert(entity, t).expect("Write HasParent failed");
                    },
                    _ => ()
                }
                match m.remove("SpriteRenderer") {
                    Some(cmpt_val) => {
                        let t: SpriteRenderer = ComponentS11n::load(cmpt_val, ctx);
                        cmpt_write.sprite_renderer_write.insert(entity, t).expect("Write SpriteRenderer failed");
                    },
                    _ => ()
                }
            }
            _ => panic!()
        }
    }
}

impl<'a> System<'a> for DefaultProtoSystem {
    type SystemData = (DefaultComponentsWrite<'a>,
                       WriteExpect<'a, EntityLoadRequests>,
                       Entities<'a>, WriteExpect<'a, ResManager>,
                       DefaultExtrasData<'a>);

    fn run(&mut self, (mut cmpt_write, mut requests, entities, mut res_mgr, extras): Self::SystemData) {
        let mut extras = extras;
        for request in &mut *requests {
            let mut entity_vec = vec![];

            let value: Value = serde_json::from_str(&load_asset::<String>(&request.path).unwrap()).unwrap();
            let entity_values = match value {
                Value::Array(v) => {
                    for _ in 0..v.len() {
                        entity_vec.push(entities.create());
                    }
                    v
                }
                _ => panic!("Invalid root type")
            };

            let mut ctx = ProtoLoadContext {
                entities: &entity_vec,
                resource_mgr: &mut *res_mgr,
                extras: &mut extras
            };

            for (idx, ev) in entity_values.into_iter().enumerate() {
                let entity = ctx.entities[idx].clone();
                Self::write_components(ev, entity, &mut ctx, &mut cmpt_write);
            }

            *request.result.lock().unwrap() = Poll::Ready(entity_vec);
        }
        requests.clear();
    }

}

// AutoGen End

pub struct DefaultSerializeModule;

impl Module for DefaultSerializeModule {
    fn init(&self, ctx: &mut InitContext) {
        ctx.init_data.world.insert(EntityLoadRequests::new());
        ctx.group_normal.dispatch(
            InsertInfo::new(""),
            |_, i| i.insert(DefaultProtoSystem {})
        );
    }
}
