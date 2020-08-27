use crate::ecs::{HasParent, Transform};
use specs::prelude::*;
use serde_json::Value;
use crate::asset::*;

pub struct EntityLoadContext {
    pub entities: Vec<Entity>
}

pub trait ComponentS11n {
    fn load(data: Value, ctx: &EntityLoadContext) -> Self;
}


struct EntityLoadRequest {
    path: String,
}

type EntityLoadRequests = Vec<EntityLoadRequest>;

// 接下来应该是自动生成的代码，暂时用手写模拟效果，先跑通流程
#[derive(SystemData)]
struct AllComponentsWrite<'a> {
    pub has_parent_write: WriteStorage<'a, HasParent>,
    pub transform_write:  WriteStorage<'a, Transform>,
}

struct DefaultSerializeSystem {

}

impl DefaultSerializeSystem {

    fn write_components<'a>(v: Value, entity: Entity, ctx: &EntityLoadContext, cmpt_write: &mut AllComponentsWrite<'a>) {
        match v {
            Value::Object(mut m) => {
                match m.remove("Transform") {
                    Some(cmpt_val) => {
                        let t: Transform = ComponentS11n::load(cmpt_val, &ctx);
                        cmpt_write.transform_write.insert(entity, t).expect("Write Transform failed");
                    },
                    _ => ()
                }
                match m.remove("HasParent") {
                    Some(cmpt_val) => {
                        let t: HasParent = ComponentS11n::load(cmpt_val, &ctx);
                        cmpt_write.has_parent_write.insert(entity, t).expect("Write HasParent failed");
                    },
                    _ => ()
                }
            }
            _ => panic!()
        }
    }
}

impl<'a> System<'a> for DefaultSerializeSystem {
    type SystemData = (AllComponentsWrite<'a>, WriteExpect<'a, EntityLoadRequests>, Entities<'a>);

    fn run(&mut self, (mut cmpt_write, mut requests, entities): Self::SystemData) {
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

            let ctx = EntityLoadContext {
                entities: entity_vec
            };

            for (idx, ev) in entity_values.into_iter().enumerate() {
                let entity = ctx.entities[idx].clone();
                Self::write_components(ev, entity, &ctx, &mut cmpt_write);
            }
        }
        requests.clear();
    }

}

// AutoGen End

pub struct DefaultSerializeModule;