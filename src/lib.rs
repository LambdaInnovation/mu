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
use std::cell::RefCell;

pub type WindowEventLoop = event_loop::EventLoop<()>;

#[macro_use]
pub extern crate log;
pub use wgpu;
pub use specs;
pub use bytemuck;

pub mod asset;
pub mod resource;
pub mod ecs;
pub mod math;
pub mod util;
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
        info!("insert {}", self.name);
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
        use std::collections::HashSet;

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
    pub wgpu_state: Rc<RefCell<WgpuState>>,
    pub res_mgr: ResManager,
    pub window: Rc<Window>
}

/// Data when game initializes. Usually used to setup all the systems.
pub struct InitContext {
    group_normal: DispatchGroup<DispatchItem>,
    group_thread_local: DispatchGroup<ThreadLocalDispatchItem>,
    pub init_data: InitData
}

impl InitContext {
    pub fn new(res_mgr: ResManager, wgpu_state: Rc<RefCell<WgpuState>>, window: Rc<Window>) -> InitContext {
        InitContext {
            group_normal: DispatchGroup::new(),
            group_thread_local: DispatchGroup::new(),
            init_data: InitData {
                wgpu_state,
                res_mgr,
                window
            }
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

    pub fn post_dispatch(mut self, world: &mut World, builder: &mut specs::DispatcherBuilder<'static, 'static>) {
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

        world.insert(self.init_data.res_mgr);
    }
}

/// Data when just before game starts. Usually used to setup the world initial entities.
pub struct StartContext<'a> {
    pub world: &'a mut specs::World,
    pub wgpu_state: WgpuStateCell
}

/// Modules inject into the game's startup process, and are
///  capable of adding Systems and Entities.
pub trait Module {
    fn init(&self, _init_data: &mut InitContext) {}
    fn start(&self, _start_data: &mut StartContext) {}
    fn get_submodules(&mut self) -> Vec<Box<dyn Module>> {
        vec![]
    }
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

    fn add_game_module_impl(&mut self, mut module: Box<dyn Module>) {
        for sub_module in module.get_submodules() {
            self.add_game_module_impl(sub_module);
        }
        self.modules.push(module);
    }

    pub fn build(mut self) -> Runtime {
        // ======= WINDOWS CREATION =======
        let client_data = futures::executor::block_on(ClientRuntimeData::new(self.name));

        // ======= INIT =======
        let mut dispatcher_builder = specs::DispatcherBuilder::new();
        let res_mgr = ResManager::new();
        let mut init_data = crate::InitContext::new(
            res_mgr, client_data.wgpu_state.clone(), client_data.window.clone());
        let mut world = World::new();

        // Default systems
        dispatcher_builder.add(HierarchySystem::<HasParent>::new(&mut world), "", &[]);

        // Module init
        for game_module in &mut self.modules {
            game_module.init(&mut init_data);
        }

        init_data.post_dispatch(&mut world, &mut dispatcher_builder);

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
        let mut start_data = crate::StartContext {
            world: &mut world,
            wgpu_state: client_data.wgpu_state.clone()
        };
        for game_module in &self.modules {
            game_module.start(&mut start_data);
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

pub type WgpuStateCell = Rc<RefCell<WgpuState>>;

pub struct ClientRuntimeData {
    event_loop: WindowEventLoop,
    window: Rc<Window>,
    wgpu_state: Rc<RefCell<WgpuState>>
}

impl ClientRuntimeData {

    async fn new(title: String) -> Self {
        let event_loop = WindowEventLoop::new();
        let wb = WindowBuilder::new().with_title(title);
        let window = Rc::new(wb.build(&event_loop).unwrap());
        let wgpu_state = WgpuState::new(&*window).await;
        let wgpu_state = Rc::new(RefCell::new(wgpu_state));
        Self {
            event_loop,
            window,
            wgpu_state
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
        let wgpu_state = self.client_data.wgpu_state;
        self.client_data.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            match &event {
                Event::WindowEvent {
                    event: WindowEvent::ScaleFactorChanged { scale_factor, .. }, ..
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
                    Event::LoopDestroyed => return,
                    Event::MainEventsCleared => {
                        Self::update_one_frame(&*window, &mut world, &wgpu_state, &mut dispatcher);
                    },
                    Event::WindowEvent { event, .. } => {
                        let mut raw_input = world.write_resource::<RawInputData>();
                        raw_input.on_window_event(&event);
                        match event {
                            WindowEvent::Resized(physical_size) => {
                                window.set_inner_size(physical_size);
                                let mut window_info = world.write_resource::<WindowInfo>();
                                window_info.pixel_size = (physical_size.width, physical_size.height)
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
        })
    }

    fn update_one_frame(window: &Window,
                        world: &mut World,
                        wgpu_state_ref: &Rc<RefCell<WgpuState>>,
                        dispatcher: &mut Dispatcher<'static, 'static>) {
        { // DeltaTime update
            let mut time = world.write_resource::<ecs::Time>();
            time.update_delta_time();
        }

        // Swap texture
        {
            let mut wgpu_state = wgpu_state_ref.borrow_mut();
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
            let mut wgpu_state = wgpu_state_ref.borrow_mut();
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