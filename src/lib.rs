#[macro_use]
pub extern crate log;

pub extern crate glium;

extern crate serde;

extern crate simplelog;
pub extern crate specs;

use simplelog::*;

use specs::prelude::*;
use std::rc::Rc;

use glium::Display;
use glutin;
use glutin::event;
use glutin::event_loop::ControlFlow;
use crate::client::input::RawInputData;
use crate::client::WindowInfo;

pub type WindowEventLoop = glutin::event_loop::EventLoop<()>;

pub mod asset;
pub mod ecs;
pub mod math;
pub mod util;
pub mod client;

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
    pub fn new(name: &str) -> InsertInfo {
        InsertInfo {
            name: String::from(name),
            deps: vec![],
            before_deps: vec![],
            order: 0,
        }
    }

    pub fn order(mut self, new_order: i32) -> Self {
        self.order = new_order;
        self
    }

    pub fn after(mut self, deps: &[&str]) -> Self {
        for s in deps {
            self.deps.push(String::from(*s));
        }
        self
    }

    pub fn before(mut self, before_deps: &[&str]) -> Self {
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
    pub display: Rc<Display>
}

impl InitData {
    pub fn new(display: Rc<Display>) -> InitData {
        InitData {
            group_normal: DispatchGroup::new(),
            group_thread_local: DispatchGroup::new(),
            display: display
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
    fn init(&self, _init_data: &mut InitData) {}
    fn start(&self, _start_data: &mut StartData) {}
    fn get_submodules(&mut self) -> Vec<Box<dyn Module>> {
        vec![]
    }
}

pub struct RuntimeBuilder {
    name: String,
    modules: Vec<Box<dyn Module>>
}

impl RuntimeBuilder {

    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            modules: vec![]
        }
    }

    pub fn add_game_module<T: Module + 'static>(mut self, game_module: T) -> Self {
        self.add_game_module_impl(Box::new(game_module));
        self
    }

    fn add_game_module_impl(&mut self, mut module: Box<dyn Module>) {
        for sub_module in module.get_submodules() {
            self.add_game_module_impl(sub_module);
        }
        self.modules.push(module);
    }

    pub fn build(mut self) -> Runtime {
        // ======= WINDOWS CREATION =======
        let (display, event_loop) = {
            let event_loop = WindowEventLoop::new();
            let wb = glutin::window::WindowBuilder::new().with_title(self.name.clone());
            let cb = glutin::ContextBuilder::new()
                //            .with_vsync(true)
                .with_srgb(true);
            (
                Rc::new(glium::Display::new(wb, cb, &event_loop).unwrap()),
                event_loop,
            )
        };

        // ======= INIT =======
        let mut dispatcher_builder = specs::DispatcherBuilder::new();
        let mut init_data = crate::InitData::new(display.clone());
        for game_module in &mut self.modules {
            game_module.init(&mut init_data);
        }
        init_data.post_dispatch(&mut dispatcher_builder);

        let mut dispatcher = dispatcher_builder.build();
        let mut world = World::new();
        dispatcher.setup(&mut world);

        // Default resources
        world.insert(ecs::Time::default());
        world.insert(RawInputData::new());
        world.insert(WindowInfo::new());

        // ======= START =======
        let mut start_data = crate::StartData {
            world: &mut world,
            display: display.clone()
        };
        for game_module in &self.modules {
            game_module.start(&mut start_data);
        }

        Runtime {
            dispatcher,
            world,
            display,
            event_loop
        }
    }

}

pub struct Runtime {
    dispatcher: Dispatcher<'static, 'static>,
    world: World,
    // Client only
    display: Rc<Display>,
    event_loop: WindowEventLoop
}

impl Runtime {

    pub fn start(self) {
        let display = self.display;
        let mut dispatcher = self.dispatcher;
        let mut world = self.world;
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            match &event {
                event::Event::WindowEvent {
                    event: event::WindowEvent::ScaleFactorChanged { scale_factor, .. }, ..
                } => {
                    info!("Scale factor changed!! {}", scale_factor);
                }
                _ => ()
            }

            let opt_ev = event.to_static();
            {
                let mut window_info = world.write_resource::<WindowInfo>();
                match opt_ev.clone() {
                    Some(ev) => window_info.frame_event_list.push(ev),
                    _ => ()
                }
            }

            if let Some(event) = opt_ev {
                match event {
                    event::Event::LoopDestroyed => return,
                    event::Event::MainEventsCleared => {
                        Self::update_one_frame(&*display, &mut world, &mut dispatcher);
                    },
                    event::Event::WindowEvent { event, .. } => {
                        let mut raw_input = world.write_resource::<RawInputData>();
                        raw_input.on_window_event(&event);
                        match event {
                            event::WindowEvent::Resized(physical_size) => {
                                display.gl_window().resize(physical_size);
                            }
                            event::WindowEvent::CloseRequested => {
                                *control_flow = glutin::event_loop::ControlFlow::Exit;
                            },
                            _ => ()
                        }
                    }
                    event::Event::DeviceEvent { event, .. } => {
                        let mut raw_input = world.write_resource::<RawInputData>();
                        raw_input.on_device_event(&event);
                    }
                    _ => ()
                }
            }
        })
    }

    fn update_one_frame(display: &Display, world: &mut World, dispatcher: &mut Dispatcher<'static, 'static>) {
        { // DeltaTime update
            let mut time = world.write_resource::<ecs::Time>();
            time.update_delta_time();
        }

        dispatcher.dispatch(world);
        world.maintain();

        { // Control update
            let mut raw_input = world.write_resource::<RawInputData>();
            raw_input.on_frame_end();
        }
        { // Window info update
            let mut window_info = world.write_resource::<WindowInfo>();
            window_info.frame_event_list.clear();
            display.gl_window().window().set_cursor_grab(window_info.grab_cursor_count > 0)
                .unwrap_or_default();
        }

        // TODO: figure out how to correct double buffering
        // display.gl_window().swap_buffers().unwrap();
    }

}

pub fn common_init() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed),
            // WriteLogger::new(LevelFilter::Info, Config::default(), File::create("my_rust_binary.log").unwrap()),
        ]
    ).unwrap();
}

#[cfg(test)]
mod tests {
}