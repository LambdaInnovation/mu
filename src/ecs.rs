/// ! Basic components used in ECS.
use specs::prelude::*;
use crate::math::*;
use std::time::Instant;

const MAX_DELTA_TIME: f32 = 0.1;

pub struct Time {
    delta_time: f32, //Duration,
    now: Instant,
}

impl Default for Time {
    fn default() -> Time {
        let now = Instant::now();
        Time {
            delta_time: 0.0,
            now,
        }
    }
}

impl Time {
    pub fn update_delta_time(&mut self) {
        self.delta_time = f32::min(MAX_DELTA_TIME, ((self.now.elapsed().as_micros() as f64) / 1e6f64) as f32);
        self.now = Instant::now();
    }

    pub fn get_delta_time(&self) -> f32 {
        self.delta_time
    }
}


pub struct Transform {
    pub pos: Vec3,
    pub rot: Vec3,
}

impl Transform {
    pub fn get_rotation(&self) -> Quaternion {
        Quaternion::from(Euler::new(deg(self.rot[0]), deg(self.rot[1]), deg(self.rot[2])))
    }
}

impl Component for Transform {
    type Storage = specs::VecStorage<Self>;
}
