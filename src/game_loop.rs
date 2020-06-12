use crate::timing::Time;
// use crate::util;
use glutin::event::Event;
use log::LevelFilter;
// use shrev::EventChannel;
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode};
use specs::prelude::*;
use std::path::Path;

pub struct GameLoop<'a, 'b> {
    world: World,
    dispatcher: Option<Dispatcher<'a, 'b>>,
    running: bool,
    modules: Vec<Box<Module>>,
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

pub struct StartData<'a> {
    world: &'a mut specs::World,
}

impl<'a, 'b> GameLoop<'a, 'b> {
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

        GameLoop {
            world,
            dispatcher: None,
            running: false,
            modules: vec![],
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

    pub fn run(&mut self) {
        self.running = true;
        let mut start_data = crate::StartData {
            world: &mut self.world,
        };
        for game_module in &self.modules {
            game_module.on_start(&mut start_data);
        }

        let mut fps_counter = crate::util::FpsCounter::new();
        while self.running {
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
        }
    }
}
