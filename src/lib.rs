use std::rc::Rc;

use winit::{
    event::*,
    event_loop,
    event_loop::ControlFlow,
    window::{Window, WindowBuilder}
};
use specs::prelude::*;

use crate::resource::ResManager;
use crate::client::input::RawInputData;
use crate::client::WindowInfo;
use crate::ecs::{Time, HasParent};
use crate::util::Color;
use std::sync::atomic::{AtomicBool, Ordering};
use specs_hierarchy::HierarchySystem;

pub type WindowEventLoop = event_loop::EventLoop<()>;

#[macro_use]
pub extern crate log;
pub use wgpu;
pub use specs;
pub use bytemuck;
use winit::dpi::PhysicalSize;
use std::collections::HashSet;

pub mod asset;
pub mod resource;
pub mod ecs;
pub mod math;
pub mod util;
pub mod proto;
pub mod proto_default;
pub mod client;

/// Helper struct for adding a sorted system.
pub struct Insert<'a> {
    builder: &'a mut specs::DispatcherBuilder<'static, 'static>,
    name: &'a str,
    deps: &'a [&'a str],
}

// FIXME: 当前允许对一个 Insert 调用insert多次，需要加个runtime check然后报错

impl<'a> Insert<'a> {
    pub fn insert<T>(self, system: T)
        where
            T: for<'x> specs::System<'x> + Send + 'static,
    {
        info!("insert {}({})", self.name, std::any::type_name::<T>());
        self.builder.add(system, self.name, self.deps);
    }
}


/// Helper struct for adding a sorted ThreadLocal system.
pub struct InsertThreadLocal<'a> {
    builder: &'a mut specs::DispatcherBuilder<'static, 'static>,
    name: &'a str,
}

impl<'a> InsertThreadLocal<'a> {

    pub fn insert_thread_local<T: 'static>(self, system: T)
        where
            T: for<'x> specs::RunNow<'x> + 'static,
    {
        info!("insert_thread_local {}({})", self.name, std::any::type_name::<T>());
        self.builder.add_thread_local(system);
    }

}

pub struct InsertInfo {
    name: String,
    deps: Vec<String>,
    before_deps: Vec<String>,
    order: i32,
}

impl Default for InsertInfo {
    fn default() -> Self {
        Self::new("")
    }
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

trait TDispatchItem {
    fn insert_info(&self) -> &InsertInfo;
}

struct DispatchItem {
    info: InsertInfo,
    func: Box<dyn FnOnce(&mut InitData, Insert)>,
}

impl TDispatchItem for DispatchItem {
    fn insert_info(&self) -> &InsertInfo {
        &self.info
    }
}

struct ThreadLocalDispatchItem {
    info: InsertInfo,
    func: Box<dyn FnOnce(&mut InitData, InsertThreadLocal)>,
}

impl TDispatchItem for ThreadLocalDispatchItem {

    fn insert_info(&self) -> &InsertInfo {
        &self.info
    }
}

struct DispatchGroup<T: TDispatchItem> {
    items: Vec<T>,
}

// FIXME: 这里的代码重复很丑，但是目前没找到办法通过generics很好的处理new Item的逻辑

impl DispatchGroup<DispatchItem> {

    pub fn dispatch<F>(&mut self, info: InsertInfo, item: F)
        where
            F: FnOnce(&mut InitData, Insert) + 'static,
    {
        self.items.push(DispatchItem {
            info,
            func: Box::new(item),
        });
    }
}

impl DispatchGroup<ThreadLocalDispatchItem> {

    pub fn dispatch<F>(&mut self, info: InsertInfo, item: F)
        where
            F: FnOnce(&mut InitData, InsertThreadLocal) + 'static,
    {
        self.items.push(ThreadLocalDispatchItem {
            info,
            func: Box::new(item),
        });
    }
}

impl<T: TDispatchItem> DispatchGroup<T> {
    pub fn new() -> Self {
        Self { items: vec![] }
    }

    pub fn post_dispatch<F>(mut self, mut visitor: F) where F: FnMut(T) {
        use std::collections::HashMap;

        // First, sort with order
        self.items.sort_by_key(|x| x.insert_info().order);

        // Topology sort
        let sorted = {
            let mut visited_deps: HashSet<String> = HashSet::new();
            let mut before_deps: HashMap<String, usize> = HashMap::new();
            let mut res: Vec<T> = vec![];

            for item in &self.items {
                item.insert_info().before_deps.iter().for_each(|x| {
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
                        .insert_info()
                        .deps
                        .iter()
                        .find(|x| !visited_deps.contains(x.as_str()))
                        .is_some();
                    let has_before_dep = before_deps.contains_key(self.items[i].insert_info().name.as_str());

                    let dep_unresolved = has_after_dep || has_before_dep;
                    if dep_unresolved {
                        i += 1;
                    } else {
                        // info!("remove dep: {}", self.items[i].name);
                        let removed = self.items.remove(i);
                        visited_deps.insert(removed.insert_info().name.clone());
                        removed.insert_info().before_deps.iter().for_each(|x| {
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
                        .map(|x| format!("{}<-[{}]", x.insert_info().name, x.insert_info().deps.join("+")))
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
            visitor(item);
        }
    }
}

pub struct InitData {
    pub res_mgr: ResManager,
    pub window: Rc<Window>,
    pub world: World
}

/// Data when game initializes. Usually used to setup all the systems.
pub struct InitContext {
    group_normal: DispatchGroup<DispatchItem>,
    group_thread_local: DispatchGroup<ThreadLocalDispatchItem>,
    pub init_data: InitData,
    pub existing_modules: HashSet<&'static str>
}

impl InitContext {
    pub fn new(res_mgr: ResManager, window: Rc<Window>, world: World,
               existing_modules: HashSet<&'static str>)
        -> InitContext {
        InitContext {
            group_normal: DispatchGroup::new(),
            group_thread_local: DispatchGroup::new(),
            init_data: InitData {
                res_mgr,
                window,
                world,
            },
            existing_modules
        }
    }

    pub fn dispatch<F>(&mut self, info: InsertInfo, func: F)
    where
        F: FnOnce(&mut InitData, Insert) + 'static,
    {
        assert_eq!(info.order, 0, "Doesn't allow custom order");
        assert!(info.before_deps.is_empty(), "Doesn't allow before_deps");
        info!("dispatch? {}", info.name);
        self.group_normal.dispatch(info, func);
    }

    pub fn dispatch_thread_local<F: 'static>(&mut self, info: InsertInfo, func: F)
    where
        F: FnOnce(&mut InitData, InsertThreadLocal) + 'static,
    {
        self.group_thread_local.dispatch(info, func);
    }

    pub fn post_dispatch(mut self, builder: &mut specs::DispatcherBuilder<'static, 'static>) -> World {
        {
            let init_data = &mut self.init_data;
            self.group_normal.post_dispatch(|info| {
                let deps_vec: Vec<&str> = info.info.deps.iter().map(|x| x.as_str()).collect();
                let insert = Insert {
                    builder,
                    name: info.info.name.as_str(),
                    deps: deps_vec.as_slice(),
                };
                (info.func)(init_data, insert);
            });

            self.group_thread_local.post_dispatch(|info| {
                let insert = InsertThreadLocal {
                    builder,
                    name: info.info.name.as_str(),
                };
                (info.func)(init_data, insert);
            });
        }

        self.init_data.world.insert(self.init_data.res_mgr);
        self.init_data.world
    }
}

/// Data when just before game starts. Usually used to setup the world initial entities.
pub struct StartContext<'a> {
    pub world: &'a mut specs::World,
}

/// Modules inject into the game's startup process, and are
///  capable of adding Systems and Entities.
pub trait Module {
    fn init(&self, _ctx: &mut InitContext) {}
    fn start(&self, _ctx: &mut StartContext) {}
    /// If this module needs to be depended on, this is the id of the module.
    fn name(&self) -> &'static str {
        ""
    }
    /// Return the dependencies of the module.
    ///
    /// The closure will be invoked to create the module, if the module isn't already present.
    fn deps(&self) -> Vec<(&'static str, Box<dyn FnOnce() -> Box<dyn Module>>)> {
        vec![]
    }

    // Module本身就有dependencies，再加上submodule的功能显得非常混乱，暂时先不实现，再斟酌下
    // fn get_submodules(&mut self) -> Vec<Box<dyn Module>> {
    //     vec![]
    // }
}

/// Use `RuntimeBuilder` to specify game's startup information and then start the game.
pub struct RuntimeBuilder {
    name: String,
    modules: Vec<Box<dyn Module>>
}

impl RuntimeBuilder {

    pub fn new(name: &str) -> Self {
        if !COMMON_INITIALIZED.load(Ordering::SeqCst) {
            common_init();
        }

        Self {
            name: String::from(name),
            modules: vec![]
        }
    }

    pub fn add_game_module<T: Module + 'static>(mut self, game_module: T) -> Self {
        self.add_game_module_impl(Box::new(game_module));
        self
    }

    fn add_game_module_impl(&mut self, module: Box<dyn Module>) {
        // for sub_module in module.get_submodules() {
        //     self.add_game_module_impl(sub_module);
        // }
        self.modules.push(module);
    }

    fn _build_existing_modules(&self) -> HashSet<&'static str> {
        let mut existing_modules: HashSet<&'static str> = HashSet::new();
        for module in &self.modules {
            if module.name() != "" {
                existing_modules.insert(module.name());
            }
        }
        existing_modules
    }

    pub fn build(mut self) -> Runtime {
        let mut world = World::new();

        let existing_modules = self._build_existing_modules();

        let mut new_modules = vec![];
        for module in &self.modules {
            let deps = module.deps();
            for (module_name, factory) in deps {
                if !existing_modules.contains(module_name) {
                    let module = factory();
                    new_modules.push(module);
                }
            }
        }

        for module in new_modules {
            self.add_game_module_impl(module);
        }

        // Topology sort modules
        {
            let mut satisfied_deps = HashSet::new();
            let ref mut remain_modules = self.modules;
            let mut result_modules = vec![];

            while remain_modules.len() > 0 {
                let mut has_changed = false;
                for i in (0..remain_modules.len()).rev() {
                    let deps = remain_modules[i].deps();
                    let satisfy = deps.iter().all(|(x, _)| satisfied_deps.contains(x));
                    if satisfy {
                        has_changed = true;
                        let name = remain_modules[i].name();
                        if !name.is_empty() {
                            satisfied_deps.insert(name);
                        }

                        result_modules.push(remain_modules.remove(i));
                        break
                    }
                }

                if !has_changed {
                    panic!("Module contains unresolvable dependency");
                }
            }

            std::mem::swap(&mut self.modules, &mut result_modules);
        }

        let existing_modules = self._build_existing_modules();

        // ======= WINDOWS CREATION =======
        let client_data = futures::executor::block_on(ClientRuntimeData::create(self.name, &mut world));

        // ======= INIT =======
        let mut dispatcher_builder = specs::DispatcherBuilder::new();
        let res_mgr = ResManager::new();
        let mut init_ctx = crate::InitContext::new(
            res_mgr, client_data.window.clone(), world, existing_modules);

        // Default systems
        dispatcher_builder.add(HierarchySystem::<HasParent>::new(&mut init_ctx.init_data.world), "", &[]);

        // Module init
        for game_module in &mut self.modules {
            game_module.init(&mut init_ctx);
        }

        let mut world = init_ctx.post_dispatch(&mut dispatcher_builder);

        let mut dispatcher = dispatcher_builder.build();
        dispatcher.setup(&mut world);

        // Default resources
        world.insert(Time::default());
        world.insert(RawInputData::new());

        let mut window_info = WindowInfo::new();
        let screen_size = client_data.window.inner_size();
        window_info.pixel_size = (screen_size.width, screen_size.height);
        world.insert(window_info);

        // ======= START =======
        let mut start_ctx = crate::StartContext {
            world: &mut world,
        };
        for game_module in &self.modules {
            game_module.start(&mut start_ctx);
        }

        Runtime {
            dispatcher,
            world,
            client_data
        }
    }

}

pub struct WgpuState {
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub swap_chain: wgpu::SwapChain,
    pub frame_texture: Option<wgpu::SwapChainOutput>,
    pub sc_desc: wgpu::SwapChainDescriptor,
}

impl WgpuState {

    pub async fn new(window: &Window) -> Self {
        let surface = wgpu::Surface::create(window);
        let adapter = wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface)
            },
            wgpu::BackendBit::PRIMARY
        ).await.unwrap();

        let (device, queue) = adapter.request_device(&Default::default()).await;
        let size = window.inner_size();

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo
        };

        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        Self {
            surface,
            adapter,
            device,
            queue,
            sc_desc,
            swap_chain,
            frame_texture: None
        }
    }

}

pub struct ClientRuntimeData {
    event_loop: WindowEventLoop,
    window: Rc<Window>,
}

impl ClientRuntimeData {

    async fn create(title: String, world: &mut World) -> Self {
        let event_loop = WindowEventLoop::new();
        let wb = WindowBuilder::new().with_title(title);
        let window = Rc::new(wb.build(&event_loop).unwrap());
        let wgpu_state = WgpuState::new(&*window).await;
        world.insert(wgpu_state);
        Self {
            event_loop,
            window,
        }
    }

}

/// `Runtime` is the game's actual running context.
pub struct Runtime {
    dispatcher: Dispatcher<'static, 'static>,
    world: World,
    client_data: ClientRuntimeData
}

impl Runtime {

    pub fn start(self) {
        let mut dispatcher = self.dispatcher;
        let mut world = self.world;
        let window = self.client_data.window;
        self.client_data.event_loop.run(move |mut event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            match &mut event {
                Event::WindowEvent {
                    event: WindowEvent::ScaleFactorChanged { scale_factor, new_inner_size },
                    window_id
                } => {
                    // ! 这里这个 reference 只会在当帧被使用，所以是安全的
                    let static_inner_size: &'static mut PhysicalSize<u32> =
                        unsafe { std::mem::transmute_copy(&*new_inner_size) };
                    let static_event = WindowEvent::ScaleFactorChanged {
                        scale_factor: *scale_factor,
                        new_inner_size: static_inner_size
                    };

                    let mut window_info = world.write_resource::<WindowInfo>();
                    window_info.frame_event_list.push(Event::WindowEvent { window_id: *window_id, event: static_event});

                    // info!("Scale factor changed!! {}", scale_factor);
                }
                _ => {
                    let opt_ev = event.to_static();
                    if let Some(ev) = opt_ev {
                        { // Push to window event list
                            let mut window_info = world.write_resource::<WindowInfo>();
                            window_info.frame_event_list.push(ev.clone());
                        }
                        match ev {
                            Event::LoopDestroyed => return,
                            Event::MainEventsCleared => {
                                Self::update_one_frame(&*window, &mut world, &mut dispatcher);
                            },
                            Event::WindowEvent { event, .. } => {
                                let mut raw_input = world.write_resource::<RawInputData>();
                                raw_input.on_window_event(&event);
                                match event {
                                    WindowEvent::Resized(physical_size) => {
                                        let mut window_info = world.write_resource::<WindowInfo>();
                                        window_info.pixel_size = (physical_size.width, physical_size.height);

                                        let mut ws = world.write_resource::<WgpuState>();
                                        ws.sc_desc.width = physical_size.width;
                                        ws.sc_desc.height = physical_size.height;
                                        ws.swap_chain = ws.device.create_swap_chain(&ws.surface, &ws.sc_desc);
                                    }
                                    WindowEvent::CloseRequested => {
                                        *control_flow = winit::event_loop::ControlFlow::Exit;
                                    },
                                    _ => ()
                                }
                            }
                            Event::DeviceEvent { event, .. } => {
                                let mut raw_input = world.write_resource::<RawInputData>();
                                raw_input.on_device_event(&event);
                            }
                            _ => ()
                        }
                    }
                }
            }
        })
    }

    fn update_one_frame(window: &Window,
                        world: &mut World,
                        dispatcher: &mut Dispatcher<'static, 'static>) {
        { // DeltaTime update
            let mut time = world.write_resource::<ecs::Time>();
            time.update_delta_time();
        }

        // Swap texture
        {
            let mut wgpu_state = world.write_resource::<WgpuState>();
            wgpu_state.frame_texture = Some(wgpu_state.swap_chain.get_next_texture().unwrap());

            let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: None
            });

            // works around https://github.com/gfx-rs/wgpu-rs/issues/507 by always clearing the texture.
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[
                    wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &wgpu_state.frame_texture.as_ref().unwrap().view,
                        resolve_target: None,
                        load_op: wgpu::LoadOp::Clear,
                        store_op: wgpu::StoreOp::Store,
                        clear_color: Color::rgb(0.0, 0.0, 0.0).into()
                    }
                ],
                depth_stencil_attachment: None
            });

            wgpu_state.queue.submit(&[encoder.finish()]);
        }

        dispatcher.dispatch(world);
        world.maintain();

        {
            let mut wgpu_state = world.write_resource::<WgpuState>();
            wgpu_state.frame_texture = None;
        }

        { // Control update
            let mut raw_input = world.write_resource::<RawInputData>();
            raw_input.on_frame_end();
        }
        { // Window info update
            let mut window_info = world.write_resource::<WindowInfo>();
            window_info.frame_event_list.clear();
            window.set_cursor_grab(window_info.grab_cursor_count > 0)
                .expect("Can't set cursor grab");
        }

        // 帧末释放所有资源
        resource::cleanup_local_resources();
        world.write_resource::<ResManager>().cleanup();
    }

}

/// Mu supports multi-instance. Use this to setup common functionalities shared between `Runtime`'s.
/// For single-instance games, first time creating `RuntimeBuilder` will call this.
pub fn common_init() {
    assert_eq!(COMMON_INITIALIZED.load(Ordering::SeqCst), false, "Can't common_init twice");
    env_logger::Builder::new()
        // TODO: use env variable, or other more flexible rule
        .parse_filters("info,gfx_backend_vulkan=warn,wgpu_core=warn")
        .init();
    COMMON_INITIALIZED.store(true, Ordering::SeqCst);
}

static COMMON_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
mod tests {
}