use crate::ecs::{HasParent, Transform};
use specs::prelude::*;
use std::collections::HashMap;
use serde_json::Value;

pub struct EntityLoadContext {
    pub entity_mapping: HashMap<u32, Entity>
}

pub trait ComponentS11n {
    fn load(data: Value, ctx: &EntityLoadContext) -> Self;
}


struct EntityLoadRequest {
    path: String,
}

// 接下来应该是自动生成的代码，暂时用手写模拟效果，先跑通流程
#[derive(SystemData)]
struct AllComponentsWrite<'a> {
    pub has_parent_write: WriteStorage<'a, HasParent>,
    pub transform_write:  WriteStorage<'a, Transform>,
}

struct DefaultSerializeSystem {

}

impl<'a> System<'a> for DefaultSerializeSystem {
    type SystemData = AllComponentsWrite<'a>;

    fn run(&mut self, mut cmpt_write: Self::SystemData) {
    }
}

// AutoGen End

pub struct DefaultSerializeModule;