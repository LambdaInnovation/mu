use specs::prelude::*;
use serde_json::Value;
use crate::resource::ResManager;

pub struct ProtoLoadContext<'a, Extras> {
    pub entities: Vec<Entity>,
    pub resource_mgr: &'a mut ResManager,
    pub extras: Extras
}

impl<'a, Extras> ProtoLoadContext<'a, Extras> {

    pub(crate) fn finish(self) -> Extras {
        self.extras
    }

}

pub trait ComponentS11n<Extras> {
    fn load(data: Value, ctx: &mut ProtoLoadContext<Extras>) -> Self;
}

pub struct EntityLoadRequest {
    pub path: String,
}

pub type EntityLoadRequests = Vec<EntityLoadRequest>;

