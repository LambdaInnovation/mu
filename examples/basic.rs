extern crate mu;
use mu::log::*;
use specs::prelude::*;

struct ExampleSystem {
    timer: f32
}

struct ExampleCmpt{
    hp: u32
}

impl Component for ExampleCmpt {
    type Storage = specs::VecStorage<Self>;
}

struct ExampleModule;

impl<'a> System<'a> for ExampleSystem {
    type SystemData = (ReadExpect<'a, mu::ecs::Time>, WriteStorage<'a, ExampleCmpt>);

    fn run(&mut self, (time, mut cmpts): Self::SystemData) {
        // info!("ExampleSystem tick dt={}s", time.get_delta_time());
        self.timer += time.get_delta_time();
        let should_reduce_hp = self.timer >= 1.0;
        if should_reduce_hp {
            self.timer -= 1.0;
        }

        for item in (&mut cmpts).join() {
            if should_reduce_hp && item.hp > 0 {
                item.hp -= 1;
                info!("HP: {}", item.hp);
            }
        }
    }
}

impl mu::Module for ExampleModule {

    fn init(&self, init_data: &mut mu::InitData) {
        info!("ExampleModule init");
        init_data.dispatch(mu::InsertInfo::new("example_module"), 
            |f| { f.insert(ExampleSystem { timer: 0.0 }) });
    }

    fn start(&self, start_data: &mut mu::StartData) {
        start_data.world
            .create_entity()
            .with(ExampleCmpt { hp: 100 })
            .build();
    }

}


fn main() {
    mu::common_init();
    info!("My start");
    let runtime = mu::RuntimeBuilder::new("Mu Example: Basic")
        .add_game_module(ExampleModule)
        .build();
    runtime.start();
}