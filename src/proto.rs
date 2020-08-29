use specs::prelude::*;
use serde_json::Value;
use crate::resource::ResManager;
use std::sync::{Arc, Mutex};
use std::task::Poll;

pub struct ProtoLoadContext<'a, Extras> {
    pub entities: &'a Vec<Entity>,
    pub resource_mgr: &'a mut ResManager,
    pub extras: &'a mut Extras
}

pub trait ComponentS11n<Extras> {
    fn load(data: Value, ctx: &mut ProtoLoadContext<Extras>) -> Self;
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

