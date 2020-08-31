// ! Auto-generated code, DO NOT MODIFY!
use crate::*;
use crate::proto::*;
use serde_json::*;
use crate::ecs::Transform;
use crate::client::sprite::SpriteRenderer;
use crate::asset::load_asset;
use futures::task::Poll;
use std::collections::HashMap;
use specs_hierarchy::Hierarchy;

#[derive(SystemData)]
struct DefaultComponentsWrite<'a> {
    pub has_parent_write: WriteStorage<'a, HasParent>,
    pub transform_write:  WriteStorage<'a, Transform>,
    pub sprite_renderer_write: WriteStorage<'a, SpriteRenderer>,
}

#[derive(SystemData)]
struct DefaultComponentsRead<'a> {
    pub has_parent_read: ReadStorage<'a, HasParent>,
    pub transform_read:  ReadStorage<'a, Transform>,
    pub sprite_renderer_read: ReadStorage<'a, SpriteRenderer>,
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

struct DefaultProtoStoreSystem;

impl DefaultProtoStoreSystem {

    fn store_components(entity: Entity, ctx: &ProtoStoreContext<DefaultExtrasData>, cmpt_read: &DefaultComponentsRead) -> Value {
        let mut ret = serde_json::Map::new();
        if let Some(has_parent) = cmpt_read.has_parent_read.get(entity) {
            ret.insert("has_parent".to_string(), ComponentS11n::store(has_parent, ctx));
        }
        if let Some(transform) = cmpt_read.transform_read.get(entity) {
            ret.insert("transform".to_string(), ComponentS11n::store(transform, ctx));
        }
        if let Some(sprite_renderer) = cmpt_read.sprite_renderer_read.get(entity) {
            ret.insert("sprite_renderer".to_string(), ComponentS11n::store(sprite_renderer, ctx));
        }

        Value::Object(ret)
    }

}

fn _walk(v: &mut Vec<Entity>, h: &Hierarchy<HasParent>, e: Entity) {
    v.push(e);
    for child in h.children(e) {
        _walk(v, h, *child);
    }
}

impl<'a> System<'a> for DefaultProtoStoreSystem {
    type SystemData = (DefaultComponentsRead<'a>,
                       WriteExpect<'a, ProtoStoreRequests>,
                       ReadExpect<'a, Hierarchy<HasParent>>,
                       WriteExpect<'a, ResManager>,
                       DefaultExtrasData<'a>);

    fn run(&mut self, (cmpt_read, mut requests, hierarchy, res_mgr, extras) : Self::SystemData) {
        for req in &*requests {
            // respect hierarchy, add all childrens in the vec
            let mut flattened_entities = vec![];
            for entity in &req.entity_vec {
                _walk(&mut flattened_entities, &*hierarchy, *entity);
            }

            // Remove duplicated entities
            flattened_entities.sort();
            flattened_entities.dedup();

            let mapping = flattened_entities.iter()
                .enumerate()
                .map(|(idx, item)| (item.clone(), idx))
                .collect::<HashMap<_, _>>();

            let ctx = ProtoStoreContext {
                entity_to_index: &mapping,
                resource_mgr: &*res_mgr,
                extras: &extras
            };

            let json_values = flattened_entities.iter()
                .map(|entity| Self::store_components(*entity, &ctx, &cmpt_read))
                .collect::<Vec<_>>();

            let result = serde_json::to_string(&Value::Array(json_values))
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
                .and_then(|s| std::fs::write(&req.path, s.as_bytes()));

            *req.result.lock().unwrap() = Poll::Ready(result);
        }

        requests.clear();
    }
}

struct DefaultProtoLoadSystem;

impl DefaultProtoLoadSystem {

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

impl<'a> System<'a> for DefaultProtoLoadSystem {
    type SystemData = (DefaultComponentsWrite<'a>,
                       WriteExpect<'a, ProtoLoadRequests>,
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

pub struct DefaultSerializeModule;

impl Module for DefaultSerializeModule {
    fn init(&self, ctx: &mut InitContext) {
        ctx.init_data.world.insert(ProtoLoadRequests::new());
        ctx.init_data.world.insert(ProtoStoreRequests::new());
        ctx.group_normal.dispatch(
            InsertInfo::default(),
            |_, i| i.insert(DefaultProtoLoadSystem)
        );
        ctx.group_normal.dispatch(
            InsertInfo::default(),
            |_, i| i.insert(DefaultProtoStoreSystem)
        );
    }
}
