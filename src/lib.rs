#[macro_use]
extern crate log;

use specs::prelude::*;
use std::rc::Rc;

use glium::Display;
use glutin;

pub type Event = glutin::event::Event<'static, ()>;
pub type EventLoop = glutin::event_loop::EventLoop<()>;

// pub mod timing;
// pub mod game_loop;
// pub mod util;
// pub mod math;
// pub mod client;
// pub mod input;
// pub mod profile;

pub struct Insert<'a, 'c, 'd> {
    builder: &'a mut specs::DispatcherBuilder<'c, 'd>,
    name: &'a str,
    deps: &'a [&'a str],
}

impl<'a, 'c, 'd> Insert<'a, 'c, 'd> {
    pub fn insert<T>(self, system: T)
    where
        T: for<'x> specs::System<'x> + Send + 'c,
    {
        info!("insert {}", self.name);
        self.builder.add(system, self.name, self.deps);
    }

    pub fn insert_thread_local<T: 'static>(self, system: T)
    where
        T: for<'x> specs::RunNow<'x> + 'c,
    {
        info!("insert_thread_local {}", self.name);
        self.builder.add_thread_local(system);
    }
}

pub struct InsertInfo {
    name: String,
    deps: Vec<String>,
    before_deps: Vec<String>,
    order: i32,
}

impl InsertInfo {
    fn new(name: &str) -> InsertInfo {
        InsertInfo {
            name: String::from(name),
            deps: vec![],
            before_deps: vec![],
            order: 0,
        }
    }

    fn order(mut self, new_order: i32) -> Self {
        self.order = new_order;
        self
    }

    fn after(mut self, deps: &[&str]) -> Self {
        for s in deps {
            self.deps.push(String::from(*s));
        }
        self
    }

    fn before(mut self, before_deps: &[&str]) -> Self {
        for s in before_deps {
            self.before_deps.push(String::from(*s));
        }
        self
    }
}

struct DispatchItem {
    info: InsertInfo,
    func: Box<dyn FnOnce(Insert)>,
}

struct DispatchGroup {
    items: Vec<DispatchItem>,
}

impl DispatchGroup {
    pub fn new() -> Self {
        Self { items: vec![] }
    }

    /// 添加一个 System 到全局的 DispatcherBuilder，在全部添加前会先正确的根据
    /// deps 进行拓补排序，因此不需要关心添加的顺序。
    ///
    /// note: 这里非要用一个 closure 这么绕是因为 System 这个 trait 根本没法
    /// 被存成一个 trait object，只能在函数调用中接续着用 trait bound 传递过去，
    /// 实为无奈之举。
    pub fn dispatch<F>(&mut self, info: InsertInfo, item: F)
    where
        F: FnOnce(Insert) + 'static,
    {
        self.items.push(DispatchItem {
            info,
            func: Box::new(item),
        });
    }

    pub fn post_dispatch(&mut self, builder: &mut specs::DispatcherBuilder) {
        use std::collections::HashMap;
        use std::collections::HashSet;

        // First, sort with order
        self.items.sort_by_key(|x| x.info.order);

        let sorted = {
            let mut visited_deps: HashSet<String> = HashSet::new();
            let mut before_deps: HashMap<String, usize> = HashMap::new();
            let mut res: Vec<DispatchItem> = vec![];

            for item in &self.items {
                item.info.before_deps.iter().for_each(|x| {
                    let key = x.as_str();
                    let count = before_deps.get(key).map(|x| *x).unwrap_or(0) + 1;
                    before_deps.insert(x.clone(), count);
                });
            }

            let mut last_len = self.items.len();
            while self.items.len() > 0 {
                let mut i = 0;
                while i < self.items.len() {
                    // Note: Can't use foreach because self.items will change
                    let has_after_dep = self.items[i]
                        .info
                        .deps
                        .iter()
                        .find(|x| !visited_deps.contains(x.as_str()))
                        .is_some();
                    let has_before_dep = before_deps.contains_key(self.items[i].info.name.as_str());

                    let dep_unresolved = has_after_dep || has_before_dep;
                    if dep_unresolved {
                        i += 1;
                    } else {
                        // info!("remove dep: {}", self.items[i].name);
                        let removed = self.items.remove(i);
                        visited_deps.insert(removed.info.name.clone());
                        removed.info.before_deps.iter().for_each(|x| {
                            let k = x.as_str();
                            let final_count = {
                                let count_ref = before_deps.get_mut(k).unwrap();
                                *count_ref -= 1;
                                *count_ref
                            };
                            if final_count == 0 {
                                before_deps.remove(k);
                            }
                        });

                        res.push(removed);
                    }
                }
                assert!(
                    self.items.len() < last_len,
                    "Systems contains unresolvable dependency, remaining: {}, visited: {}",
                    self.items
                        .iter()
                        .map(|x| format!("{}<-[{}]", x.info.name, x.info.deps.join("+")))
                        .collect::<Vec<_>>()
                        .join(","),
                    visited_deps
                        .iter()
                        .map(|x| x.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                ); // assert that the list converges
                last_len = self.items.len();
            }

            res
        };

        for item in sorted {
            let deps_vec: Vec<&str> = item.info.deps.iter().map(|x| x.as_str()).collect();
            let insert = Insert {
                builder,
                name: item.info.name.as_str(),
                deps: deps_vec.as_slice(),
            };
            (item.func)(insert);
        }
    }
}

pub struct InitData {
    group_normal: DispatchGroup,
    group_thread_local: DispatchGroup,
}

impl InitData {
    pub fn new() -> InitData {
        InitData {
            group_normal: DispatchGroup::new(),
            group_thread_local: DispatchGroup::new(),
        }
    }

    pub fn dispatch<F>(&mut self, info: InsertInfo, func: F)
    where
        F: FnOnce(Insert) + 'static,
    {
        assert_eq!(info.order, 0, "Doesn't allow custom order");
        assert!(info.before_deps.is_empty(), "Doesn't allow before_deps");
        self.group_normal.dispatch(info, func);
    }

    pub fn dispatch_thread_local<F: 'static>(&mut self, info: InsertInfo, func: F)
    where
        F: FnOnce(Insert) + 'static,
    {
        self.group_thread_local.dispatch(info, func);
    }

    pub fn post_dispatch(mut self, builder: &mut specs::DispatcherBuilder) {
        self.group_normal.post_dispatch(builder);
        self.group_thread_local.post_dispatch(builder);
    }
}

pub struct StartData<'a> {
    pub world: &'a mut specs::World,
    pub display: Rc<Display>
}

pub trait Module {
    fn init(&self, init_data: &mut InitData) {}
    fn start(&self, start_data: &mut StartData) {}
}

pub struct RuntimeBuilder {
    name: String,
    modules: Vec<Box<Module>>
}

impl RuntimeBuilder {

    fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            modules: vec![]
        }
    }

    fn build(mut self) -> Runtime {
        let mut dispatcher_builder = specs::DispatcherBuilder::new();
        let mut init_data = crate::InitData::new();
        for game_module in &mut self.modules {
            game_module.init(&mut init_data);
        }
        init_data.post_dispatch(&mut dispatcher_builder);

        let mut dispatcher = dispatcher_builder.build();
        let mut world = World::new();
        dispatcher.setup(&mut world);

        let (display, event_loop) = {
            let event_loop = EventLoop::new();
            let wb = glutin::window::WindowBuilder::new().with_title(self.name.clone());
            let cb = glutin::ContextBuilder::new()
                //            .with_vsync(true)
                .with_srgb(true);
            (
                Rc::new(glium::Display::new(wb, cb, &event_loop).unwrap()),
                event_loop,
            )
        };

        Runtime {
            dispatcher,
            display,
            event_loop
        }
    }

}

pub struct Runtime {
    dispatcher: Dispatcher<'static, 'static>,
    // Client only
    display: Rc<Display>,
    event_loop: EventLoop
}

impl Runtime {

    fn start(mut self) {
        self.event_loop.run(move |event, _, _| {
            match event {
                glutin::event::Event::MainEventsCleared => {
                    println!("Main loop!");
                },
                _ => ()
            }
        })
    }

}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}