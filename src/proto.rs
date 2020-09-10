use specs::prelude::*;
use serde_json::Value;
use crate::resource::ResManager;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use specs::shred::DynamicSystemData;
use crate::asset::load_asset;
use std::future::Future;
use std::pin::Pin;

pub enum ComponentLoadState {
    Init(Value),
    Processing,
    Finished,
    Integrate(Entity),
    Finalize
}

pub type ProtoLoadResult = Arc<Mutex<Poll <Vec<Entity>> >>;

pub struct LoadingEntity {
    components: HashMap<String, ComponentLoadState>
}

pub struct ProtoLoadContext {
    idx: u32, // An unique id to distinguish between load requests
    loading_entities: Vec<LoadingEntity>,
    result: ProtoLoadResult
}

pub struct ProtoLoadContexts {
    v: Vec<ProtoLoadContext>,
    counter: u32,
}

impl ProtoLoadContexts {

    pub fn push_request(&mut self, path: &str) -> ProtoLoadResult {
        let s: String = load_asset(path).unwrap();
        let value: Value = serde_json::from_str(&s).unwrap();

        let loading_entities = match value {
            Value::Array(v) => {
                v.into_iter()
                    .map(|entity_value| {
                        match entity_value {
                            Value::Object(m) => {
                                let mut components = HashMap::new();
                                for (k, v) in m {
                                    components.insert(k, ComponentLoadState::Init(v));
                                }
                                LoadingEntity {
                                    components
                                }
                            }
                            _ => panic!("Invalid entity data type, expecting object")
                        }
                    })
                    .collect::<Vec<_>>()
            }
            _ => panic!("Invalid root type")
        };

        let arc = Arc::new(Mutex::new( Poll::Pending ));
        let ctx = ProtoLoadContext {
            idx: self.counter,
            loading_entities,
            result: arc.clone()
        };
        self.v.push(ctx);

        arc
    }

}

pub struct ComponentPostIntegrateContext<'a> {
    pub self_idx: usize,
    pub entity_vec: &'a Vec<Entity>,
}

pub trait ComponentS11n<'a, T> where T: Component {
    type SystemData: DynamicSystemData<'a>;

    /// Load the data from given json value. It allows you to read the system data and then
    /// do the other job in the async fasion.
    fn load(data: &Value, system_data: &mut Self::SystemData) -> Pin<Box<dyn Future<Output = T>>>;

    /// Invoked after all entities' components load complete, and just before this component is inserted
    /// into the storage.
    ///
    /// Usually used to gather cross-entity references.
    fn integrate(_instance: &mut T, _ctx: ComponentPostIntegrateContext) {}

    /// Get type name literal used in json representation.
    /// We can use std::any::type_name, but that has no stability guarantee.
    fn type_name() -> &'static str;
}

struct ComponentStagingData<T> where T: Component {
    staging_components: HashMap<(u32, usize), Arc<Mutex<Poll<T>>>>
}

impl<'a, C, T> System<'a> for T where C: ComponentS11n<'a, T>, T: Component {
    type SystemData = (
        WriteExpect<'a, ProtoLoadContexts>,
        WriteExpect<'a, ComponentStagingData<T>>,
        WriteStorage<'a, T>,
        C::SystemData);

    fn run(&mut self, (mut proto_loads, mut staging_data, mut cmpt_write, mut data): Self::SystemData) {
        for entry in &mut proto_loads.v {
            for (idx, ent) in entry.loading_entities.iter_mut().enumerate() {
                if let Some(state) = ent.components.get_mut(C::type_name()) {
                    let next_state = match state {
                        ComponentLoadState::Init(v) => { // Init时，目前直接调用序列化代码
                            let arc = Arc::new(Mutex::new(Poll::Pending));
                            let arc_clone = arc.clone();
                            async move {
                                let cmpt = C::load(&*v, &mut data).await;
                                (*arc_clone.lock()) = Poll::Ready(cmpt);
                            }
                            staging_data.staging_components.insert((entry.idx, idx), arc);
                            Some(ComponentLoadState::Processing)
                        },
                        ComponentLoadState::Processing => {
                            // TODO
                        },
                        ComponentLoadState::Integrate(e) => {
                            let cmpt = staging_data.staging_components.remove(&(entry.idx, idx)).unwrap();
                            cmpt_write.insert(*e, cmpt);

                            Some(ComponentLoadState::Finalize)
                        },
                        _ => None,
                    };
                    if let Some(next) = next_state {
                        *state = next;
                    }
                }
            }
        }
    }
}

struct EntityLoadSystem;

impl<'a> System<'a> for EntityLoadSystem {
    type SystemData = ();

    fn run(&mut self, data: Self::SystemData) {
        unimplemented!()
    }
}

// pub struct ProtoLoadContext<'a, Extras> {
//     pub entities: &'a Vec<Entity>,
//     pub resource_mgr: &'a mut ResManager,
//     pub extras: &'a mut Extras
// }
//
// pub struct ProtoStoreContext<'a, Extras> {
//     pub entity_to_index: &'a HashMap<Entity, usize>,
//     pub resource_mgr: &'a ResManager,
//     pub extras: &'a Extras,
// }
//
// pub trait ComponentS11n<Extras> {
//     fn load(data: Value, ctx: &mut ProtoLoadContext<Extras>) -> Self;
//     fn store(&self, ctx: &ProtoStoreContext<Extras>) -> Value;
// }
//
// impl<T: Serialize + DeserializeOwned + Clone, Extras> ComponentS11n<Extras> for T {
//     fn load(data: Value, _: &mut ProtoLoadContext<Extras>) -> Self {
//         serde_json::from_value(data).unwrap()
//     }
//
//     fn store(&self, _: &ProtoStoreContext<Extras>) -> Value {
//         serde_json::to_value(self.clone()).expect(&format!("Serialize {} failed", std::any::type_name::<T>()))
//     }
// }
//
//
// impl ProtoLoadRequest {
//
//     pub fn new(path: &str) -> Self {
//         Self {
//             path: path.to_string(),
//             result: Arc::new(Mutex::new(Poll::Pending))
//         }
//     }
//
// }
//
// pub type ProtoLoadRequests = Vec<ProtoLoadRequest>;
//
// /// A request to store the given entity in the given asset path.
// pub struct ProtoStoreRequest {
//     pub entity_vec: Vec<Entity>,
//     pub path: String,
//     pub result: Arc<Mutex<Poll<std::io::Result<()>>>>
// }
//
// impl ProtoStoreRequest {
//
//     pub fn new(entities: &Vec<Entity>, path: &str) -> Self {
//         Self {
//             entity_vec: entities.clone(),
//             path: path.to_string(),
//             result: Arc::new(Mutex::new(Poll::Pending))
//         }
//     }
//
// }
//
// pub type ProtoStoreRequests = Vec<ProtoStoreRequest>;
