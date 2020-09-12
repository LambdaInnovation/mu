use specs::prelude::*;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use crate::asset::load_asset;
use std::future::Future;
use std::pin::Pin;
use futures::executor::{ThreadPool, block_on};
use std::marker::PhantomData;
use crate::{Module, InitContext, InsertInfo};

pub static DEP_PROTO_LOAD: &str = "proto_load";

pub struct ProtoLoadRequest {
    pub path: String,
    pub result: ProtoLoadResult
}

impl ProtoLoadRequest {

    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            result: Arc::new(Mutex::new(Poll::Pending))
        }
    }

}

pub type ProtoLoadRequests = Vec<ProtoLoadRequest>;

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
    components: HashMap<String, ComponentLoadState>,
}

pub struct ProtoLoadContext {
    idx: u32, // An unique id to distinguish between load requests
    loading_entities: Vec<LoadingEntity>,
    entities: Vec<Entity>,
    state: ProtoLoadState,
    result: ProtoLoadResult
}

pub struct ProtoStoreContext {}

pub struct ProtoLoadContexts {
    v: Vec<ProtoLoadContext>,
}

impl ProtoLoadContexts {

    pub(super) fn new() -> Self {
        ProtoLoadContexts {
            v: vec![],
        }
    }

}

pub struct ComponentLoadArgs<'a> {
    pub data: Value,
    pub entity_idx: usize,
    pub all_entity_vec: &'a Vec<Entity>
}

pub trait ComponentS11n<'a> {
    type SystemData: SystemData<'a>;
    type Output: 'static + Component + Send + Sync;

    fn load_async(&mut self, ctx: ComponentLoadArgs, system_data: &mut Self::SystemData) -> Pin<Box<dyn Future<Output = Self::Output> + Send + Sync>>;

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

    fn load_async(&mut self, ctx: ComponentLoadArgs, _: &mut Self::SystemData)
        -> Pin<Box<dyn Future<Output=Self::Output> + Send + Sync>> {
        let v = ctx.data;
        Box::pin(async move {
            let ret: T = serde_json::from_value(v).unwrap();
            ret
        })
    }

    fn type_name(&self) -> &'static str {
        self.name
    }
}

pub struct ComponentStagingData<T> where T: Component {
    staging_components: HashMap<(u32, usize), Arc<Mutex<Poll<T>>>>,
    #[allow(dead_code)]
    thread_pool: ThreadPool
}

impl<T: Component> Default for ComponentStagingData<T> {
    fn default() -> Self {
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
        Write<'a, ComponentStagingData<T::Output>>,
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

                            let fut = self.0.load_async(ComponentLoadArgs {
                                data: temp_value,
                                entity_idx: idx,
                                all_entity_vec: &entry.entities
                            }, &mut data);
                            // https://github.com/rust-lang/rust/issues/71723
                            // 下面是预期的真正的async load代码，但是遇到了个奇怪的编译器报错，
                            //     | ...                   staging_data.thread_pool.spawn_ok(async move {}
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
                                Poll::Ready(cmpt) => {
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

struct EntityLoadSystem {
    counter: u32
}

impl EntityLoadSystem {
    pub fn new() -> Self {
        Self {
            counter: 0
        }
    }
}

impl<'a> System<'a> for EntityLoadSystem {
    type SystemData = (WriteExpect<'a, ProtoLoadRequests>, WriteExpect<'a, ProtoLoadContexts>, Entities<'a>);

    fn run(&mut self, (mut requests, mut proto_loads, entities): Self::SystemData) {
        requests.drain(..)
            .for_each(|req| {
                let s: String = load_asset(&req.path).unwrap();
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
                                            components,
                                        }
                                    }
                                    _ => panic!("Invalid entity data type, expecting object")
                                }
                            })
                            .collect::<Vec<_>>()
                    }
                    _ => panic!("Invalid root type")
                };
                let entity_count = loading_entities.len();

                let arc = Arc::new(Mutex::new( Poll::Pending ));
                let ctx = ProtoLoadContext {
                    idx: self.counter,
                    loading_entities,
                    result: arc.clone(),
                    state: ProtoLoadState::ComponentLoad,
                    entities: (0..entity_count).map(|_| entities.create()).collect()
                };
                proto_loads.v.push(ctx);
                self.counter += 1;
            });

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

pub(super) struct ProtoModule;

impl Module for ProtoModule {
    fn init(&self, ctx: &mut InitContext) {
        ctx.init_data.world.insert(ProtoLoadRequests::new());
        ctx.init_data.world.insert(ProtoLoadContexts::new());

        ctx.dispatch(InsertInfo::new(DEP_PROTO_LOAD), |_, i| i.insert(EntityLoadSystem::new()));
    }
}

pub trait InitContextProtoExt {
    fn add_component_s11n<T: 'static + for<'a> ComponentS11n<'a> + Send>(&mut self, s11n: T);
}

impl InitContextProtoExt for super::InitContext {
    fn add_component_s11n<T: 'static + for<'a> ComponentS11n<'a> + Send>(&mut self, s11n: T) {
        self.dispatch(InsertInfo::default().after(&[DEP_PROTO_LOAD]),
            |_, i| i.insert(ComponentS11nSystem(s11n)));
    }
}