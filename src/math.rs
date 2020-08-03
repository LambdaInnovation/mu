/// ! General mathematics operation.

use std::f32::consts;
use std::ops::{Div, Mul, Sub, Add};
use serde::{Serialize, Deserialize};

pub use cgmath::num_traits::*;

/// ! Many structs in cgmath crate is used by Mu::math, so we provide direct access here.
pub use cgmath;

pub type Float = f32;

pub type Vec4 = cgmath::Vector4<Float>;
pub type Vec3 = cgmath::Vector3<Float>;
pub type Vec2 = cgmath::Vector2<Float>;

pub type Mat3 = cgmath::Matrix3<Float>;
pub type Mat4 = cgmath::Matrix4<Float>;

pub type Quaternion = cgmath::Quaternion<Float>;
pub type Rotation3 = cgmath::Basis3<Float>;

pub type Deg = cgmath::Deg<Float>;
pub type Euler = cgmath::Euler<Deg>;
pub type Perspective = cgmath::PerspectiveFov<Float>;

pub const PI: f32 = consts::PI;
pub const DEG_2_RAD: f32 = PI / 180.0;
pub const RAD_2_DEG: f32 = 180.0 / PI;

#[derive(Serialize, Deserialize)]
#[serde(remote = "Vec2")]
pub struct Vec2SerdeRef {
    x: Float,
    y: Float
}

/// Linearly interpolates between a and b with parameter t.
#[inline]
pub fn lerp<T>(a: T, b: T, t: T) -> T
where T: Add<Output=T> + Sub<Output=T> + Mul<Output=T> + Copy {
    a + (b - a) * t
}

/// Limit x in the range of [min, max].
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

// Currently same as unity, maybe can be better?
#[inline]
pub fn approx_eq(lhs: f32, rhs: f32) -> bool {
    let delta = abs(rhs - lhs);
    delta < f32::max(f32::EPSILON * 8., 1e-6 * f32::max(abs(lhs), abs(rhs)))
}

#[inline]
pub fn vec2(x: Float, y: Float) -> Vec2 {
    Vec2::new(x, y)
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

/// Calculates floor(a / b) for a > 0 and b > 0.
#[inline]
pub fn div_floor<T>(a: T, b: T) -> T
where
    T: Copy + One + Ord + Sub<Output = T> + Div<Output = T> + Mul<Output = T>,
{
    let one = T::one();
    let res = a / b;
    if res * b > a {
        res - one
    } else {
        res
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {

    pub fn new_origin(width: f32, height: f32) -> Self {
        Self { x: 0., y: 0., width, height }
    }

    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn approx_eq(lhs: &Rect, rhs: &Rect) -> bool {
        approx_eq(lhs.x, rhs.x) &&
            approx_eq(lhs.y, rhs.y) &&
            approx_eq(lhs.width, rhs.width) &&
            approx_eq(lhs.height, rhs.height)
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.width, self.height)
    }

}

pub mod mat3 {
    use super::*;

    #[inline]
    pub fn extend_to_mat4(m: &Mat3) -> Mat4 {
        #[inline]
        fn ext_col(v: &Vec3) -> Vec4 {
            Vec4::new(v.x, v.y, 0., v.z)
        }

        Mat4 {
            x: ext_col(&m.x),
            y: ext_col(&m.y),
            z: Vec4::new(0., 0., 1., 0.),
            w: ext_col(&m.z)
        }
    }

    pub fn translate(p: Vec2) -> Mat3 {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        Mat3::new(
            1., 0., 0.,
            0., 1., 0.,
            p.x, p.y, 1.,
        )
    }

    pub fn ortho(left: f32, right: f32, bottom: f32, top: f32) -> Mat3 {
        let c0r0 = 2. / (right - left);
        let c0r1 = 0.;
        let c0r2 = 0.;

        let c1r0 = 0.;
        let c1r1 = 2. / (top - bottom);
        let c1r2 = 0.;

        let c2r0 = -(right + left) / (right - left);
        let c2r1 = -(top + bottom) / (top - bottom);
        let c2r2 = 1.;

        #[cfg_attr(rustfmt, rustfmt_skip)]
        Mat3::new(
            c0r0, c0r1, c0r2,
            c1r0, c1r1, c1r2,
            c2r0, c2r1, c2r2
        )
    }

    pub fn rotate_around(p: Vec2, angle: Deg) -> Mat3 {
        unimplemented!();
    }

    pub fn scale_around(p: Vec2, scl: Vec2) -> Mat3 {
        unimplemented!();
    }

    pub fn scale(scl: Vec2) -> Mat3 {
        let c0r0 = scl.x;
        let c1r0 = 0.;
        let c2r0 = 0.;
        let c0r1 = 0.;
        let c1r1 = scl.y;
        let c2r1 = 0.;
        let c0r2 = 0.;
        let c1r2 = 0.;
        let c2r2 = 1.;

        #[cfg_attr(rustfmt, rustfmt_skip)]
            Mat3::new(
            c0r0, c0r1, c0r2,
            c1r0, c1r1, c1r2,
            c2r0, c2r1, c2r2
        )
    }
}

/// Convenient matrix operations.
pub mod mat {
    use super::*;

    pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
        cgmath::ortho(left, right, bottom, top, near, far)
    }

    pub fn perspective(fov: Deg, aspect: Float, z_near: Float, z_far: Float) -> Mat4 {
        cgmath::perspective(fov, aspect, z_near, z_far)
    }
}


pub mod rand {
    pub use rand::prelude::*;

    type Float = super::Float;

    pub fn range(from: Float, to: Float) -> Float {
        from + (to - from) * random::<Float>()
    }
}
