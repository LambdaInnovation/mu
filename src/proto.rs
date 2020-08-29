use specs::prelude::*;
use serde_json::Value;
use crate::resource::ResManager;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;

pub struct ProtoLoadContext<'a, Extras> {
    pub entities: &'a Vec<Entity>,
    pub resource_mgr: &'a mut ResManager,
    pub extras: &'a mut Extras
}

pub struct ProtoStoreContext<'a, Extras> {
    pub entity_to_index: &'a HashMap<Entity, usize>,
    pub resource_mgr: &'a mut ResManager,
    pub extras: &'a mut Extras
}

pub trait ComponentS11n<Extras> {
    fn load(data: Value, ctx: &mut ProtoLoadContext<Extras>) -> Self;
    fn store(&self, ctx: &ProtoStoreContext<Extras>) -> Value;
}

impl<T: Serialize + DeserializeOwned + Clone, Extras> ComponentS11n<Extras> for T {
    fn load(data: Value, _: &mut ProtoLoadContext<Extras>) -> Self {
        serde_json::from_value(data).unwrap()
    }

    fn store(&self, _: &ProtoStoreContext<Extras>) -> Value {
        serde_json::to_value(self.clone()).expect(&format!("Serialize {} failed", std::any::type_name::<T>()))
    }
}

pub struct EntityLoadRequest {
    pub path: String,
    pub result: Arc<Mutex<Poll< Vec<Entity> >>>
}

impl EntityLoadRequest {

    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            result: Arc::new(Mutex::new(Poll::Pending))
        }
    }

}

pub type EntityLoadRequests = Vec<EntityLoadRequest>;

