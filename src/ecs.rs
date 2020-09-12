/// ! Basic components used in ECS.
use specs::prelude::*;
use crate::math::*;
use std::time::Instant;
use specs_hierarchy::Parent;
use crate::proto::*;
use serde::{Serialize, Deserialize};
use specs_derive::*;
use std::pin::Pin;
use futures::Future;

const MAX_DELTA_TIME: f32 = 0.1;


/// A `Resource`. Time information for every frame.
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

/// A generic 3d transform.
#[derive(Serialize, Deserialize, Clone)]
#[derive(Component)]
#[storage(VecStorage)]
pub struct Transform {
    #[serde(default="_vec3_zero")]
    pub pos: Vec3,
    #[serde(default="_quat_identity")]
    pub rot: Quaternion,
}

fn _vec3_zero() -> Vec3 {
    Vec3::zero()
}

fn _quat_identity() -> Quaternion {
    Quaternion::one()
}

impl Transform {

    pub fn new() -> Self {
        Self {
            pos: vec3(0., 0., 0.),
            rot: Quaternion::one()
        }
    }

    pub fn pos(mut self, p: Vec3) -> Self {
        self.pos = p;
        self
    }

    pub fn rot(mut self, r: Quaternion) -> Self {
        self.rot = r;
        self
    }

    pub fn get_world_view(&self) -> Mat4 {
        let rot: Mat4 = self.rot.into();
        let world_view = Mat4::from_translation(-self.pos) * rot;
        world_view
    }

}

/// Generic parent component used for `specs-hierarchy`.
/// for detailed usage see [specs-hierarchy site](https://github.com/rustgd/specs-hierarchy)
#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct HasParent {
    pub parent: Entity
}

impl HasParent {

    pub fn new(parent: Entity) -> Self {
        Self { parent }
    }

}

impl Component for HasParent {
    type Storage = FlaggedStorage<Self, DenseVecStorage<Self>>;
}

impl Parent for HasParent {
    fn parent_entity(&self) -> Entity {
        self.parent
    }
}

#[derive(Serialize, Deserialize)]
struct HasParentS11nData {
    entity_ix: usize
}

pub struct HasParentS11n;

impl ComponentS11n<'_> for HasParentS11n {
    type SystemData = ();
    type Output = HasParent;

    fn load_async(&mut self, ctx: ComponentLoadArgs, _: &mut Self::SystemData) -> Pin<Box<dyn Future<Output=Self::Output> + Send + Sync>> {
        let s11n: HasParentS11nData = serde_json::from_value(ctx.data).unwrap();
        let ent = ctx.all_entity_vec[s11n.entity_ix];
        Box::pin(async move {
            HasParent {
                parent: ent
            }
        })
    }

    fn type_name(&self) -> &'static str { "HasParent" }
}
