use specs::prelude::*;
use serde_json::Value;
use crate::resource::ResManager;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use crate::asset::load_asset;
use std::future::Future;
use std::pin::Pin;
use futures::executor::{ThreadPool, block_on};
use std::marker::PhantomData;

#[derive(Copy, Clone, PartialEq)]
pub enum ProtoLoadState {
    ComponentLoad,
    Integrate,
    Finalize
}

pub enum ComponentLoadState {
    Init(Value),
    Processing,
    Finished,
    Integrate,
    Finalize
}

pub type ProtoLoadResult = Arc<Mutex<Poll <Vec<Entity>> >>;

pub struct LoadingEntity {
    components: HashMap<String, ComponentLoadState>
}

pub struct ProtoLoadContext {
    idx: u32, // An unique id to distinguish between load requests
    loading_entities: Vec<LoadingEntity>,
    entities: Vec<Entity>, // Empty in the beginning, will be filled just before component Integrate state start.
    state: ProtoLoadState,
    result: ProtoLoadResult
}

pub struct ProtoStoreContext {}

pub struct ProtoLoadContexts {
    v: Vec<ProtoLoadContext>,
    counter: u32,
}

impl ProtoLoadContexts {

    pub fn new() -> Self {
        ProtoLoadContexts {
            v: vec![],
            counter: 0
        }
    }

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
            result: arc.clone(),
            entities: vec![],
            state: ProtoLoadState::ComponentLoad
        };
        self.v.push(ctx);

        arc
    }

}

pub struct ComponentPostIntegrateContext<'a> {
    /// Index of the proto load request.
    pub request_idx: u32,
    /// Index of entity in the loaded entity list.
    pub self_idx: usize,
    pub entity_vec: &'a Vec<Entity>,
}

pub trait ComponentS11n<'a> {
    type SystemData: SystemData<'a>;
    type Output: 'static + Component + Send + Sync;
    type LoadResult: 'static + Sized + Send;

    /// Load the data from given json value. The data is used in the integration process to be converted into
    /// actual component. Can be component itself or other middle representation. The process is async.
    fn load(&mut self, data: Value, system_data: &mut Self::SystemData) -> Pin<Box<dyn Future<Output = Self::LoadResult> + Send + Sync>>;

    /// Convert the load result into component, which will be later inserted into the storage.
    fn integrate(&mut self, load_result: Self::LoadResult, ctx: ComponentPostIntegrateContext) -> Self::Output;

    /// Get type name literal used in json representation.
    /// We can use std::any::type_name, but that has no stability guarantee.
    fn type_name(&self) -> &'static str;
}

pub struct DefaultS11n<T>
    where T: Component + Send + Sync + Serialize + DeserializeOwned {
    name: &'static str,
    marker: PhantomData<T>
}

impl<T> DefaultS11n<T>
    where T: Component + Send + Sync + Serialize + DeserializeOwned {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData
        }
    }
}

impl<'a, T> ComponentS11n<'a> for DefaultS11n<T>
    where T: Component + Send + Sync + Serialize + DeserializeOwned
{
    type SystemData = ();
    type Output = T;
    type LoadResult = T;

    fn load(&mut self, data: Value, _: &mut Self::SystemData) -> Pin<Box<dyn Future<Output=T> + Send + Sync>> {
        let ret: T = serde_json::from_value(data).unwrap();
        Box::pin(async move {
            ret
        })
    }

    fn integrate(&mut self, instance: Self::Output, _: ComponentPostIntegrateContext) -> T {
        instance
    }

    fn type_name(&self) -> &'static str {
        self.name
    }
}

pub struct ComponentStagingData<T> where T: Send {
    staging_components: HashMap<(u32, usize), Arc<Mutex<Poll<T>>>>,
    #[allow(dead_code)]
    thread_pool: ThreadPool
}

impl<T: Send> ComponentStagingData<T> {
    pub fn new() -> Self {
        Self {
            staging_components: HashMap::new(),
            thread_pool: ThreadPool::new().unwrap()
        }
    }
}

pub struct ComponentS11nSystem<T>(pub T);

impl<'a, T> System<'a> for ComponentS11nSystem<T>
    where T: ComponentS11n<'a> {
    type SystemData = (
        WriteExpect<'a, ProtoLoadContexts>,
        WriteExpect<'a, ComponentStagingData<T::LoadResult>>,
        WriteStorage<'a, T::Output>,
        T::SystemData);

    fn run(&mut self, (mut proto_loads, mut staging_data, mut cmpt_write, mut data): Self::SystemData) {
        for entry in &mut proto_loads.v {
            for (idx, ent) in entry.loading_entities.iter_mut().enumerate() {
                let key = (entry.idx, idx);
                if let Some(state) = ent.components.get_mut(self.0.type_name()) {
                    let next_state = match &state {
                        ComponentLoadState::Init(v) => { // Init时，目前直接调用序列化代码
                            let arc = Arc::new(Mutex::new(Poll::Pending));
                            let arc_clone = arc.clone();

                            // TODO: Useless and expensive clone
                            let temp_value = v.clone();

                            let fut = self.0.load(temp_value, &mut data);
                            // https://github.com/rust-lang/rust/issues/71723
                            // 下面是预期的真正的async load代码，但是遇到了个奇怪的编译器报错，
                            //     | ...                   staging_data.thread_pool.spawn_ok(async move {
                            //     |                                                ^^^^^^^^ one type is more general than the other
                            //     |
                            //     = note: expected type `proto::ComponentS11n<'_>`
                            //                found type `proto::ComponentS11n<'a>`
                            // 暂时不知如何解决，故先做成blocking了
                            // staging_data.thread_pool.spawn_ok(async move {
                            //     let loaded_data = fut.await;
                            //     *arc_clone.lock().unwrap() = Poll::Ready(loaded_data);
                            // });

                            {
                                let loaded_data = block_on(fut);
                                *arc_clone.lock().unwrap() = Poll::Ready(loaded_data);
                            }

                            staging_data.staging_components.insert((entry.idx, idx), arc);
                            Some(ComponentLoadState::Processing)
                        },
                        ComponentLoadState::Processing => {
                            let can_remove = {
                                let ref x = staging_data.staging_components[&key];
                                x.lock().unwrap().is_ready()
                            };

                            if can_remove {
                                Some(ComponentLoadState::Finished)
                            } else {
                                None
                            }
                        },
                        ComponentLoadState::Integrate => {
                            let e = entry.entities[idx];
                            let result_arc = staging_data.staging_components.remove(&key).unwrap();
                            let result = Arc::try_unwrap(result_arc)
                                .unwrap_or_else(|_| panic!())
                                .into_inner()
                                .unwrap();

                            match result {
                                Poll::Ready(load_data) => {
                                    let cmpt = self.0.integrate(load_data, ComponentPostIntegrateContext {
                                        self_idx: idx,
                                        entity_vec: &entry.entities,
                                        request_idx: entry.idx
                                    });
                                    cmpt_write.insert(e, cmpt).unwrap();
                                }
                                _ => unreachable!()
                            }

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
    type SystemData = (WriteExpect<'a, ProtoLoadContexts>, Entities<'a>);

    fn run(&mut self, (mut proto_loads, entities): Self::SystemData) {
        for ctx in &mut proto_loads.v {
            match ctx.state {
                ProtoLoadState::ComponentLoad => {
                    // 若所有组件加载完成 则进入Integrate状态
                    if ctx.loading_entities.iter()
                        .all(|x| x.components.values()
                            .all(|x| match x { ComponentLoadState::Finished => true, _ => false } )) {
                        for _ in 0..ctx.loading_entities.len() {
                            ctx.entities.push(entities.create());
                            ctx.loading_entities.iter_mut()
                                .for_each(|loading_ent| {
                                    loading_ent.components.values_mut()
                                        .for_each(|cmpt| *cmpt = ComponentLoadState::Integrate);
                                })
                        }
                        ctx.state = ProtoLoadState::Integrate;
                    }
                }
                ProtoLoadState::Integrate => {
                    // 若所有组件Finalize 则进入Finalize状态
                    if ctx.loading_entities.iter()
                        .all(|x| x.components.values()
                            .all(|y| match y { ComponentLoadState::Finalize => true, _ => false }) ){
                        *ctx.result.lock().unwrap() = Poll::Ready(ctx.entities.drain(..).collect());
                    }
                }
                ProtoLoadState::Finalize => ()
            }
        }

        proto_loads.v.retain(|x| x.state != ProtoLoadState::Finalize);
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
