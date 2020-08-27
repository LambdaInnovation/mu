use crate::ecs::{HasParent, Transform};
use specs::prelude::*;
use serde_json::Value;
use crate::asset::*;
use crate::client::sprite::{SpriteRenderer};
use crate::resource::ResManager;
use crate::{WgpuState, Module, InitContext, InsertInfo};

pub struct EntityLoadContext<'a> {
    pub entities: Vec<Entity>,
    pub resource_mgr: &'a mut ResManager,
    pub wgpu_state: &'a WgpuState
}

pub trait ComponentS11n {
    fn load(data: Value, ctx: &mut EntityLoadContext) -> Self;
}


pub struct EntityLoadRequest {
    pub path: String,
}

pub type EntityLoadRequests = Vec<EntityLoadRequest>;

// 接下来应该是自动生成的代码，暂时用手写模拟效果，先跑通流程
#[derive(SystemData)]
struct AllComponentsWrite<'a> {
    pub has_parent_write: WriteStorage<'a, HasParent>,
    pub transform_write:  WriteStorage<'a, Transform>,
    pub sprite_renderer_write: WriteStorage<'a, SpriteRenderer>,
}

struct DefaultSerializeSystem {
}

impl DefaultSerializeSystem {

    fn write_components<'a>(v: Value, entity: Entity, ctx: &mut EntityLoadContext, cmpt_write: &mut AllComponentsWrite<'a>) {
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

impl<'a> System<'a> for DefaultSerializeSystem {
    type SystemData = (AllComponentsWrite<'a>,
        WriteExpect<'a, EntityLoadRequests>,
        Entities<'a>, WriteExpect<'a, ResManager>,
        ReadExpect<'a, WgpuState>);

    fn run(&mut self, (mut cmpt_write, mut requests, entities, mut res_mgr, wgpu_state): Self::SystemData) {
        for request in &*requests {
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

            let mut ctx = EntityLoadContext {
                entities: entity_vec,
                resource_mgr: &mut *res_mgr,
                wgpu_state: &*wgpu_state
            };

            for (idx, ev) in entity_values.into_iter().enumerate() {
                let entity = ctx.entities[idx].clone();
                Self::write_components(ev, entity, &mut ctx, &mut cmpt_write);
            }
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
            |_, i| i.insert(DefaultSerializeSystem {})
        );
    }
}