use crate::timing::Time;
// use crate::util;
use log::LevelFilter;
// use shrev::EventChannel;
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode};
use specs::prelude::*;
use std::path::Path;

use std::rc::Rc;
use glium::Display;
pub type Event = glutin::event::Event<'static, ()>;
pub type EventLoop = glutin::event_loop::EventLoop<()>;

pub struct GameLoop {
    world: World,
    dispatcher: Option<Dispatcher<'static, 'static>>,
    running: bool,
    modules: Vec<Box<Module>>,
    display: Rc<Display>,
    event_loop: Option<EventLoop>
    // event_reader_id: ReaderId<Event>,
}

pub trait Module {
    fn build(&mut self, _init_data: &mut crate::InitData) {}
    fn on_start(&self, _start_data: &mut crate::StartData) {}

    fn get_submodules(&mut self) -> Vec<Box<Module>> {
        vec![]
    }
}

#[derive(Debug)]
pub struct FpsInfo {
    pub fps: f32,
}

impl GameLoop {
    pub fn new(name: String) -> Self {
        CombinedLogger::init(vec![TermLogger::new(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Mixed,
        )])
        .unwrap();

        info!("{} init", name);

        let mut world = specs::World::new();

        // world.insert(EventChannel::<Event>::with_capacity(2000));
        world.insert(Time::default());

        // let event_reader_id =
        //     world.exec(|mut ev: Write<'_, EventChannel<Event>>| ev.register_reader());
        let (display, event_loop) = {
            let event_loop = EventLoop::new();
            let wb = glutin::window::WindowBuilder::new().with_title(name.clone());
            let cb = glutin::ContextBuilder::new()
                //            .with_vsync(true)
                .with_srgb(true);
            (
                Rc::new(glium::Display::new(wb, cb, &event_loop).unwrap()),
                event_loop,
            )
        };

        GameLoop {
            world,
            dispatcher: None,
            running: false,
            modules: vec![],
            display,
            event_loop: Some(event_loop)
            // event_reader_id,
        }
    }

    pub fn build(mut self) -> Self {
        let mut dispatcher_builder = specs::DispatcherBuilder::new();
        let mut init_data = crate::InitData::new();
        for game_module in &mut self.modules {
            game_module.build(&mut init_data);
        }
        init_data.post_dispatch(&mut dispatcher_builder);

        let mut dispatcher = dispatcher_builder.build();
        dispatcher.setup(&mut self.world);
        self.dispatcher = Some(dispatcher);

        self
    }

    pub fn add_game_module<T: Module + 'static>(mut self, game_module: T) -> Self {
        self.add_game_module_impl(Box::new(game_module));
        self
    }

    fn add_game_module_impl(&mut self, mut module: Box<Module>) {
        for sub_module in module.get_submodules() {
            self.add_game_module_impl(sub_module);
        }
        self.modules.push(module);
    }

    pub fn run(self) {
        self.running = true;
        let mut start_data = crate::StartData {
            world: &mut self.world,
            display: self.display.clone()
        };
        for game_module in &self.modules {
            game_module.on_start(&mut start_data);
        }

        let mut fps_counter = crate::util::FpsCounter::new();
        self.event_loop.take().unwrap().run(move |event, _, _| {
            match event {
                Event::MainEventsCleared => {
                    fps_counter.begin_frame();

                    {
                        let world = &self.world;
                        let mut time = world.write_resource::<Time>();
                        time.update_delta_time();
                    }

                    if let Some(dispatcher) = &mut self.dispatcher {
                        dispatcher.dispatch(&self.world);
                        self.world.maintain();
                    } else {
                        self.running = false;
                    }

                    if fps_counter.end_frame() {
                        self.world.insert(FpsInfo {
                            fps: fps_counter.get_fps(),
                        })
                    }
                },
                _ => (),
            }
        });
    }
}
