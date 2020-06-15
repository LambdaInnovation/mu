/// ! Basic components used in ECS.
use specs::prelude::*;
use crate::math::*;

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