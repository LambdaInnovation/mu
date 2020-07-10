pub use glium::uniforms;
pub use cgmath::num_traits::*;
pub use cgmath;
use std::f32::consts;
use std::ops::{Div, Mul, Sub};

pub type Float = f32;

pub type Vec3 = cgmath::Vector3<Float>;
pub type Vec2 = cgmath::Vector2<Float>;
pub type Mat4 = cgmath::Matrix4<Float>;

pub type Quaternion = cgmath::Quaternion<Float>;
// #[allow(dead_code)]
// pub type UnitQuaternion = nalgebra::UnitQuaternion<Float>;
pub type Rotation3 = cgmath::Basis3<Float>;

pub type Deg = cgmath::Deg<Float>;
pub type Euler = cgmath::Euler<Deg>;
pub type Perspective = cgmath::PerspectiveFov<Float>;

pub const PI: f32 = consts::PI;
pub const DEG_2_RAD: f32 = PI / 180.0;
pub const RAD_2_DEG: f32 = 180.0 / PI;

// TODO: Generalize to vec and other floats
// TODO: Bounded version
#[inline]
pub fn lerp(a: Float, b: Float, t: Float) -> Float {
    a + (b - a) * t
}

#[inline]
pub fn clamp<T: PartialOrd>(x: T, min: T, max: T) -> T {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

#[inline]
pub fn vec3(x: Float, y: Float, z: Float) -> Vec3 {
    Vec3::new(x, y, z)
}

#[inline]
pub fn deg(x: Float) -> Deg {
    cgmath::Deg(x)
}

#[inline]
pub fn deg2rad(deg: Float) -> Float {
    DEG_2_RAD * deg
}

#[inline]
pub fn rad2deg(rad: Float) -> Float {
    RAD_2_DEG * rad
}

/// Calculates the modulo-friendly division for a/b, which is only well defined for b>0.
#[inline]
pub fn div_low<T>(a: T, b: T) -> T
where
    T: Copy + One + Ord + Sub<Output = T> + Div<Output = T> + Mul<Output = T>,
{
    let myone = T::one();
    let res = a / b;
    if res * b > a {
        res - myone
    } else {
        res
    }
}

pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    cgmath::ortho(left, right, bottom, top, near, far)
}

pub fn perspective(fov: Deg, aspect: Float, z_near: Float, z_far: Float) -> Mat4 {
    cgmath::perspective(fov, aspect, z_near, z_far)
}

pub mod rand {
    pub use rand::prelude::*;

    type Float = super::Float;

    pub fn range(from: Float, to: Float) -> Float {
        from + (to - from) * random::<Float>()
    }
}
