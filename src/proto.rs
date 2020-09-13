use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use futures::executor::{block_on, ThreadPool};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use specs::prelude::*;

use internal::*;

use crate::{InitContext, InsertInfo, Module};
use crate::asset;

pub static DEP_PROTO_LOAD: &str = "proto_load";
pub static DEP_PROTO_STORE: &str = "proto_store";

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

pub struct ProtoStoreRequest {
    pub entities: Vec<Entity>,
    pub target_path: String,
}

impl ProtoStoreRequest {

    pub fn new(entities: &[Entity], target_path: &str) -> Self {
        Self {
            entities: entities.iter().map(|x| *x).collect(),
            target_path: target_path.to_string()
        }
    }

}

pub type ProtoStoreRequests = Vec<ProtoStoreRequest>;

pub type ProtoLoadResult = Arc<Mutex<Poll <Vec<Entity>> >>;

pub struct ComponentLoadArgs<'a> {
    pub data: Value,
    pub entity_idx: usize,
    pub all_entity_vec: &'a Vec<Entity>
}

impl<'a> ComponentLoadArgs<'a> {

    pub fn inherit_with_data(&self, new_data: Value) -> Self {
        Self {
            data: new_data,
            entity_idx: self.entity_idx,
            all_entity_vec: self.all_entity_vec
        }
    }

}

pub struct ComponentStoreArgs<'a, T> {
    pub component: &'a T,
    pub entity_idx: usize,
    pub all_entity_vec: &'a Vec<Entity>
}

impl<'a, T: Component> ComponentStoreArgs<'a, T> {
    pub fn inherit_with_other<U>(&self, other: &'a U) -> ComponentStoreArgs<U> {
        ComponentStoreArgs {
            component: other,
            entity_idx: self.entity_idx,
            all_entity_vec: self.all_entity_vec
        }
    }
}

pub trait ComponentS11n<'a> {
    type SystemData: SystemData<'a>;
    type StoreSystemData: SystemData<'a>;
    type Output: Component + Send + Sync;

    fn load_async(&mut self, ctx: ComponentLoadArgs, system_data: &mut Self::SystemData) -> Pin<Box<dyn Future<Output = Self::Output> + Send + Sync>>;

    fn store(&mut self, ctx: ComponentStoreArgs<Self::Output>, system_data: &mut Self::StoreSystemData) -> Value;

    /// Get type name literal used in json representation.
    /// We can use std::any::type_name, but that has no stability guarantee.
    fn type_name(&self) -> &'static str;
}

#[derive(Clone)]
pub struct ComponentS11nDefault<T>
    where T: Send + Sync + Serialize + DeserializeOwned {
    name: &'static str,
    marker: PhantomData<T>
}

impl<T> ComponentS11nDefault<T>
    where T: Send + Sync + Serialize + DeserializeOwned {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData
        }
    }
}

impl<'a, T> ComponentS11n<'a> for ComponentS11nDefault<T>
    where T: Component + Send + Sync + Serialize + DeserializeOwned
{
    type SystemData = ();
    type StoreSystemData = ();
    type Output = T;

    fn load_async(&mut self, ctx: ComponentLoadArgs, _: &mut Self::SystemData)
        -> Pin<Box<dyn Future<Output=Self::Output> + Send + Sync>> {
        let v = ctx.data;
        Box::pin(async move {
            let ret: T = serde_json::from_value(v).unwrap();
            ret
        })
    }

    fn store(&mut self, ctx: ComponentStoreArgs<Self::Output>, _system_data: &mut Self::StoreSystemData)
        -> Value {
        serde_json::to_value(ctx.component).expect("Serialize failed")
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

pub(super) struct ProtoModule;

impl Module for ProtoModule {
    fn init(&self, ctx: &mut InitContext) {
        if ctx.init_data.world.try_fetch::<ProtoStoreGlobalData>().is_none() {
            ctx.init_data.world.insert(internal::ProtoStoreGlobalData::default());
        }
        ctx.init_data.world.insert(ProtoLoadRequests::new());
        ctx.init_data.world.insert(ProtoLoadContexts::new());

        ctx.dispatch(InsertInfo::new(DEP_PROTO_LOAD),
                     |_, i| i.insert(internal::ProtoLoadSystem::new()));
        ctx.dispatch(InsertInfo::new(DEP_PROTO_STORE),
                     |_, i| i.insert(internal::ProtoStoreSystem));
    }
}

pub trait InitContextProtoExt {
    fn add_component_s11n<T: 'static + for<'a> ComponentS11n<'a> + Send + Clone>(&mut self, s11n: T);
}

impl InitContextProtoExt for super::InitContext {
    fn add_component_s11n<T: 'static + for<'a> ComponentS11n<'a> + Send + Clone>(&mut self, s11n: T) {
        if self.init_data.world.try_fetch::<ProtoStoreGlobalData>().is_none() {
            self.init_data.world.insert(internal::ProtoStoreGlobalData::default());
        }

        self.init_data.world.write_resource::<ProtoStoreGlobalData>()
            .all_component_names.push(s11n.type_name());
        let cloned_s11n = s11n.clone();
        self.dispatch(InsertInfo::default().after(&[DEP_PROTO_LOAD]),
            |_, i| i.insert(ComponentLoadSystem(cloned_s11n)));
        self.dispatch(InsertInfo::default().after(&[DEP_PROTO_STORE]),
                      |_, i| i.insert(ComponentStoreSystem(s11n)));
    }
}

pub(super) mod internal {
    use super::*;

    pub struct ProtoStoreGlobalData {
        pub all_component_names: Vec<&'static str>
    }

    impl Default for ProtoStoreGlobalData {
        fn default() -> Self {
            Self {
                all_component_names: vec![]
            }
        }
    }

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

    #[derive(Copy, Clone, PartialEq)]
    pub enum ProtoStoreState {
        Init, Processing, Finished
    }

    pub enum ComponentStoreState {
        Await,
        Stored(Option<Value>)
    }

    impl ComponentStoreState {

        pub fn unwrap(self) -> Option<Value> {
            match self {
                ComponentStoreState::Await => panic!("Unwrap on ComponentStoreState::Await"),
                ComponentStoreState::Stored(v) => v
            }
        }

        pub fn is_await(&self) -> bool {
            match self {
                ComponentStoreState::Await => true,
                _ => false
            }
        }
    }

    pub struct LoadingEntity {
        pub components: HashMap<String, ComponentLoadState>,
    }

    pub struct ProtoLoadContext {
        pub idx: u32, // An unique id to distinguish between load requests
        pub loading_entities: Vec<LoadingEntity>,
        pub entities: Vec<Entity>,
        pub state: ProtoLoadState,
        pub result: ProtoLoadResult
    }

    pub struct ProtoLoadContexts {
        pub v: Vec<ProtoLoadContext>,
    }

    impl ProtoLoadContexts {

        pub(super) fn new() -> Self {
            ProtoLoadContexts {
                v: vec![],
            }
        }

    }

    pub struct ComponentLoadSystem<T>(pub T);

    impl<'a, T> System<'a> for ComponentLoadSystem<T>
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

    pub struct StoringEntity {
        pub components: HashMap<String, ComponentStoreState>,
    }

    impl StoringEntity {

        fn new(names: &[&'static str]) -> Self {
            Self {
                components: names.iter().map(|x| (x.to_string(), ComponentStoreState::Await)).collect()
            }
        }

    }

    pub struct ProtoStoreContext {
        pub target_path: String,
        pub entities: Vec<Entity>,
        pub results: Vec<StoringEntity>,
        pub state: ProtoStoreState
    }

    pub type ProtoStoreContexts = Vec<ProtoStoreContext>;

    pub struct ComponentStoreSystem<T>(pub T);

    impl<'a, T> System<'a> for ComponentStoreSystem<T>
        where T: ComponentS11n<'a>, <T as ComponentS11n<'a>>::Output: Component {
        type SystemData = (
            WriteExpect<'a, ProtoStoreContexts>,
            ReadStorage<'a, T::Output>,
            T::StoreSystemData
        );

        fn run(&mut self, (mut proto_stores, cmpt_read, mut data): Self::SystemData) {
            for ctx in &mut *proto_stores {
                if ctx.state != ProtoStoreState::Processing {
                    continue
                }

                for i in 0..ctx.results.len() {
                    if !ctx.results[i].components[self.0.type_name()].is_await() {
                        continue
                    }

                    let cmpt = cmpt_read.get(ctx.entities[i]);
                    *ctx.results[i].components.get_mut(self.0.type_name()).unwrap() = ComponentStoreState::Stored(
                        cmpt.map(|val| {
                            self.0.store(ComponentStoreArgs {
                                component: val,
                                entity_idx: i,
                                all_entity_vec: &ctx.entities
                            }, &mut data)
                        })
                    );
                }
            }
        }
    }

    pub struct ProtoLoadSystem {
        counter: u32
    }

    impl ProtoLoadSystem {
        pub fn new() -> Self {
            Self {
                counter: 0
            }
        }
    }

    impl<'a> System<'a> for ProtoLoadSystem {
        type SystemData = (WriteExpect<'a, ProtoLoadRequests>, WriteExpect<'a, ProtoLoadContexts>, Entities<'a>);

        fn run(&mut self, (mut requests, mut proto_loads, entities): Self::SystemData) {
            requests.drain(..)
                .for_each(|req| {
                    let s: String = asset::load_asset(&req.path).unwrap();
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

                    let ctx = ProtoLoadContext {
                        idx: self.counter,
                        loading_entities,
                        result: req.result,
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
                            ctx.state = ProtoLoadState::Finalize;
                        }
                    }
                    ProtoLoadState::Finalize => ()
                }
            }

            proto_loads.v.retain(|x| x.state != ProtoLoadState::Finalize);
        }
    }

    pub struct ProtoStoreSystem;

    impl<'a> System<'a> for ProtoStoreSystem {
        type SystemData = (Write<'a, ProtoStoreRequests>, Write<'a, ProtoStoreContexts>, ReadExpect<'a, ProtoStoreGlobalData>);

        fn run(&mut self, (mut requests, mut ctxs, global_data): Self::SystemData) {
            requests.drain(..).for_each(|req| {
                let entity_count = req.entities.len();
                let ctx = ProtoStoreContext {
                    target_path: req.target_path,
                    entities: req.entities,
                    results: (0..entity_count).map(|_| StoringEntity::new(&*global_data.all_component_names)).collect(),
                    state: ProtoStoreState::Init
                };
                ctxs.push(ctx);
            });

            for entry in &mut *ctxs {
                if entry.state == ProtoStoreState::Init {
                    entry.state = ProtoStoreState::Processing;
                } else if entry.state == ProtoStoreState::Processing { // Processing
                    // 若所有component序列化完毕，则开始写入
                    if entry.results.iter().all(|x|
                        x.components.values().all(|y| !y.is_await())) {
                        let entity_objs: Vec<_> = entry.results.drain(..)
                            .map(|x| {
                                let m: serde_json::Map<String, _> =
                                    x.components.into_iter()
                                        .flat_map(|(k, v)| v.unwrap().map(|v2| (k, v2)))
                                        .collect();
                                Value::Object(m)
                            })
                            .collect();
                        let result_str = serde_json::to_string_pretty(&Value::Array(entity_objs)).expect("Failed serialize to JSON");
                        let result_path = asset::get_fs_path(&entry.target_path);
                        std::fs::write(result_path, result_str).unwrap();

                        entry.state = ProtoStoreState::Finished;
                    }
                }
            }

            ctxs.retain(|x| x.state != ProtoStoreState::Finished);
        }
    }
}